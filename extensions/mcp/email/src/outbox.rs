//! The outbound reconciliation ledger.
//!
//! Every send writes a `queued` row before it touches the relay and settles it
//! afterwards. That ordering is the point: if the process dies between the SMTP
//! accept and the Odoo write, the row is the only evidence that a real email
//! left the building, and `email_outbox_unlogged_idx` finds it.
//!
//! All four functions are best-effort in the same sense as
//! `systemprompt_mcp_shared::record_mcp_access`: a ledger write that fails is
//! logged and swallowed, because refusing to send a human-approved email over a
//! bookkeeping error is the worse failure. The tradeoff is deliberate and it is
//! the reason `claim` returns `()` rather than a guard.

use systemprompt::database::DbPool;
use systemprompt::identifiers::UserId;

/// Everything known about a send before it is attempted.
#[derive(Debug)]
// Why: `rfc5322_id`, not `message_id`. The RFC5322 Message-ID is the mail
// protocol's own identifier for a message on the wire; it is not this
// platform's `MessageId`, which names a row in the messages table. Giving it
// that name would invite someone to reach for the typed id and be wrong.
pub struct OutboxEntry<'a> {
    pub rfc5322_id: &'a str,
    pub user_id: &'a UserId,
    pub recipients: &'a [String],
    pub subject: &'a str,
    pub res_model: Option<&'a str>,
    pub res_id: Option<i64>,
}

// Why: Ensures the ledger table exists.
//
// This server is its own process against the tenant database and cannot assume
// the host binary migrated on its behalf — the same reasoning, and the same
// `IF NOT EXISTS` DDL, as the knowledge bank's `ensure_installed`.
//
// Propagates the sqlx error, because a server with nowhere to record sends
// should not start.
pub async fn ensure_installed(pool: &DbPool) -> Result<(), sqlx::Error> {
    let Some(pg_pool) = pool.pool() else {
        tracing::warn!("no Postgres pool available; skipping email_outbox install");
        return Ok(());
    };
    sqlx::raw_sql(include_str!("../schema/01_email_outbox.sql"))
        .execute(pg_pool.as_ref())
        .await?;
    Ok(())
}

// Why: Records the intent to send, before the relay is contacted.
pub async fn claim(pool: &DbPool, entry: &OutboxEntry<'_>) {
    let Some(pg_pool) = pool.pool() else {
        tracing::warn!("no Postgres pool available to claim an outbox row");
        return;
    };
    let result = sqlx::query!(
        "INSERT INTO email_outbox (message_id, user_id, recipients, subject, res_model, res_id, \
         status) VALUES ($1, $2, $3, $4, $5, $6, 'queued') ON CONFLICT (message_id) DO NOTHING",
        entry.rfc5322_id,
        entry.user_id.as_str(),
        entry.recipients,
        entry.subject,
        entry.res_model,
        entry.res_id,
    )
    .execute(pg_pool.as_ref())
    .await;

    if let Err(e) = result {
        tracing::warn!(error = %e, rfc5322_id = entry.rfc5322_id, "could not claim an outbox row (non-fatal)");
    }
}

pub async fn mark_sent(pool: &DbPool, rfc5322_id: &str) {
    let Some(pg_pool) = pool.pool() else {
        return warn_no_pool(rfc5322_id);
    };
    let result = sqlx::query!(
        "UPDATE email_outbox SET status = 'sent', sent_at = NOW() WHERE message_id = $1",
        rfc5322_id
    )
    .execute(pg_pool.as_ref())
    .await;
    warn_on_error(result, rfc5322_id);
}

pub async fn mark_failed(pool: &DbPool, rfc5322_id: &str, error: &str) {
    let Some(pg_pool) = pool.pool() else {
        return warn_no_pool(rfc5322_id);
    };
    let result = sqlx::query!(
        "UPDATE email_outbox SET status = 'failed', error = $2 WHERE message_id = $1",
        rfc5322_id,
        error
    )
    .execute(pg_pool.as_ref())
    .await;
    warn_on_error(result, rfc5322_id);
}

pub async fn mark_logged(pool: &DbPool, rfc5322_id: &str, odoo_message_id: i64) {
    let Some(pg_pool) = pool.pool() else {
        return warn_no_pool(rfc5322_id);
    };
    let result = sqlx::query!(
        "UPDATE email_outbox SET status = 'logged', logged_at = NOW(), odoo_message_id = $2 WHERE \
         message_id = $1",
        rfc5322_id,
        odoo_message_id
    )
    .execute(pg_pool.as_ref())
    .await;
    warn_on_error(result, rfc5322_id);
}

// Why: the row stays `sent`, not `failed` — the mail really did go out. Only
// the chatter write is missing, which is what the reconciliation index looks
// for.
pub async fn mark_log_failed(pool: &DbPool, rfc5322_id: &str, error: &str) {
    let Some(pg_pool) = pool.pool() else {
        return warn_no_pool(rfc5322_id);
    };
    let result = sqlx::query!(
        "UPDATE email_outbox SET error = $2 WHERE message_id = $1",
        rfc5322_id,
        error
    )
    .execute(pg_pool.as_ref())
    .await;
    warn_on_error(result, rfc5322_id);
}

fn warn_no_pool(rfc5322_id: &str) {
    tracing::warn!(
        rfc5322_id,
        "no Postgres pool available to update an outbox row"
    );
}

fn warn_on_error(result: Result<sqlx::postgres::PgQueryResult, sqlx::Error>, rfc5322_id: &str) {
    if let Err(e) = result {
        tracing::warn!(error = %e, rfc5322_id, "could not update an outbox row (non-fatal)");
    }
}
