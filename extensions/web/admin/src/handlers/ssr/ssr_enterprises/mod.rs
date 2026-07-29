//! `/admin/enterprises` — every organization on the instance, and one of them
//! in detail.
//!
//! This is the console's landing page and its highest level of abstraction: a
//! customer, the plan they bought, the seats they filled, and what that
//! contract earned against what their inference cost. Users and departments are
//! not top-level objects here — they are what an enterprise is made of, and
//! they are reached by drilling into one.
//!
//! Both handlers are behind `require_platform_admin_middleware`, so they do not
//! re-check authorisation: a route reachable only through that layer that also
//! guards itself invites the reader to assume the layer is optional.

use std::sync::Arc;

use axum::extract::{Extension, Path, State};
use axum::response::Response;
use sqlx::PgPool;

use crate::error::{AdminError, AdminHtmlResult};
use crate::repositories::organizations;
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};

use super::types::{EnterpriseDetailPageData, EnterpriseView, EnterprisesPageData};

mod view;

/// Modifier the margin tile takes when money is going the wrong way.
const fn margin_variant(margin_microdollars: i64) -> &'static str {
    if margin_microdollars < 0 {
        "stat-card--negative"
    } else {
        ""
    }
}

pub(crate) async fn enterprises_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
) -> AdminHtmlResult<Response> {
    // Why: a failed query must not render as "no customers". This page is the
    // instance's inventory, and an empty one reads as a true statement about
    // the business rather than as a database blip.
    let metrics = organizations::metrics::list_organization_metrics(&pool).await?;

    let mut enterprises: Vec<EnterpriseView> = metrics.iter().map(view::enterprise_view).collect();
    // Least profitable first — the customers costing more than they pay are
    // the reason to open this page.
    enterprises.sort_by_key(|e| e.margin_microdollars);

    // The platform tenant is a row on the page — its spend is real — but it is
    // not a customer, so it must not inflate the count an operator reads as
    // "how many enterprises do we have".
    let customers = enterprises.iter().filter(|e| !e.is_platform).count();
    let total_revenue: i64 = enterprises.iter().map(|e| e.revenue_microdollars).sum();
    let total_cost: i64 = enterprises.iter().map(|e| e.cost_mtd_microdollars).sum();
    let total_margin = total_revenue - total_cost;

    let data = EnterprisesPageData {
        page: "enterprises",
        title: "Enterprises",
        subtitle: "Every organization on this instance, its plan, and what that contract earns \
                   against what it costs to serve.",
        total_enterprises: i64::try_from(customers).unwrap_or(i64::MAX),
        total_seats: enterprises.iter().map(|e| e.seats_used).sum(),
        total_departments: enterprises.iter().map(|e| e.departments).sum(),
        total_requests_30d: enterprises.iter().map(|e| e.requests_30d).sum(),
        enterprises,
        total_revenue_microdollars: total_revenue,
        total_cost_microdollars: total_cost,
        total_margin_microdollars: total_margin,
        margin_variant: margin_variant(total_margin),
    };

    Ok(super::render_typed_page(
        &engine,
        "enterprises",
        &data,
        &user_ctx,
        &mkt_ctx,
    ))
}

pub(crate) async fn enterprise_detail_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Path(slug): Path<String>,
) -> AdminHtmlResult<Response> {
    let Some(metrics) = organizations::metrics::find_organization_metrics(&pool, &slug).await?
    else {
        return Err(AdminError::NotFound(format!("No enterprise with slug '{slug}'.")).into());
    };

    let members: Vec<_> = organizations::crud::list_members(&pool, &metrics.id)
        .await?
        .into_iter()
        .map(view::member_view)
        .collect();

    let departments: Vec<_> =
        organizations::detail::list_organization_departments(&pool, &metrics.id)
            .await?
            .into_iter()
            .map(view::department_view)
            .collect();

    let entitlements: Vec<_> =
        organizations::detail::list_organization_entitlements(&pool, &metrics.slug)
            .await?
            .into_iter()
            .map(view::entitlement_view)
            .collect();

    let models = view::model_views(
        organizations::detail::list_organization_model_usage(&pool, &metrics.id).await?,
    );

    let enterprise = view::enterprise_view(&metrics);
    let data = EnterpriseDetailPageData {
        page: "enterprise-detail",
        title: enterprise.name.clone(),
        margin_variant: margin_variant(enterprise.margin_microdollars),
        enterprise,
        members,
        departments,
        entitlements,
        models,
    };

    Ok(super::render_typed_page(
        &engine,
        "enterprise-detail",
        &data,
        &user_ctx,
        &mkt_ctx,
    ))
}
