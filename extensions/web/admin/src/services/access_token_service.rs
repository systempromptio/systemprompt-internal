//! Personal access token lifecycle.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt::identifiers::UserId;

use crate::error::{AdminError, AdminResult};
use crate::repositories::access_tokens::{self, IssuedApiKey};

pub(crate) async fn issue_pat(
    pool: &PgPool,
    user_id: &UserId,
    name: &str,
    expires_at: Option<DateTime<Utc>>,
) -> AdminResult<IssuedApiKey> {
    let issued = access_tokens::issue_api_key(pool, user_id, name, expires_at).await?;
    Ok(issued)
}

pub(crate) async fn revoke_pat(pool: &PgPool, user_id: &UserId, id: &str) -> AdminResult<()> {
    let revoked = access_tokens::revoke_api_key(pool, user_id, id).await?;
    if !revoked {
        return Err(AdminError::NotFound("PAT not found".to_owned()));
    }
    Ok(())
}
