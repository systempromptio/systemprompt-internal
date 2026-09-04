//! `/admin/demo/skills` — which skills get used, by whom, and what they cost.

use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::response::Response;
use sqlx::PgPool;

use super::context::DemoSkillsContext;
use super::view::{
    AttributedTotals, SkillCatalogIndex, matrix_view, skill_total_view, user_total_views,
};
use super::{ATTRIBUTION_NOTE, CHART_DAYS, skill_kpi_strip};
use crate::error::{AdminError, AdminHtmlResult};
use crate::handlers::ssr::types::{TabLinkView, daily_count_chart};
use crate::repositories::demo::kpis::{DemoKpis, get_demo_kpis};
use crate::repositories::demo::series::list_skill_daily_series;
use crate::repositories::demo::skill_invocations::list_skill_totals;
use crate::repositories::demo::skill_matrix::list_user_skill_matrix;
use crate::repositories::demo::{DemoFilter, UsageMatrix};
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};

const PAGE_URL: &str = "/admin/demo/skills";

// Why: the headline strip is pinned above the tabs, so both tabs render the
// same KPI cards; the split is only in the body beneath them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillsTab {
    BySkill,
    ByUser,
}

impl SkillsTab {
    // Why: an unrecognised ?tab= lands on the default view rather than an
    // error. A stale bookmark should still show the page.
    fn from_slug(slug: Option<&str>) -> Self {
        match slug {
            Some("users") => Self::ByUser,
            _ => Self::BySkill,
        }
    }

    fn links(self) -> Vec<TabLinkView> {
        // Why: no count pills. Both numbers a pill could carry — distinct
        // skills, distinct users — are already KPI cards directly above.
        [
            (Self::BySkill, "skills", "By skill", PAGE_URL.to_owned()),
            (
                Self::ByUser,
                "users",
                "By user",
                format!("{PAGE_URL}?tab=users"),
            ),
        ]
        .into_iter()
        .map(|(tab, slug, label, href)| TabLinkView {
            slug,
            label,
            href,
            is_active: tab == self,
            count: None,
        })
        .collect()
    }
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct SkillsQuery {
    tab: Option<String>,
}

async fn build_page_json(pool: &PgPool, tab: SkillsTab) -> DemoSkillsContext {
    let filter = DemoFilter::all_users();
    let (kpis, totals, series, matrix) = tokio::join!(
        get_demo_kpis(pool, &filter),
        list_skill_totals(pool, &filter),
        list_skill_daily_series(pool, &filter, CHART_DAYS),
        list_user_skill_matrix(pool, &filter),
    );
    let kpis = kpis.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "demo kpi query failed");
        DemoKpis::default()
    });
    let totals = totals.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "skill totals query failed");
        Vec::new()
    });
    let series = series.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "skill daily series query failed");
        Vec::new()
    });
    let matrix = matrix.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "skill matrix query failed");
        UsageMatrix::default()
    });

    let usage = totals
        .iter()
        .fold(AttributedTotals::default(), |mut acc, t| {
            acc.add(t.total_tokens, t.cost_microdollars);
            acc
        });
    // Why: every query runs on both tabs because the pinned KPI strip is fed
    // by all of them — distinct skills from the totals, distinct users from
    // the matrix, tokens and cost from the totals. Only the body is split.
    // Why: read the on-disk catalog so a recorded name that no longer names a
    // skill can be marked retired instead of passing as live. A failure here
    // costs only the badge, so it degrades to "nothing is judged".
    let catalog = crate::handlers::shared::get_services_path().ok().map_or_else(
        SkillCatalogIndex::default,
        |path| {
            let plugins = crate::repositories::marketplace::plugins::list_plugin_catalog(&path)
                .unwrap_or_default()
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<_>>();
            let skills = crate::repositories::marketplace::plugins::list_skill_catalog(&path)
                .unwrap_or_default();
            SkillCatalogIndex::new(&plugins, &skills)
        },
    );
    let skills: Vec<_> = totals
        .iter()
        .map(|t| skill_total_view(t, &catalog))
        .collect();
    let distinct_skills = skills.len() as i64;
    let distinct_users = matrix.rows.len() as i64;
    let user_totals = user_total_views(&matrix);
    let is_by_skill = tab == SkillsTab::BySkill;
    let is_by_user = tab == SkillsTab::ByUser;

    DemoSkillsContext {
        page: "demo-skills",
        title: "Skill Adoption",
        subtitle: "Skill invocations recorded by the Claude Code hooks, with the \
                   AI usage attributed to each one.",
        kpis: skill_kpi_strip(&kpis, distinct_skills, distinct_users, &usage),
        chart: daily_count_chart(
            &series,
            "Skill invocations, last 14 days",
            "accent",
            "No skill invocations in this window.",
        ),
        tabs: tab.links(),
        is_by_skill,
        is_by_user,
        has_skills: !skills.is_empty(),
        skills,
        matrix: matrix_view(&matrix),
        has_user_totals: !user_totals.is_empty(),
        user_totals,
        attribution_note: ATTRIBUTION_NOTE,
    }
}

pub(crate) async fn demo_skills_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Query(query): Query<SkillsQuery>,
) -> AdminHtmlResult<Response> {
    if !user_ctx.is_admin {
        return Err(AdminError::Forbidden("Admin access required.".to_owned()).into());
    }
    let payload = build_page_json(&pool, SkillsTab::from_slug(query.tab.as_deref())).await;
    Ok(super::super::render_typed_page(
        &engine,
        "demo-skills",
        &payload,
        &user_ctx,
        &mkt_ctx,
    ))
}
