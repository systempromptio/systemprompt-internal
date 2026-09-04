//! The tab strip shared by every SSR page that splits its body into views.

use serde::Serialize;

/// One link in an `sp-tabs` strip. Each tab is a plain GET against the page's
/// own URL, so a view is bookmarkable and only the active tab's queries run.
#[derive(Debug, Serialize)]
pub(crate) struct TabLinkView {
    pub slug: &'static str,
    pub label: &'static str,
    pub href: String,
    pub is_active: bool,
    // Why: Shown as a count pill next to the label. A tab omits it when the
    // number the reader wants is already in the body it leads to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}
