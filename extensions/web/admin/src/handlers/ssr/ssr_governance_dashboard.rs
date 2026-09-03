//! `/admin/governance` — the policy chain, what each stage is doing right
//! now, and where its decisions are coming from. Companion to
//! `/admin/entities/skills`: skill invocations are the most common source of
//! governed tool calls, so each policy card links out to the demo skill's
//! decisions and the dashboard cross-links back to skill usage.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::response::Response;
use serde::Serialize;
use sqlx::PgPool;

use crate::error::{AdminError, AdminHtmlResult};
use crate::handlers::webhook::governance::engine as governance_engine;
use crate::repositories::governance;
use crate::repositories::governance::PerPolicyCounts;
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, TopActor, TopPolicy, UserContext};

const WINDOW_24H_SECS: i64 = 86_400;
const TOP_LIMIT: i64 = 10;

#[derive(Debug, Serialize)]
struct GovernanceDashboardContext {
    page: &'static str,
    title: &'static str,
    window_total: i64,
    window_allowed: i64,
    window_denied: i64,
    window_breaches: i64,
    lifetime_total: i64,
    has_policies: bool,
    policies: Vec<PolicyRow>,
    has_top_policies: bool,
    top_policies: Vec<TopPolicyRow>,
    has_top_actors: bool,
    top_actors: Vec<TopActorRow>,
    config_path: &'static str,
}

#[derive(Debug, Serialize)]
struct PolicyRow {
    id: String,
    name: String,
    description: String,
    enabled: bool,
    window_allowed: i64,
    window_denied: i64,
    window_evaluations: i64,
    deny_rate: String,
    has_recent_denies: bool,
    last_at: String,
    decisions_url: String,
    deny_decisions_url: String,
}

#[derive(Debug, Serialize)]
struct TopPolicyRow {
    policy: String,
    tool_name: String,
    hits: i64,
    distinct_actors: i64,
    decisions_url: String,
}

#[derive(Debug, Serialize)]
struct TopActorRow {
    display_name: String,
    email: Option<String>,
    deny_count: i64,
    secret_count: i64,
    total: i64,
}

fn format_deny_rate(denied: i64, evaluations: i64) -> String {
    if evaluations <= 0 {
        return "—".to_owned();
    }
    let rate = (denied as f64 / evaluations as f64) * 100.0;
    format!("{rate:.1}%")
}

fn format_local(t: chrono::DateTime<chrono::Utc>) -> String {
    t.with_timezone(&chrono::Local)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

fn build_policy_rows(
    window_by_id: &mut HashMap<String, PerPolicyCounts>,
) -> Vec<PolicyRow> {
    governance_engine::engine()
        .policies()
        .map(|(cfg, p)| {
            let id = p.id().as_str().to_owned();
            let win = window_by_id.remove(&id);
            let window_allowed = win.as_ref().map_or(0, |s| s.allowed);
            let window_denied = win.as_ref().map_or(0, |s| s.denied);
            let window_evaluations = window_allowed + window_denied;
            let last_at = win
                .as_ref()
                .and_then(|s| s.last_at)
                .map(format_local)
                .unwrap_or_default();
            PolicyRow {
                name: p.name().to_owned(),
                description: p.description().to_owned(),
                enabled: cfg.enabled,
                window_allowed,
                window_denied,
                window_evaluations,
                deny_rate: format_deny_rate(window_denied, window_evaluations),
                has_recent_denies: window_denied > 0,
                last_at,
                decisions_url: format!("/admin/governance/decisions?policy={id}"),
                deny_decisions_url: format!(
                    "/admin/governance/decisions?policy={id}&outcome=deny"
                ),
                id,
            }
        })
        .collect()
}

fn build_top_policy_rows(top_policies: &[TopPolicy]) -> Vec<TopPolicyRow> {
    top_policies
        .iter()
        .map(|t| TopPolicyRow {
            policy: t.policy.clone(),
            tool_name: t.tool_name.clone(),
            hits: t.hits,
            distinct_actors: t.distinct_actors,
            decisions_url: format!(
                "/admin/governance/decisions?policy={}&outcome=deny",
                t.policy
            ),
        })
        .collect()
}

fn build_top_actor_rows(top_actors: &[TopActor]) -> Vec<TopActorRow> {
    top_actors
        .iter()
        .map(|a| TopActorRow {
            display_name: a.display_name.clone(),
            email: a.email.clone(),
            deny_count: a.deny_count,
            secret_count: a.secret_count,
            total: a.total,
        })
        .collect()
}

async fn build_page_json(pool: &PgPool) -> GovernanceDashboardContext {
    let (lifetime, window, per_policy_window, top_policies, top_actors) = tokio::join!(
        governance::get_governance_counts(pool),
        governance::get_governance_counts_windowed(pool, WINDOW_24H_SECS),
        governance::list_per_policy_counts_windowed(pool, WINDOW_24H_SECS),
        governance::list_top_policies(pool, WINDOW_24H_SECS, TOP_LIMIT),
        governance::list_top_actors(pool, WINDOW_24H_SECS, TOP_LIMIT),
    );

    let lifetime = lifetime.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "governance lifetime counts query failed");
        governance::GovernanceCounts::default()
    });
    let window = window.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "governance windowed counts query failed");
        governance::GovernanceCounts::default()
    });
    let mut window_by_id: HashMap<String, PerPolicyCounts> = per_policy_window
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "per-policy windowed counts query failed");
            Vec::new()
        })
        .into_iter()
        .map(|r| (r.policy.clone(), r))
        .collect();
    let top_policies = top_policies.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "top denying policies query failed");
        Vec::new()
    });
    let top_actors = top_actors.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "top denied actors query failed");
        Vec::new()
    });

    let policies = build_policy_rows(&mut window_by_id);
    let top_policies_view = build_top_policy_rows(&top_policies);
    let top_actors_view = build_top_actor_rows(&top_actors);

    GovernanceDashboardContext {
        page: "governance",
        title: "Governance",
        window_total: window.total,
        window_allowed: window.allowed,
        window_denied: window.denied,
        window_breaches: window.secret_breaches,
        lifetime_total: lifetime.total,
        has_policies: !policies.is_empty(),
        policies,
        has_top_policies: !top_policies_view.is_empty(),
        top_policies: top_policies_view,
        has_top_actors: !top_actors_view.is_empty(),
        top_actors: top_actors_view,
        config_path: "services/governance/config.yaml",
    }
}

pub(crate) async fn governance_dashboard_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
) -> AdminHtmlResult<Response> {
    if !user_ctx.is_admin {
        return Err(AdminError::Forbidden("Admin access required.".to_owned()).into());
    }
    let payload = build_page_json(&pool).await;
    Ok(super::render_typed_page(
        &engine, "governance", &payload, &user_ctx, &mkt_ctx,
    ))
}
