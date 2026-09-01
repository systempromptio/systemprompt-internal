//! The proposal pipeline's reads and the propose-side writes on
//! `knowledge_documents`; the decide-side transitions live in
//! [`super::settlement`].
//!
//! Every state transition is a compare-and-set on `status`, so two workers —
//! the tool applying inline and the reconcile job a second later — cannot
//! both claim the same document. The JSON columns cross the SQL boundary as
//! `serde_json::Value` and are typed the moment they leave it.

use uuid::Uuid;

use super::rows::{FeedDocumentRow, ProposalDocument, ProposalDocumentRow};
use super::{KnowledgeStore, MAX_LIST_LIMIT};
use crate::error::KnowledgeBankError;
use crate::proposal::{DocumentStatus, Proposal};

/// What the feed can be narrowed by.
#[derive(Debug, Clone, Default)]
pub struct FeedFilter {
    pub status: Option<DocumentStatus>,
    pub query: Option<String>,
    pub limit: Option<i64>,
}

impl KnowledgeStore {
    pub async fn list_feed(
        &self,
        filter: &FeedFilter,
    ) -> Result<Vec<ProposalDocument>, KnowledgeBankError> {
        let pool = self.read()?;
        let limit = filter
            .limit
            .unwrap_or(MAX_LIST_LIMIT)
            .clamp(1, MAX_LIST_LIMIT);
        let status = filter.status.map(DocumentStatus::as_str);
        let pattern = filter
            .query
            .as_deref()
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .map(super::like_pattern);
        let rows = sqlx::query_as!(
            FeedDocumentRow,
            r#"
            SELECT id, title, created_at, category, status, metadata, structured,
                   proposal, proposal_revision, proposal_call_id, proposal_error, skip_reason,
                   apply_attempts, applied, decided_by, decided_at
            FROM knowledge_documents
            WHERE source = 'email'
              AND ($1::text IS NULL OR status = $1)
              AND ($2::text IS NULL OR title ILIKE $2 ESCAPE '\' OR metadata->>'from' ILIKE $2 ESCAPE '\')
            ORDER BY created_at DESC
            LIMIT $3
            "#,
            status,
            pattern,
            limit
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))?;
        rows.into_iter().map(ProposalDocument::try_from).collect()
    }

    pub async fn find_proposal_document(
        &self,
        id: Uuid,
    ) -> Result<Option<ProposalDocument>, KnowledgeBankError> {
        let pool = self.read()?;
        let row = sqlx::query_as!(
            ProposalDocumentRow,
            r#"
            SELECT id, title, created_at, category, status, metadata, structured, content,
                   proposal, proposal_revision, proposal_call_id, proposal_error, skip_reason,
                   apply_attempts, applied, decided_by, decided_at
            FROM knowledge_documents
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))?;
        row.map(ProposalDocument::try_from).transpose()
    }

    pub async fn list_categorized(
        &self,
        limit: i64,
    ) -> Result<Vec<ProposalDocument>, KnowledgeBankError> {
        let pool = self.read()?;
        let rows = sqlx::query_as!(
            ProposalDocumentRow,
            r#"
            SELECT id, title, created_at, category, status, metadata, structured, content,
                   proposal, proposal_revision, proposal_call_id, proposal_error, skip_reason,
                   apply_attempts, applied, decided_by, decided_at
            FROM knowledge_documents
            WHERE status = 'categorized' AND source = 'email'
            ORDER BY created_at
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))?;
        rows.into_iter().map(ProposalDocument::try_from).collect()
    }

    pub async fn set_skipped(&self, id: Uuid, reason: &str) -> Result<bool, KnowledgeBankError> {
        let pool = self.write()?;
        let updated = sqlx::query!(
            r#"
            UPDATE knowledge_documents
            SET status = 'skipped', skip_reason = $2
            WHERE id = $1 AND status = 'categorized'
            "#,
            id,
            reason
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))?;
        Ok(updated.rows_affected() == 1)
    }

    pub async fn set_proposed(
        &self,
        id: Uuid,
        proposal: &Proposal,
        call_id: &str,
    ) -> Result<bool, KnowledgeBankError> {
        let pool = self.write()?;
        let json = serde_json::to_value(proposal)?;
        let updated = sqlx::query!(
            r#"
            UPDATE knowledge_documents
            SET status = 'proposed', proposal = $2, proposal_revision = $3,
                proposal_call_id = $4, proposal_error = NULL, skip_reason = NULL
            WHERE id = $1 AND status = 'categorized' AND proposal_revision = $3 - 1
            "#,
            id,
            json,
            proposal.revision,
            call_id
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))?;
        Ok(updated.rows_affected() == 1)
    }

    // Why: the document is marked before the approval row is opened; if the
    // open fails the mark is undone so the feed never shows a proposal nobody
    // can answer.
    pub async fn revert_proposed(&self, id: Uuid, call_id: &str) -> Result<(), KnowledgeBankError> {
        let pool = self.write()?;
        sqlx::query!(
            r#"
            UPDATE knowledge_documents
            SET status = 'categorized', proposal_call_id = NULL
            WHERE id = $1 AND status = 'proposed' AND proposal_call_id = $2
            "#,
            id,
            call_id
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))?;
        Ok(())
    }
}
