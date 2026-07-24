//! SSR decisions ledger: recent governance decisions, filterable by policy,
//! outcome, and user. The drill-through target for every policy card.

use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::error::{AdminError, AdminHtmlResult};
use crate::repositories;
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};

const DECISIONS_LIMIT: i64 = 200;

#[derive(Debug, Deserialize)]
pub(crate) struct DecisionsQuery {
    policy: Option<String>,
    outcome: Option<String>,
    user_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct DecisionRowView {
    created_at: String,
    policy: String,
    decision: String,
    is_deny: bool,
    tool_name: String,
    user_id: String,
    user_url: String,
    agent_scope: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct FilterChip {
    label: String,
    value: String,
    remove_url: String,
}

#[derive(Debug, Serialize)]
struct GovernanceDecisionsContext {
    page: &'static str,
    title: &'static str,
    hero_title: &'static str,
    hero_subtitle: &'static str,
    total: usize,
    denied: usize,
    allowed: usize,
    at_limit: bool,
    limit: i64,
    rows: Vec<DecisionRowView>,
    has_rows: bool,
    chips: Vec<FilterChip>,
    has_chips: bool,
}

fn normalize(param: Option<&String>) -> Option<&str> {
    param.map(String::as_str).filter(|s| !s.is_empty())
}

fn build_url(policy: Option<&str>, outcome: Option<&str>, user_id: Option<&str>) -> String {
    let mut parts = Vec::new();
    if let Some(p) = policy {
        parts.push(format!("policy={p}"));
    }
    if let Some(o) = outcome {
        parts.push(format!("outcome={o}"));
    }
    if let Some(u) = user_id {
        parts.push(format!("user_id={u}"));
    }
    if parts.is_empty() {
        "/admin/governance/decisions".to_owned()
    } else {
        format!("/admin/governance/decisions?{}", parts.join("&"))
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

    let policy = normalize(params.policy.as_ref());
    let outcome = normalize(params.outcome.as_ref());
    let user_id = normalize(params.user_id.as_ref());

    let decisions = repositories::governance::decisions::list_decisions_filtered(
        &pool,
        policy,
        outcome,
        user_id,
        DECISIONS_LIMIT,
    )
    .await
    .map_err(AdminError::from)?;

    let mut chips = Vec::new();
    if let Some(p) = policy {
        chips.push(FilterChip {
            label: "Policy".to_owned(),
            value: p.to_owned(),
            remove_url: build_url(None, outcome, user_id),
        });
    }
    if let Some(o) = outcome {
        chips.push(FilterChip {
            label: "Outcome".to_owned(),
            value: o.to_owned(),
            remove_url: build_url(policy, None, user_id),
        });
    }
    if let Some(u) = user_id {
        chips.push(FilterChip {
            label: "User".to_owned(),
            value: u.to_owned(),
            remove_url: build_url(policy, outcome, None),
        });
    }

    let total = decisions.len();
    let denied = decisions.iter().filter(|d| d.decision == "deny").count();
    let rows: Vec<DecisionRowView> = decisions
        .into_iter()
        .map(|d| DecisionRowView {
            created_at: d.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            is_deny: d.decision == "deny",
            user_url: format!("/admin/user?id={}", d.user_id),
            user_id: d.user_id.to_string(),
            policy: d.policy,
            decision: d.decision,
            tool_name: d.tool_name,
            agent_scope: d.agent_scope.unwrap_or_default(),
            reason: d.reason,
        })
        .collect();

    let ctx = GovernanceDecisionsContext {
        page: "governance-decisions",
        title: "Governance Decisions",
        hero_title: "Governance Decisions",
        hero_subtitle: "Every allow/deny verdict the policy pipeline has issued, newest first.",
        total,
        denied,
        allowed: total - denied,
        at_limit: total == usize::try_from(DECISIONS_LIMIT).unwrap_or(usize::MAX),
        limit: DECISIONS_LIMIT,
        has_rows: !rows.is_empty(),
        rows,
        has_chips: !chips.is_empty(),
        chips,
    };

    Ok(super::render_typed_page(
        &engine,
        "governance-decisions",
        &ctx,
        &user_ctx,
        &mkt_ctx,
    ))
}
