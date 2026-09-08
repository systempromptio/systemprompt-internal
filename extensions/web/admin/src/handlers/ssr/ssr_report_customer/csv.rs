//! `/admin/reports/customer.csv` — the customer usage report as a download.
//!
//! Scoped to the same organization as the page, then narrowed by any group
//! or project filters. `?dimension=` picks
//! users (default), projects, or models.

use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::response::Response;
use serde::Deserialize;
use sqlx::PgPool;

use crate::error::{AdminError, AdminResult};
use crate::handlers::ssr::csv::CsvBuilder;
use crate::repositories::dashboard_reports::customer;
use crate::repositories::organizations::crud;
use crate::repositories::scope::{ScopeRequest, SubjectScope};
use crate::types::UserContext;
use crate::util::month_range::{MonthQuery, parse_month_range};

#[derive(Debug, Deserialize)]
pub(crate) struct CustomerCsvQuery {
    pub month: Option<String>,
    pub org: Option<String>,
    pub group: Option<String>,
    pub project: Option<String>,
    pub dimension: Option<String>,
}

pub(crate) async fn report_customer_csv(
    Extension(user_ctx): Extension<UserContext>,
    State(pool): State<Arc<PgPool>>,
    Query(query): Query<CustomerCsvQuery>,
) -> AdminResult<Response> {
    if !user_ctx.is_admin {
        return Err(AdminError::Forbidden("Admin access required.".to_owned()));
    }

    let month = parse_month_range(&MonthQuery {
        month: query.month.clone(),
    });
    let request =
        ScopeRequest::from_query(&user_ctx, query.group.as_deref(), query.project.as_deref());
    let scope = crate::repositories::scope::membership::get_subject_scope(&pool, &request).await?;
    let slug = super::resolve_slug(&pool, &user_ctx, query.org.as_deref()).await?;
    let Some(org) = crud::find_organization_by_slug(&pool, &slug).await? else {
        return Err(AdminError::NotFound(format!(
            "No organization with slug '{slug}'."
        )));
    };
    // Why: a group filter may only narrow the organization's membership.
    let members = crud::list_members(&pool, &org.id).await?;
    let scope = SubjectScope::Users(
        members
            .into_iter()
            .map(|member| member.user_id.as_str().to_owned())
            .filter(|id| scope.as_sql().is_none_or(|ids| ids.contains(id)))
            .collect(),
    );

    let dimension = query.dimension.as_deref().unwrap_or("users");
    let filename = format!("usage-{slug}-{}-{dimension}.csv", month.key);

    let csv = match dimension {
        "projects" => projects_csv(
            customer::export_list_customer_month_projects(&pool, &scope, month.from, month.to)
                .await?,
        ),
        "models" => models_csv(
            customer::export_list_customer_month_models(&pool, &scope, month.from, month.to)
                .await?,
        ),
        _ => users_csv(
            customer::export_list_customer_month_users(&pool, &scope, month.from, month.to).await?,
        ),
    };
    Ok(csv.into_response(&filename))
}

fn users_csv(rows: Vec<customer::ExportCustomerUserUsage>) -> CsvBuilder {
    let mut csv = CsvBuilder::new(&[
        "email",
        "display_name",
        "project",
        "requests",
        "input_tokens",
        "output_tokens",
        "reasoning_tokens",
        "total_tokens",
        "distinct_models",
    ]);
    for r in rows {
        csv.row(&[
            &r.email,
            &r.display_name,
            r.project.as_deref().unwrap_or(""),
            &r.requests.to_string(),
            &r.input_tokens.to_string(),
            &r.output_tokens.to_string(),
            &r.reasoning_tokens.to_string(),
            &r.total_tokens.to_string(),
            &r.distinct_models.to_string(),
        ]);
    }
    csv
}

fn projects_csv(rows: Vec<customer::ExportCustomerProjectUsage>) -> CsvBuilder {
    let mut csv = CsvBuilder::new(&[
        "project",
        "members",
        "requests",
        "input_tokens",
        "output_tokens",
        "reasoning_tokens",
        "total_tokens",
    ]);
    for r in rows {
        csv.row(&[
            &r.project,
            &r.members.to_string(),
            &r.requests.to_string(),
            &r.input_tokens.to_string(),
            &r.output_tokens.to_string(),
            &r.reasoning_tokens.to_string(),
            &r.total_tokens.to_string(),
        ]);
    }
    csv
}

fn models_csv(rows: Vec<customer::ExportCustomerModelUsage>) -> CsvBuilder {
    let mut csv = CsvBuilder::new(&[
        "provider",
        "model",
        "requests",
        "input_tokens",
        "output_tokens",
        "cache_read_tokens",
        "reasoning_tokens",
        "total_tokens",
    ]);
    for r in rows {
        csv.row(&[
            &r.provider,
            &r.model,
            &r.requests.to_string(),
            &r.input_tokens.to_string(),
            &r.output_tokens.to_string(),
            &r.cache_read_tokens.to_string(),
            &r.reasoning_tokens.to_string(),
            &r.total_tokens.to_string(),
        ]);
    }
    csv
}
