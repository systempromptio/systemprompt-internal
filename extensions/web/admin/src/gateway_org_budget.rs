//! Gateway guards enforcing what an organization's plan bought.
//!
//! Two guards, registered through `register_gateway_guard!` and consulted by
//! the gateway on every `/v1/messages` request right after the quota precheck:
//!
//! - [`RouteEntitlementGuard`] — the plan's **model tier**. The resolved route
//!   is a `gateway_route` entity, so this is the ordinary authz resolver run
//!   over the same rules the access matrix shows, with the organization
//!   dimension in the ladder. A denial is 403: no amount of retrying buys a
//!   model the customer did not pay for.
//! - [`OrgBudgetGuard`] — the plan's **monthly spend cap**. A denial is 429
//!   with the default quota kind, because the customer's month does roll over.
//!
//! Why the budget cap is not a `quota_windows` entry now that core supports
//! `subject: organization` buckets: `ai_gateway_policies` rows are global, so a
//! window there is one number for every customer. The cap is per-plan, and a
//! plan is a property of the organization, not of the policy. Core's
//! subject-keyed windows are the right tool for a house-wide backstop; this
//! guard is what makes Standard and Enterprise differ.
//!
//! The cap is enforced one request late, because a request's cost is known only
//! after its response. That is true of core's own ceilings too, and for a
//! monthly contract cap overshooting by one request is immaterial.

use sqlx::PgPool;
use systemprompt::extension::{
    GatewayDenyReason, GatewayGuardRequest, GatewayRequestGuard, register_gateway_guard,
};
use systemprompt::identifiers::UserId;
use systemprompt_security::authz::resolver::{ResolveInput, resolve};
use systemprompt_security::authz::{AuthzError, Decision, EntityRef};

use crate::authz;
use crate::repositories::config::gateway_acl;
use crate::repositories::organizations;

#[derive(Debug, Clone, Copy, Default)]
pub struct RouteEntitlementGuard;

#[derive(Debug, Clone, Copy, Default)]
pub struct OrgBudgetGuard;

#[async_trait::async_trait]
impl GatewayRequestGuard for RouteEntitlementGuard {
    async fn check(
        &self,
        pool: &PgPool,
        request: &GatewayGuardRequest<'_>,
    ) -> Result<(), GatewayDenyReason> {
        let Some(route_id) = request.route_id else {
            return Ok(());
        };
        let user_id = UserId::new(request.user_id.to_owned());
        // Why: every input to this decision — the route entity, its rules, the
        // caller's roles and their subject attributes — reads as a grant when it
        // is missing: no rule matches, and the request passes. A lookup that
        // fails is not an entitlement, so the request is held instead.
        let decision = match resolve_route(pool, route_id, &user_id).await {
            Ok(Some(decision)) => decision,
            Ok(None) => return Ok(()),
            Err(e) => {
                tracing::error!(
                    error = %e, route_id, user_id = %user_id,
                    "route entitlement unresolved: entitlement lookup failed",
                );
                return Err(GatewayDenyReason::unavailable(
                    "Entitlements are temporarily unavailable.",
                ));
            },
        };
        let Decision::Deny { reason } = decision else {
            return Ok(());
        };

        tracing::warn!(
            user_id = request.user_id,
            route_id,
            model = request.model,
            ?reason,
            "gateway request denied: route not included in the caller's plan",
        );
        Err(GatewayDenyReason::forbidden(format!(
            "{} is not included in your plan.",
            request.model
        )))
    }
}

async fn resolve_route(
    pool: &PgPool,
    route_id: &str,
    user_id: &UserId,
) -> Result<Option<Decision>, AuthzError> {
    let entity = gateway_acl::find_entity(pool, route_id).await?;
    let rules = gateway_acl::list_rules_for_route(pool, route_id).await?;
    // Why: no `users` row is not a failed lookup — it is an answer, and the
    // resolver reads it as a caller holding no roles.
    let Some(user_roles) = load_roles(pool, user_id).await? else {
        return Ok(None);
    };
    let attributes = authz::subject_attributes_for(pool, user_id).await?;

    let entity_ref =
        EntityRef::GatewayRoute(systemprompt::identifiers::RouteId::new(route_id.to_owned()));

    Ok(Some(resolve(ResolveInput {
        entity: &entity_ref,
        rules: &rules,
        user_id,
        user_roles: &user_roles,
        default_included: entity.map(|e| e.default_included),
        parents: &[],
        attributes: &attributes,
        dimensions: authz::dimensions(pool),
    })))
}

async fn load_roles(pool: &PgPool, user_id: &UserId) -> Result<Option<Vec<String>>, AuthzError> {
    let roles = sqlx::query_scalar!(
        r#"SELECT roles AS "roles!: Vec<String>" FROM users WHERE id = $1"#,
        user_id.as_str()
    )
    .fetch_optional(pool)
    .await?;
    Ok(roles)
}

#[async_trait::async_trait]
impl GatewayRequestGuard for OrgBudgetGuard {
    async fn check(
        &self,
        pool: &PgPool,
        request: &GatewayGuardRequest<'_>,
    ) -> Result<(), GatewayDenyReason> {
        // Why: a cap this guard cannot read is not a cap that was met. The
        // module tolerates overshooting by one request because a request's cost
        // is known only after it runs; a lookup that keeps failing overshoots
        // without bound, so the request is held. Holding is the quota denial —
        // 429, retryable — and costs a caller nothing once the read recovers.
        // No spend row is a different answer: the caller is in no organization
        // carrying a cap, and nothing constrains them here.
        let user_id = UserId::new(request.user_id.to_owned());
        let spend = match organizations::spend::find_spend_for_user(pool, &user_id).await {
            Ok(Some(spend)) => spend,
            Ok(None) => return Ok(()),
            Err(e) => {
                tracing::error!(
                    error = %e, user_id = request.user_id,
                    "organization budget unresolved: spend lookup failed",
                );
                return Err(GatewayDenyReason::unavailable(
                    "Spend limits are temporarily unavailable.",
                ));
            },
        };
        if spend.spent_microdollars < spend.cap_microdollars {
            return Ok(());
        }

        tracing::warn!(
            user_id = request.user_id,
            organization = %spend.name,
            spent_microdollars = spend.spent_microdollars,
            cap_microdollars = spend.cap_microdollars,
            "gateway request denied: organization monthly budget exhausted",
        );
        Err(GatewayDenyReason::new(format!(
            "{} has reached its monthly spend cap of ${:.2}. Contact your administrator to raise \
             the plan limit.",
            spend.name,
            micro_to_usd(spend.cap_microdollars),
        )))
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "display only: a contract cap in dollars is far below f64's exact-integer range"
)]
fn micro_to_usd(microdollars: i64) -> f64 {
    microdollars as f64 / 1_000_000.0
}

register_gateway_guard!(RouteEntitlementGuard);
register_gateway_guard!(OrgBudgetGuard);
