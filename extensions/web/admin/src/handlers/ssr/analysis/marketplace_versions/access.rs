//! Who may read which marketplace's version history.

use sqlx::PgPool;
use systemprompt::identifiers::MarketplaceId;

use crate::error::{AdminError, AdminHtmlResult, AdminResult};
use crate::repositories::analysis::marketplace_versions::{
    MarketplaceVersionMetricsRow, VersionWindow, list_marketplace_version_metrics,
};
use crate::types::UserContext;

// Why: a console seat sees every marketplace's history; a caller without
// one has no version history to look at.
pub(crate) fn require_versions_reader(user: &UserContext) -> AdminResult<()> {
    if !user.has_scoped_console() {
        return Err(AdminError::Forbidden(
            "Console access required for version history".to_owned(),
        ));
    }
    Ok(())
}

// Why: the per-marketplace read check the version pages and exports share.
// This instance has no marketplace participation tier, so every console
// seat reads every marketplace and no one else reads any; a caller refused
// here gets the same answer as for a marketplace that was never recorded.
pub(crate) const fn may_read_marketplace(
    user: &UserContext,
    _marketplace_id: &MarketplaceId,
) -> bool {
    user.is_console
}

// Why: a caller who may not read the marketplace gets the same not-found as
// a marketplace that was never recorded.
pub(super) async fn readable_versions(
    pool: &PgPool,
    window: VersionWindow,
    marketplace_id: &MarketplaceId,
    user: &UserContext,
) -> AdminHtmlResult<Vec<MarketplaceVersionMetricsRow>> {
    let rows = if may_read_marketplace(user, marketplace_id) {
        list_marketplace_version_metrics(pool, window, marketplace_id).await?
    } else {
        Vec::new()
    };
    if rows.is_empty() {
        return Err(AdminError::NotFound(format!(
            "No version of marketplace '{marketplace_id}' has been recorded"
        ))
        .into());
    }
    Ok(rows)
}
