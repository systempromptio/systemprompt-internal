//! How much of a federated provider's claim about roles is allowed to land.
//!
//! Why this is its own module: role authority is a privilege-escalation
//! boundary, not bookkeeping. It decides when an external identity provider
//! may raise a local account's privileges, and the answer depends on whether
//! the binding was already established rather than on the sign-in succeeding.
//! Keeping it beside the row-writing code invites a change to one being read
//! as a change to the other.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;
use systemprompt_web_shared::error::MarketplaceError;

// Why: how much authority the provider has over this row's roles on *this* sign-in.
//
// Why it varies: the provider earns role authority from an established
// binding, not from the mere fact of authenticating. A first attachment to a
// pre-existing local account is exactly the moment where granting would turn
// control of an external account into control of a local one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RoleAuthority {
    // Why: returning sign-in on an established `(issuer, external_sub)` mapping.
    // The provider is the authority: grants and revocations both apply,
    // which is what makes group mapping work.
    Sync,
    // Why: first attachment to a pre-existing local row. Revocations apply, grants
    // do not — a first bind must never hand out a role the account did not
    // have.
    //
    // A row being *created* has no authority question to answer, so it does
    // not appear here: `create_federated` writes its roles directly,
    // through the same [`strip_gated_grants`] filter.
    DowngradeOnly,
}

// Why: roles a federated claim may never *add* without being explicitly permitted
// to.
const GATED_ROLES: [&str; 1] = ["admin"];

// Why: env flag permitting a federated claim to add [`GATED_ROLES`]. Default off.
const ALLOW_GRANT_ENV: &str = "FEDERATED_ROLES_MAY_GRANT_ADMIN";

fn federated_may_grant_gated_roles() -> bool {
    std::env::var(ALLOW_GRANT_ENV)
        .is_ok_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
}

// Why: drops role additions that a federated claim is not permitted to make.
//
// Why this applies on provisioning too: a brand-new account is where the grant
// is least visible and most valuable to an attacker. If control of an external
// account could mint a *platform admin* on first sign-in, the external
// system's user list would silently become the platform's admin list.
//
// Removals are never gated — only additions of [`GATED_ROLES`] the account
// does not already hold.
pub(super) fn strip_gated_grants(issuer: &str, stored: &[String], next: &mut Vec<String>) {
    if federated_may_grant_gated_roles() {
        return;
    }
    next.retain(|role| {
        let is_new_gated_grant = GATED_ROLES.contains(&role.as_str()) && !stored.contains(role);
        if is_new_gated_grant {
            tracing::warn!(
                issuer = %issuer,
                role = %role,
                "Federated sign-in tried to grant a gated role; dropped. Set {} to permit it.",
                ALLOW_GRANT_ENV
            );
        }
        !is_new_gated_grant
    });
}

// JSON: JSONB column — `user_activity.metadata`
#[derive(serde::Serialize)]
struct RoleChangeMetadata<'a> {
    issuer: &'a str,
    authority: &'a str,
    previous_roles: &'a [String],
    new_roles: &'a [String],
}

// Why: applies provider-computed roles within the authority this sign-in carries.
//
// `None` means the caller could not compute roles this time; the stored set
// stands. Dropped grants are logged and skipped rather than raised: a sign-in
// should not fail because the provider tried to over-grant.
// Why: the five things that describe one role application, carried together
// so the signature cannot be called with `issuer` and the stored roles the
// wrong way round — they are both plain strings at the call site.
pub(super) struct RoleUpdate<'a> {
    pub user_id: &'a UserId,
    pub issuer: &'a str,
    pub stored: Vec<String>,
    pub desired: Option<&'a [String]>,
    pub authority: RoleAuthority,
}

pub(super) async fn apply_roles(
    pool: &PgPool,
    update: RoleUpdate<'_>,
) -> Result<Vec<String>, MarketplaceError> {
    let RoleUpdate {
        user_id,
        issuer,
        stored,
        desired,
        authority,
    } = update;
    let Some(desired) = desired else {
        return Ok(stored);
    };

    let mut next: Vec<String> = match authority {
        RoleAuthority::Sync => desired.to_vec(),
        // Why: keep only what the account already had: revocations land, grants do not.
        RoleAuthority::DowngradeOnly => desired
            .iter()
            .filter(|r| stored.contains(r))
            .cloned()
            .collect(),
    };

    strip_gated_grants(issuer, &stored, &mut next);

    if stored == next {
        return Ok(stored);
    }

    sqlx::query!(
        "UPDATE users SET roles = $2, updated_at = CURRENT_TIMESTAMP WHERE id = $1",
        user_id.as_str(),
        &next,
    )
    .execute(pool)
    .await?;

    tracing::info!(
        user_id = %user_id,
        issuer = %issuer,
        authority = ?authority,
        previous = ?stored,
        roles = ?next,
        "Federated sign-in updated platform roles"
    );

    // Why: audited, not just logged: this is the one path where an external system
    // changes local privilege, so it belongs in the timeline an operator reads.
    let metadata = serde_json::to_value(RoleChangeMetadata {
        issuer,
        authority: match authority {
            RoleAuthority::Sync => "sync",
            RoleAuthority::DowngradeOnly => "downgrade_only",
        },
        previous_roles: &stored,
        new_roles: &next,
    })
    .unwrap_or(serde_json::Value::Null);

    crate::repositories::users::activity::record(
        pool,
        crate::activity::NewActivity {
            user_id: user_id.clone(),
            category: crate::activity::ActivityCategory::UserManagement,
            action: crate::activity::ActivityAction::Updated,
            entity: None,
            description: format!("Federated sign-in via {issuer} updated platform roles"),
            metadata,
        },
    )
    .await;

    Ok(next)
}
