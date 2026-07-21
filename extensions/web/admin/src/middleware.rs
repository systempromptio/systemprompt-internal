//! Admin plane request middleware: authentication, authorisation, and page
//! context.
//!
//! The marketplace counts injected into every render are cached because they
//! are derived from a remote catalog and are identical for every user holding
//! the same role set.

use std::sync::Arc;

use axum::Extension;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use sqlx::PgPool;
use systemprompt::identifiers::{Email, UserId};

use super::handlers::extract_user_from_cookie;
use super::handlers::shared::ErrorBody;
use super::types::UserContext;

#[derive(Debug, Serialize)]
struct AuthMeResponse {
    user_id: UserId,
    username: String,
    email: Email,
    roles: Vec<String>,
    department: String,
    is_admin: bool,
}

pub(crate) use super::marketplace_context::marketplace_context_middleware;

pub(crate) async fn user_context_middleware(
    State(pool): State<Arc<PgPool>>,
    mut request: Request,
    next: Next,
) -> Response {
    let headers = request.headers();
    let session = match extract_user_from_cookie(headers) {
        Ok(s) => s,
        Err(reason) => {
            tracing::warn!(reason = %reason, "UserContext middleware: no valid session");
            return next.run(request).await;
        },
    };

    let (roles, department) = fetch_user_roles_department(&pool, &session.user_id)
        .await
        .unwrap_or_else(|| (vec!["user".to_owned()], String::new()));

    let is_admin = roles.contains(&"admin".to_owned());
    let ctx = UserContext {
        user_id: session.user_id,
        username: session.username,
        email: session.email,
        roles,
        department,
        is_admin,
        email_verified: false,
    };

    request.extensions_mut().insert(ctx);
    next.run(request).await
}

async fn fetch_user_roles_department(
    pool: &PgPool,
    user_id: &UserId,
) -> Option<(Vec<String>, String)> {
    super::repositories::users::queries::find_user_roles_department(pool, user_id)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, user_id = %user_id, "Failed to fetch user roles");
        })
        .ok()
        .flatten()
}

pub(crate) async fn require_user_middleware(request: Request, next: Next) -> Response {
    let user_ctx = request.extensions().get::<UserContext>().cloned();
    match user_ctx {
        Some(ctx) if !ctx.user_id.as_str().is_empty() => next.run(request).await,
        _ => {
            let uri = request
                .extensions()
                .get::<axum::extract::OriginalUri>()
                .map_or_else(
                    || request.uri().path().to_owned(),
                    |o| o.0.path().to_owned(),
                );
            let redirect_url = format!("/admin/login?redirect={uri}");
            axum::response::Redirect::temporary(&redirect_url).into_response()
        },
    }
}

pub(crate) async fn require_auth_middleware(request: Request, next: Next) -> Response {
    let user_ctx = request.extensions().get::<UserContext>().cloned();
    match user_ctx {
        Some(ctx) if !ctx.user_id.as_str().is_empty() => next.run(request).await,
        _ => (
            StatusCode::UNAUTHORIZED,
            axum::Json(ErrorBody {
                error: "Authentication required".to_owned(),
            }),
        )
            .into_response(),
    }
}

pub(crate) async fn require_admin_middleware(request: Request, next: Next) -> Response {
    let user_ctx = request.extensions().get::<UserContext>().cloned();
    match user_ctx {
        Some(ctx) if ctx.is_admin => next.run(request).await,
        _ => (
            StatusCode::FORBIDDEN,
            axum::Json(ErrorBody {
                error: "Admin access required".to_owned(),
            }),
        )
            .into_response(),
    }
}

/// Restrict non-admin users to the profile page, settings page, and a few
/// account-management endpoints. Other admin routes redirect to /admin/profile.
///
/// Admins pass through unchanged. Anonymous users are handled by
/// `require_user_middleware` which runs after this layer.
pub(crate) async fn non_admin_gate_middleware(request: Request, next: Next) -> Response {
    let path = request
        .extensions()
        .get::<axum::extract::OriginalUri>()
        .map_or_else(
            || request.uri().path().to_owned(),
            |o| o.0.path().to_owned(),
        );
    let user_ctx = request.extensions().get::<UserContext>().cloned();

    let Some(ctx) = user_ctx else {
        return next.run(request).await;
    };
    if ctx.is_admin || ctx.user_id.as_str().is_empty() {
        return next.run(request).await;
    }

    if is_non_admin_allowed_path(&path) {
        next.run(request).await
    } else {
        tracing::warn!(
            path,
            user_id = %ctx.user_id,
            "non-admin user blocked from admin route; redirecting to /admin/profile"
        );
        axum::response::Redirect::to("/admin/profile").into_response()
    }
}

fn is_non_admin_allowed_path(path: &str) -> bool {
    path.starts_with("/admin/profile")
        || path.starts_with("/admin/settings")
        || path.starts_with("/admin/auth/")
        || path.starts_with("/admin/api/")
        || path == "/admin/logout"
        || path == "/admin/login"
        || path == "/admin/register"
        || path == "/admin/add-passkey"
        || path == "/admin/verify-pending"
        || path == "/admin/setup"
        || path == "/admin/demo-register"
        || path == "/admin/"
        || path == "/admin"
}

pub(crate) async fn auth_me_handler(Extension(user_ctx): Extension<UserContext>) -> Response {
    if user_ctx.user_id.as_str().is_empty() {
        return (StatusCode::UNAUTHORIZED, "Not authenticated").into_response();
    }
    axum::Json(AuthMeResponse {
        user_id: user_ctx.user_id,
        username: user_ctx.username,
        email: user_ctx.email,
        roles: user_ctx.roles,
        department: user_ctx.department,
        is_admin: user_ctx.is_admin,
    })
    .into_response()
}
