//! Session and activity tracking driven by webhook events.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use sqlx::PgPool;

use crate::error::AdminResult;
use crate::repositories::dashboard::session_registry;
use crate::types::webhook::{ContextWindow, StatusLinePayload, StatusLineQuery};

use super::helpers::authenticate_webhook_claims;

const MICRODOLLARS_PER_USD: f64 = 1_000_000.0;

pub(crate) async fn track_statusline_event(
    State(pool): State<Arc<PgPool>>,
    headers: HeaderMap,
    Query(query): Query<StatusLineQuery>,
    Json(payload): Json<StatusLinePayload>,
) -> AdminResult<Response> {
    let claims = authenticate_webhook_claims(&headers)?;

    let Some(session_id) = query.session_id else {
        tracing::debug!(user_id = %claims.sub, "Statusline event without a session id");
        return Ok(StatusCode::NO_CONTENT.into_response());
    };

    session_registry::update_session_statusline(&session_registry::StatuslineParams {
        pool: &pool,
        session_id: &session_id,
        model: payload
            .model
            .as_ref()
            .and_then(|m| m.api_model_id.as_deref()),
        live_cost_microdollars: payload
            .cost
            .and_then(|c| c.total_cost_usd)
            .map(to_microdollars),
        context_pct: payload.context_window.and_then(context_pct),
    })
    .await;

    Ok(StatusCode::NO_CONTENT.into_response())
}

fn to_microdollars(usd: f64) -> i64 {
    let scaled = (usd * MICRODOLLARS_PER_USD).round();
    if scaled.is_finite() {
        crate::numeric::to_i64(scaled)
    } else {
        0
    }
}

fn context_pct(window: ContextWindow) -> Option<i16> {
    let size = window.context_window_size.filter(|s| *s > 0)?;
    let usage = window.current_usage?;
    let used = usage.input.unwrap_or(0) + usage.output.unwrap_or(0);
    let pct = used.saturating_mul(100) / size;
    i16::try_from(pct.clamp(0, 100)).ok()
}
