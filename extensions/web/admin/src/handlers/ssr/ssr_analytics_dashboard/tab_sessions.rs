//! View assembly for the Sessions tab.
//!
//! Every figure is what the gateway metered for one conversation — the
//! `conversation_facts` rollup — so the page can quote cost and tokens as
//! measurements rather than a client's own estimate.

use crate::handlers::ssr::entity_urls::{context_detail_url, session_detail_url};
use crate::handlers::ssr::format::format_cost;
use crate::handlers::ssr::list_view::PageWindow;
use crate::repositories::analytics::site::sessions::SessionCostRow;

use super::context::{KpiTile, SessionCostRowView, SessionsTabView};
use super::tab_models::{per, share};
use super::view::{compact, format_date};
use super::{AnalyticsDashboardQuery, urls};
use crate::handlers::ssr::list_view::DEFAULT_PAGE_SIZE;

pub(super) struct SessionsInput<'a> {
    pub rows: &'a [SessionCostRow],
    pub total_rows: i64,
    pub page: i64,
}

pub(super) fn sessions_tab(
    input: &SessionsInput<'_>,
    query: &AnalyticsDashboardQuery,
) -> SessionsTabView {
    let max = input
        .rows
        .iter()
        .map(|r| r.total_cost_microdollars)
        .max()
        .unwrap_or(0);
    let views: Vec<SessionCostRowView> = input.rows.iter().map(|r| row(r, max)).collect();

    let pagination = (input.total_rows > DEFAULT_PAGE_SIZE).then(|| {
        urls::build_pagination(
            query,
            PageWindow::new(
                input.page,
                DEFAULT_PAGE_SIZE,
                input.total_rows,
                i64::try_from(input.rows.len()).unwrap_or(DEFAULT_PAGE_SIZE),
                "sessions",
            ),
        )
    });

    SessionsTabView {
        kpis: kpis(input),
        session_count: input.total_rows,
        has_rows: !views.is_empty(),
        rows: views,
        pagination,
    }
}

fn row(r: &SessionCostRow, max: i64) -> SessionCostRowView {
    SessionCostRowView {
        context_short: short_id(r.context_id.as_str()),
        model_display: r.model.clone().unwrap_or_else(|| "—".to_owned()),
        cost_display: format_cost(r.total_cost_microdollars),
        share_pct: share(r.total_cost_microdollars, max),
        requests_display: compact(r.request_count),
        input_display: compact(r.input_tokens),
        output_display: compact(r.output_tokens),
        cache_display: compact(r.cache_read_tokens),
        updated_display: format_date(r.updated_at),
        detail_url: context_detail_url(&r.context_id),
        session_url: r.session_id.as_ref().map(session_detail_url),
        user_url: format!("/admin/users/{}", urlencoding::encode(r.user_id.as_str())),
        context_id: r.context_id.clone(),
        user_id: r.user_id.clone(),
    }
}

// Why: a conversation id is a uuid nobody reads in full; the head is enough
// to recognise a row, and the cell links to the conversation itself.
fn short_id(id: &str) -> String {
    id.chars().take(12).collect()
}

fn kpis(input: &SessionsInput<'_>) -> Vec<KpiTile> {
    let cost: i64 = input.rows.iter().map(|r| r.total_cost_microdollars).sum();
    let requests: i64 = input.rows.iter().map(|r| r.request_count).sum();
    let cache: i64 = input.rows.iter().map(|r| r.cache_read_tokens).sum();
    let input_tokens: i64 = input.rows.iter().map(|r| r.input_tokens).sum();
    let prompt_tokens = cache + input_tokens;
    let cache_share = if prompt_tokens > 0 {
        cache.saturating_mul(100) / prompt_tokens
    } else {
        0
    };
    vec![
        KpiTile {
            label: "Conversations".to_owned(),
            value: input.total_rows.to_string(),
            note: "with a gateway request in this window".to_owned(),
            tone: "accent",
        },
        KpiTile {
            label: "Cost on this page".to_owned(),
            value: format_cost(cost),
            note: format!(
                "{} per conversation · gateway-metered",
                format_cost(per(cost, i64::try_from(input.rows.len()).unwrap_or(0)))
            ),
            tone: "ok",
        },
        KpiTile {
            label: "Requests on this page".to_owned(),
            value: compact(requests),
            note: format!(
                "{} per conversation",
                per(requests, i64::try_from(input.rows.len()).unwrap_or(0))
            ),
            tone: "warn",
        },
        KpiTile {
            label: "Served from cache".to_owned(),
            value: format!("{cache_share}%"),
            note: format!(
                "{} of {} prompt tokens",
                compact(cache),
                compact(cache + input_tokens)
            ),
            tone: "ok",
        },
    ]
}
