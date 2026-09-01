//! The per-action ledger in `knowledge_odoo_projection`.
//!
//! Claim before the Odoo call, finish after it. A claim that finds the row
//! already `done` hands back what was written so the caller reuses it; that is
//! what makes a retried proposal idempotent against a lead that was created
//! just before the process died.

use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct LedgerKey {
    pub document_id: Uuid,
    pub revision: i32,
    pub action_index: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct NewProjection<'a> {
    pub key: LedgerKey,
    pub kind: &'a str,
    pub res_model: &'a str,
    pub rfc5322_id: &'a str,
    pub applied_by: &'a str,
    pub odoo_login: &'a str,
}

/// What a claim found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claim {
    Open,
    Done {
        res_id: Option<i64>,
        odoo_message_id: Option<i64>,
    },
    Excluded,
}

pub async fn claim_action(pool: &PgPool, row: &NewProjection<'_>) -> Result<Claim, sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO knowledge_odoo_projection
            (document_id, revision, action_index, kind, res_model, rfc5322_id, applied_by, odoo_login)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (document_id, revision, action_index) DO NOTHING
        "#,
        row.key.document_id,
        row.key.revision,
        row.key.action_index,
        row.kind,
        row.res_model,
        row.rfc5322_id,
        row.applied_by,
        row.odoo_login,
    )
    .execute(pool)
    .await?;

    let existing = sqlx::query!(
        r#"
        SELECT status, res_id, odoo_message_id
        FROM knowledge_odoo_projection
        WHERE document_id = $1 AND revision = $2 AND action_index = $3
        "#,
        row.key.document_id,
        row.key.revision,
        row.key.action_index,
    )
    .fetch_one(pool)
    .await?;

    Ok(match existing.status.as_str() {
        "done" => Claim::Done {
            res_id: existing.res_id,
            odoo_message_id: existing.odoo_message_id,
        },
        "excluded" => Claim::Excluded,
        _ => Claim::Open,
    })
}

pub async fn finish_action(
    pool: &PgPool,
    key: LedgerKey,
    res_id: Option<i64>,
    odoo_message_id: Option<i64>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE knowledge_odoo_projection
        SET status = 'done', res_id = $4, odoo_message_id = $5, error = NULL, applied_at = now()
        WHERE document_id = $1 AND revision = $2 AND action_index = $3
        "#,
        key.document_id,
        key.revision,
        key.action_index,
        res_id,
        odoo_message_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fail_action(pool: &PgPool, key: LedgerKey, error: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE knowledge_odoo_projection
        SET status = 'failed', error = $4
        WHERE document_id = $1 AND revision = $2 AND action_index = $3
        "#,
        key.document_id,
        key.revision,
        key.action_index,
        error,
    )
    .execute(pool)
    .await?;
    Ok(())
}

// Why: an exclusion is written to the ledger, not carried in memory, so a
// retry that knows nothing about the original decision still honours it.
pub async fn mark_excluded(pool: &PgPool, row: &NewProjection<'_>) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO knowledge_odoo_projection
            (document_id, revision, action_index, kind, res_model, rfc5322_id, applied_by, odoo_login, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'excluded')
        ON CONFLICT (document_id, revision, action_index)
            DO UPDATE SET status = 'excluded'
            WHERE knowledge_odoo_projection.status <> 'done'
        "#,
        row.key.document_id,
        row.key.revision,
        row.key.action_index,
        row.kind,
        row.res_model,
        row.rfc5322_id,
        row.applied_by,
        row.odoo_login,
    )
    .execute(pool)
    .await?;
    Ok(())
}
