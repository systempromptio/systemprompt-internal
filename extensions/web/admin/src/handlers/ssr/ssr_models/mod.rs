//! `/admin/models` — the Pi demo model-selection screen.
//!
//! One page that closes the demo loop: which models the gateway exposes,
//! whether the selected user may call each one (a `user`-band deny in
//! `access_control_rules` overrides the role allow on the next request),
//! and what that user has actually done — requests, tokens, cost, denials.

use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::response::Response;
use serde::Deserialize;
use sqlx::PgPool;

use crate::error::{AdminError, AdminHtmlResult};
use crate::repositories;
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};

use super::types::PageStatView;

mod data;
mod view;

use view::{ModelsPageData, build_user_options};

#[derive(Debug, Deserialize)]
pub(crate) struct ModelsQuery {
    user_id: Option<systemprompt::identifiers::UserId>,
}

pub(crate) async fn models_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Query(params): Query<ModelsQuery>,
) -> AdminHtmlResult<Response> {
    if !user_ctx.is_admin {
        return Err(AdminError::Forbidden("Admin access required.".to_owned()).into());
    }

    let all_users = repositories::users::queries::list_users(&pool)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to list users for model selection");
            vec![]
        });

    let selected_id: Option<String> = params
        .user_id
        .map(|u| u.to_string())
        .filter(|s| !s.is_empty());
    let users = build_user_options(&all_users, selected_id.as_deref());

    let selected_user_label = users
        .iter()
        .find(|u| u.selected)
        .map(|u| u.label.clone())
        .unwrap_or_default();
    let selected_user_id = selected_id.clone().unwrap_or_default();
    let has_selection = selected_id.is_some();

    let models = data::load_model_rows(&pool, selected_id.as_deref()).await?;
    let (usage, usage_totals) = data::load_usage(&pool, selected_id.as_deref()).await;

    let denied_models = models.iter().filter(|m| m.denied).count();
    let page_stats = vec![
        PageStatView {
            value: models.len() as i64,
            label: "Models",
        },
        PageStatView {
            value: denied_models as i64,
            label: "Disabled",
        },
        PageStatView {
            value: usage_totals.requests,
            label: "Requests",
        },
    ];

    let requests_link = if has_selection {
        format!("/admin/entities/requests?user_id={selected_user_id}")
    } else {
        "/admin/entities/requests".to_owned()
    };

    let data = ModelsPageData {
        page: "models",
        title: "Model Selection",
        users,
        has_selection,
        selected_user_id,
        selected_user_label,
        models,
        usage,
        has_usage: usage_totals.requests > 0,
        usage_totals,
        requests_link,
        page_stats,
    };

    Ok(super::render_typed_page(
        &engine, "models", &data, &user_ctx, &mkt_ctx,
    ))
}
