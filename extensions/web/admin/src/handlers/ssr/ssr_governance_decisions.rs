//! `/admin/governance/decisions?policy=<id>` — recent decisions for one
//! policy stage, the drill-through target from the governance dashboard.

use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use systemprompt::identifiers::UserId;

use crate::error::{AdminError, AdminHtmlResult};
use crate::repositories::governance::list_decisions_for_policy;
use crate::templates::AdminTemplateEngine;
use crate::types::{GovernanceDecisionRow, MarketplaceContext, UserContext};

const DECISIONS_LIMIT: i64 = 200;

#[derive(Debug, Deserialize)]
pub(crate) struct DecisionsQuery {
    policy: Option<String>,
    outcome: Option<String>,
}

#[derive(Debug, Serialize)]
struct DecisionRowView {
    created_at: String,
    decision: String,
    is_deny: bool,
    tool_name: String,
    user_id: UserId,
    agent_scope: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct GovernanceDecisionsContext {
    page: &'static str,
    title: &'static str,
    hero_subtitle: String,
    policy: String,
    total: usize,
    denied: usize,
    allowed: usize,
    at_limit: bool,
    limit: i64,
    rows: Vec<DecisionRowView>,
    has_rows: bool,
}

fn matches_outcome(row: &GovernanceDecisionRow, outcome: Option<&str>) -> bool {
    match outcome {
        Some("deny") => row.decision == "deny",
        Some("allow") => row.decision == "allow",
        _ => true,
    }
}

pub(crate) async fn governance_decisions_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Query(params): Query<DecisionsQuery>,
) -> AdminHtmlResult<Response> {
    if !user_ctx.is_admin {
        return Err(AdminError::Forbidden("Admin access required.".to_owned()).into());
    }

    let policy = params.policy.unwrap_or_default();
    let outcome = params.outcome.as_deref();

    let decisions = if policy.is_empty() {
        Vec::new()
    } else {
        list_decisions_for_policy(&pool, &policy, DECISIONS_LIMIT)
            .await
            .map_err(AdminError::from)?
    };

    let total = decisions.len();
    let rows: Vec<DecisionRowView> = decisions
        .into_iter()
        .filter(|d| matches_outcome(d, outcome))
        .map(|d| DecisionRowView {
            created_at: d.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            is_deny: d.decision == "deny",
            decision: d.decision,
            tool_name: d.tool_name,
            user_id: d.user_id,
            agent_scope: d.agent_scope.unwrap_or_default(),
            reason: d.reason,
        })
        .collect();

    let denied = rows.iter().filter(|r| r.is_deny).count();

    let ctx = GovernanceDecisionsContext {
        page: "governance-decisions",
        title: "Governance Decisions",
        hero_subtitle: if policy.is_empty() {
            "Choose a policy from the governance dashboard to see its decisions.".to_owned()
        } else {
            format!("Recent decisions from the {policy} policy stage.")
        },
        total: rows.len(),
        denied,
        allowed: rows.len() - denied,
        at_limit: total == usize::try_from(DECISIONS_LIMIT).unwrap_or(usize::MAX),
        limit: DECISIONS_LIMIT,
        has_rows: !rows.is_empty(),
        rows,
        policy,
    };

    Ok(super::render_typed_page(
        &engine,
        "governance-decisions",
        &ctx,
        &user_ctx,
        &mkt_ctx,
    ))
}
