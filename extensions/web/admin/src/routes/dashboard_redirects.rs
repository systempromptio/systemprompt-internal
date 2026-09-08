//! Compatibility redirects for the newly shared dashboard surfaces.

use axum::Router;
use axum::extract::{Path, RawQuery};
use axum::response::Redirect;
use axum::routing::get;
use sqlx::PgPool;
use std::sync::Arc;

pub(super) fn legacy_routes() -> Router<Arc<PgPool>> {
    Router::new()
        .route("/access/groups/unassigned", get(access_unassigned))
        .route("/access/groups", get(access_groups))
        .route("/access/groups/{group_id}", get(access_group_detail))
        .route("/access/projects", get(access_projects))
        .route("/access/projects/{project_id}", get(access_project_detail))
        .route("/access/control", get(access_control))
        .route("/catalog/marketplaces", get(catalog_marketplaces))
        .route(
            "/catalog/marketplaces/{marketplace_id}",
            get(catalog_marketplace_detail),
        )
        .route("/catalog/access-control", get(access_control))
        .route("/analytics/users/{user_id}", get(analytics_user))
        .route("/governance/warnings", get(governance_warnings))
}

async fn access_control(RawQuery(q): RawQuery) -> Redirect {
    with_query("/admin/access-control", q)
}

async fn access_group_detail(Path(id): Path<String>, RawQuery(q): RawQuery) -> Redirect {
    with_query(&format!("/admin/groups/{id}"), q)
}

async fn access_groups(RawQuery(q): RawQuery) -> Redirect {
    with_query("/admin/groups", q)
}

async fn access_project_detail(Path(id): Path<String>, RawQuery(q): RawQuery) -> Redirect {
    with_query(&format!("/admin/projects/{id}"), q)
}

async fn access_projects(RawQuery(q): RawQuery) -> Redirect {
    with_query("/admin/projects", q)
}

async fn access_unassigned() -> Redirect {
    moved("/admin/users?filter=unassigned")
}

async fn analytics_user(Path(user_id): Path<String>) -> Redirect {
    moved(&format!("/admin/users/{user_id}?tab=usage"))
}

async fn catalog_marketplace_detail(Path(id): Path<String>, RawQuery(q): RawQuery) -> Redirect {
    with_query(&format!("/admin/marketplaces/{id}"), q)
}

async fn catalog_marketplaces(RawQuery(q): RawQuery) -> Redirect {
    with_query("/admin/marketplaces", q)
}

async fn governance_warnings(RawQuery(q): RawQuery) -> Redirect {
    with_query("/admin/governance", q)
}

fn moved(target: &str) -> Redirect {
    Redirect::permanent(target)
}

fn with_query(base: &str, query: Option<String>) -> Redirect {
    match query {
        Some(q) if !q.is_empty() => moved(&format!("{base}?{q}")),
        _ => moved(base),
    }
}
