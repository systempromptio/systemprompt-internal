//! Device enrolment and personal access token lifecycle.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt::identifiers::UserId;

use crate::error::{AdminError, AdminResult};
use crate::repositories::bridge::{self, EnrollDeviceParams, EnrolledDevice, IssuedApiKey};
use crate::repositories::config::security_policy::{SecurityPolicy, bound_pat_expiry};

// Why: both minting paths write a `user_api_keys` row, so both go through the
// policy — a device enrolment is a token with a hostname attached.
fn policy_bounded(expires_at: Option<DateTime<Utc>>) -> AdminResult<DateTime<Utc>> {
    bound_pat_expiry(expires_at, Utc::now(), SecurityPolicy::get())
        .map_err(|e| AdminError::BadRequest(e.to_string()))
}

pub(crate) struct EnrollDeviceInput<'a> {
    pub name: &'a str,
    pub platform: &'a str,
    pub hostname: &'a str,
    pub expires_at: Option<DateTime<Utc>>,
}

pub(crate) async fn enroll_device(
    pool: &PgPool,
    user_id: &UserId,
    req: EnrollDeviceInput<'_>,
) -> AdminResult<EnrolledDevice> {
    let expires_at = policy_bounded(req.expires_at)?;
    let enrolled = bridge::enroll_device(
        pool,
        user_id,
        EnrollDeviceParams {
            name: req.name,
            platform: req.platform,
            hostname: req.hostname,
            expires_at: Some(expires_at),
        },
    )
    .await?;
    Ok(enrolled)
}

pub(crate) async fn issue_pat(
    pool: &PgPool,
    user_id: &UserId,
    name: &str,
    expires_at: Option<DateTime<Utc>>,
) -> AdminResult<IssuedApiKey> {
    let expires_at = policy_bounded(expires_at)?;
    let issued = bridge::issue_api_key(pool, user_id, name, Some(expires_at)).await?;
    Ok(issued)
}

pub(crate) async fn revoke_pat(pool: &PgPool, user_id: &UserId, id: &str) -> AdminResult<()> {
    let revoked = bridge::revoke_api_key(pool, user_id, id).await?;
    if !revoked {
        return Err(AdminError::NotFound("PAT not found".to_owned()));
    }
    Ok(())
}

pub(crate) async fn revoke_device_cert(
    pool: &PgPool,
    user_id: &UserId,
    id: &str,
) -> AdminResult<()> {
    let revoked = bridge::revoke_device_cert(pool, user_id, id).await?;
    if !revoked {
        return Err(AdminError::NotFound("cert not found".to_owned()));
    }
    Ok(())
}
