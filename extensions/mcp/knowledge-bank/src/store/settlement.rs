//! The decide-side transitions on `knowledge_documents`: claiming a document
//! for apply, recording a denial or expiry, writing what landed, and the two
//! queries the reconcile job drains.

use chrono::{DateTime, Utc};
use systemprompt::identifiers::UserId;
use uuid::Uuid;

use super::KnowledgeStore;
use super::rows::SettleableRow;
use crate::error::KnowledgeBankError;
use crate::proposal::DocumentStatus;
use crate::proposal::apply::AppliedOutcome;

pub const MAX_APPLY_ATTEMPTS: i32 = 5;

// Why: an `approved` document older than this with nothing applied is a
// worker that died mid-apply; the reconcile job takes it back.
const STALE_APPROVED_MINUTES: i64 = 5;

impl KnowledgeStore {
    pub async fn claim_for_apply(
        &self,
        id: Uuid,
        from: DocumentStatus,
        approver: &UserId,
        decided_at: DateTime<Utc>,
    ) -> Result<bool, KnowledgeBankError> {
        let pool = self.write()?;
        let updated = sqlx::query!(
            r#"
            UPDATE knowledge_documents
            SET status = 'approved', decided_by = $3, decided_at = $4,
                apply_attempts = apply_attempts + 1, next_attempt_at = NULL
            WHERE id = $1 AND status = $2
            "#,
            id,
            from.as_str(),
            approver.as_str(),
            decided_at
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))?;
        Ok(updated.rows_affected() == 1)
    }

    pub async fn set_decided(
        &self,
        id: Uuid,
        to: DocumentStatus,
        approver: &UserId,
        decided_at: DateTime<Utc>,
    ) -> Result<bool, KnowledgeBankError> {
        let pool = self.write()?;
        let updated = sqlx::query!(
            r#"
            UPDATE knowledge_documents
            SET status = $2, decided_by = $3, decided_at = $4
            WHERE id = $1 AND status = 'proposed'
            "#,
            id,
            to.as_str(),
            approver.as_str(),
            decided_at
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))?;
        Ok(updated.rows_affected() == 1)
    }

    // Why: exponential backoff in minutes, capped by MAX_APPLY_ATTEMPTS in the
    // retry query rather than here, so a manual re-approve always runs.
    pub async fn set_applied(
        &self,
        id: Uuid,
        applied: Option<&AppliedOutcome>,
        error: Option<&str>,
    ) -> Result<(), KnowledgeBankError> {
        let pool = self.write()?;
        let json = applied.map(serde_json::to_value).transpose()?;
        let status = if error.is_some() {
            DocumentStatus::Failed
        } else {
            DocumentStatus::Applied
        };
        sqlx::query!(
            r#"
            UPDATE knowledge_documents
            SET status = $2, applied = COALESCE($3, applied), proposal_error = $4,
                next_attempt_at = CASE WHEN $4::text IS NULL THEN NULL
                    ELSE now() + make_interval(mins => (2 ^ LEAST(apply_attempts, 8))::int) END
            WHERE id = $1 AND status = 'approved'
            "#,
            id,
            status.as_str(),
            json,
            error
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))?;
        Ok(())
    }

    pub async fn list_settleable(
        &self,
        limit: i64,
    ) -> Result<Vec<SettleableRow>, KnowledgeBankError> {
        let pool = self.read()?;
        sqlx::query_as!(
            SettleableRow,
            r#"
            SELECT d.id AS "document_id!", d.proposal_call_id AS "call_id!"
            FROM knowledge_documents d
            JOIN approval_requests ar ON ar.call_id = d.proposal_call_id
            WHERE d.status = 'proposed' AND ar.status <> 'pending'
            ORDER BY ar.decided_at NULLS LAST, d.created_at
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))
    }

    pub async fn list_retry_due(
        &self,
        limit: i64,
    ) -> Result<Vec<SettleableRow>, KnowledgeBankError> {
        let pool = self.read()?;
        sqlx::query_as!(
            SettleableRow,
            r#"
            SELECT id AS "document_id!", proposal_call_id AS "call_id!"
            FROM knowledge_documents
            WHERE proposal_call_id IS NOT NULL
              AND (
                (status = 'failed' AND next_attempt_at <= now() AND apply_attempts < $2)
                OR (status = 'approved' AND decided_at < now() - make_interval(mins => $3))
              )
            ORDER BY created_at
            LIMIT $1
            "#,
            limit,
            MAX_APPLY_ATTEMPTS,
            STALE_APPROVED_MINUTES as i32
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))
    }
}
