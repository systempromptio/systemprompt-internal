//! Departments and Access tokens SSR pages.
//!
//! Three admin-only page handlers: the department roster, a single department
//! detail (members + token/cost rollup + top tools), and the access-token
//! console. View-model assembly lives in the `departments` / `access_tokens`
//! children.

use std::sync::Arc;

use axum::extract::{Extension, Path, State};
use axum::response::Response;
use sqlx::PgPool;

use crate::error::{AdminError, AdminHtmlError, AdminHtmlResult};
use crate::repositories;
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};

use super::ssr_helpers::render_typed_page;

mod access_tokens;
mod departments;

use access_tokens::{
    ManagementAccessTokensPageData, build_token_rows, compute_owner_rowspans, load_access_tokens,
    load_token_user_options,
};
use departments::{DepartmentDetailPageData, DepartmentsPageData, sum_member_totals, url_escape};

fn forbidden() -> AdminHtmlError {
    AdminError::Forbidden("Admin access required.".to_owned()).into()
}

pub(crate) async fn management_departments_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
) -> AdminHtmlResult<Response> {
    if !user_ctx.is_admin {
        return Err(forbidden());
    }

    let departments = repositories::departments::list_departments(&pool)
        .await
        .unwrap_or_default();

    let data = DepartmentsPageData {
        page: "management-departments",
        title: "Departments",
        departments,
    };

    Ok(render_typed_page(
        &engine,
        "management-departments",
        &data,
        &user_ctx,
        &mkt_ctx,
    ))
}

pub(crate) async fn management_access_tokens_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
) -> AdminHtmlResult<Response> {
    if !user_ctx.is_admin {
        return Err(forbidden());
    }

    let rows = load_access_tokens(&pool).await;

    let (mut tokens, counts) = build_token_rows(rows);
    compute_owner_rowspans(&mut tokens);

    let user_options = load_token_user_options(&pool).await;

    let department_options: Vec<String> = repositories::departments::list_departments(&pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|d| d.name)
        .collect();

    let data = ManagementAccessTokensPageData {
        page: "tokens",
        title: "Access tokens",
        tokens,
        total: counts.total,
        active: counts.active,
        expiring_soon: counts.expiring_soon,
        user_options,
        department_options,
    };
    Ok(render_typed_page(
        &engine,
        "management-access-tokens",
        &data,
        &user_ctx,
        &mkt_ctx,
    ))
}

pub(crate) async fn management_department_detail_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Path(id): Path<String>,
) -> AdminHtmlResult<Response> {
    if !user_ctx.is_admin {
        return Err(forbidden());
    }

    let Some(department) = repositories::departments::find_department(&pool, &id).await? else {
        return Err(AdminError::NotFound("Department not found".to_owned()).into());
    };

    let members = repositories::departments::list_department_members(&pool, &department.name)
        .await
        .unwrap_or_default();
    let member_count = members.len() as i64;

    let top_tools =
        repositories::departments::list_department_top_tools(&pool, &department.name, 10)
            .await
            .unwrap_or_default();

    let totals = sum_member_totals(&members);

    let assignments_url = format!(
        "/admin/access/matrix?department={}",
        url_escape(&department.name)
    );

    let title = format!("Department · {}", department.name);
    let data = DepartmentDetailPageData {
        page: "management-department-detail",
        title,
        department,
        members,
        member_count,
        assignments_url,
        top_tools,
        total_input_tokens: totals.input_tokens,
        total_output_tokens: totals.output_tokens,
        total_requests: totals.requests,
        total_cost_microdollars: totals.cost_microdollars,
    };

    Ok(render_typed_page(
        &engine,
        "management-department-detail",
        &data,
        &user_ctx,
        &mkt_ctx,
    ))
}
