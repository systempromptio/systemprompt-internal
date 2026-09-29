//! Server-rendered routes for the console's operating pages: what the
//! instance keeps and archives (Data lifecycle), what it is configured with
//! (Configuration), where its telemetry goes (Observability), and every tool
//! call and artifact it saw (Tools & artifacts).
//!
//! Merged into the SSR router beside the sidebar-group modules in `ssr.rs`,
//! so each group's table stays short enough to read in one screen.

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use sqlx::PgPool;

use super::super::handlers;

pub(super) fn routes() -> Router<Arc<PgPool>> {
    Router::new()
        // Why: every tool result, from every client, as one linked entity;
        // the preview is the artifact rendered for the detail page's frame.
        .route("/tools", get(handlers::ssr::tools_page))
        .route("/artifacts", get(handlers::ssr::artifacts_page))
        .route(
            "/artifacts/{artifact_id}",
            get(handlers::ssr::artifact_detail_page),
        )
        .route(
            "/artifacts/{artifact_id}/preview",
            get(handlers::ssr::artifact_preview),
        )
        // Why: the Platform group's home — every kind of configuration the
        // instance loads, where it comes from and whether the database
        // agrees.
        .route("/configuration", get(handlers::ssr::configuration_page))
        // Why: the retention ledger — measurements, archives and the health
        // report the retention_* jobs write — beside Configuration, where the
        // windows and the cleanup job's last run are shown.
        .route("/lifecycle", get(handlers::ssr::lifecycle_page))
        .route(
            "/lifecycle/archive/{tier}/{period}/{file}",
            get(handlers::ssr::lifecycle_archive_download),
        )
        // Why: the exporter is core's `otlp_export` job and its config is a
        // profile block; this page shows both and triggers the job out of
        // turn. It lives beside Sync because that is where the declared
        // config is read from.
        .route(
            "/system/observability",
            get(handlers::ssr::observability_page),
        )
        .route(
            "/system/observability/export",
            post(handlers::ssr::observability_export_now),
        )
        .route(
            "/system/observability/test",
            post(handlers::ssr::observability_test_connection),
        )
}
