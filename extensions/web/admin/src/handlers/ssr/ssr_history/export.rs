//! The history listing as the export surface reads it: the rows behind a
//! file, and the export view the page hands its dialog.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;

use crate::error::AdminError;
use crate::export::model::ExportWindow;
use crate::export::view::TranscriptSource;
use crate::repositories::analytics::conversations::{
    HistoryFilter, HistoryItem, list_history_items,
};
use crate::types::UserContext;

use super::{HistoryQuery, HistoryView};

// Why: the export is the listing the reader sees — their own scope, the same
// search — through the page's own query type. The dialog's resolved window
// bounds the rows; this page has no window of its own, so without one the
// export is all time.
// What an export asks of the history listing, as opposed to who is asking.
pub(crate) struct ExportRequest {
    pub query: HistoryQuery,
    pub view: HistoryView,
    pub window: Option<ExportWindow>,
    pub limit: i64,
}

pub(crate) async fn export_rows(
    pool: &PgPool,
    user_ctx: &UserContext,
    request: ExportRequest,
) -> Result<(Vec<HistoryItem>, i64), AdminError> {
    let ExportRequest {
        query,
        view,
        window,
        limit,
    } = request;
    let scope = view.scope(user_ctx);
    let target = query
        .user_id
        .as_ref()
        .filter(|u| !u.as_str().trim().is_empty());
    let scope_ids = match target {
        Some(target_id) if scope.may_view(target_id) => Some(vec![target_id.as_str().to_owned()]),
        Some(_) => {
            return Err(AdminError::Forbidden(
                "You may only export conversation history within your own scope.".to_owned(),
            ));
        },
        None => scope.user_ids(),
    };
    Ok(list_history_items(
        pool,
        HistoryFilter {
            scope_user_ids: scope_ids.as_deref(),
            search: query.q.as_deref(),
            include_side_calls: query.show_side(),
            since: window.map(|w| w.from),
            until: window.map(|w| w.to),
        },
        limit,
        0,
    )
    .await?)
}

// Why: the export opens on the listing the reader is looking at — the same
// search, user and side calls — and offers the full record of every
// conversation it selects. The dialog's retained contract tops out at a year,
// and this page is all time, so the dialog opens on its widest window.
pub(super) fn export_view(query: &HistoryQuery, view: HistoryView) -> crate::export::ExportView {
    let pairs = [
        ("q", query.q.as_deref()),
        ("user_id", query.user_id.as_ref().map(UserId::as_str)),
        ("side", query.side.as_deref()),
        ("days", Some("365")),
    ];
    let source = match view {
        HistoryView::Org => TranscriptSource::Conversations,
        HistoryView::Own => TranscriptSource::History,
    };
    crate::export::ExportView::single(view.dataset(), &crate::export::view::query_string(&pairs))
        .with_transcripts(source)
}
