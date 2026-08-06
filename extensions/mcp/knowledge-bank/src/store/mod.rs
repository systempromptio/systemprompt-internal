//! The knowledge bank's persistence layer: reads and writes against
//! `knowledge_documents` on the tenant Postgres.
//!
//! Search has three modes, picked before any SQL runs:
//!
//! - a query with something to match on goes to `websearch_to_tsquery` ranked
//!   by `ts_rank_cd`, with a `ts_headline` snippet;
//! - a query that produces no lexemes — punctuation, a bare stopword, a
//!   substring of a word — falls back to `ILIKE`, because returning nothing
//!   for "checkou" would read as an empty knowledge bank rather than a query
//!   the tokenizer could not use;
//! - an empty or one-character query lists the newest documents, which is what
//!   a caller orienting themselves actually wants.
//!
//! The mode decision and the limit clamp are pure functions so they can be
//! tested without a database; the queries themselves are exercised by the
//! integration suite against a throwaway schema.

pub mod query;
pub mod rows;

pub use query::{
    DEFAULT_SEARCH_LIMIT, MAX_CONTENT_BYTES, MAX_LIST_LIMIT, MAX_SEARCH_LIMIT, SearchMode,
    check_content_size, clamp_search_limit, like_pattern, normalize_optional, require_non_empty,
    search_mode,
};
pub use rows::{DocumentSummary, NewDocument, SearchHit, UploadedDocument};

use systemprompt::database::DbPool;

use crate::error::KnowledgeBankError;

// Why: characters of `content` returned as a snippet when there is no
// `ts_headline` to build one from (the newest-first and `ILIKE` paths).
const SNIPPET_CHARS: i32 = 300;

/// The knowledge bank's document store.
#[derive(Debug, Clone)]
pub struct KnowledgeStore {
    pool: DbPool,
}

impl KnowledgeStore {
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn read(&self) -> Result<std::sync::Arc<sqlx::PgPool>, KnowledgeBankError> {
        self.pool.pool().ok_or_else(|| {
            KnowledgeBankError::Internal("no Postgres read pool available".to_owned())
        })
    }

    fn write(&self) -> Result<std::sync::Arc<sqlx::PgPool>, KnowledgeBankError> {
        self.pool.write_pool().ok_or_else(|| {
            KnowledgeBankError::Internal("no Postgres write pool available".to_owned())
        })
    }

    /// Ranked full-text search, with the two fallbacks described on the module.
    ///
    /// # Errors
    /// [`KnowledgeBankError::Internal`] if the query fails.
    pub async fn search(
        &self,
        query: &str,
        project: Option<&str>,
        limit: i64,
    ) -> Result<Vec<SearchHit>, KnowledgeBankError> {
        if search_mode(query) == SearchMode::Newest {
            return self.newest(project, limit).await;
        }

        let pool = self.read()?;
        let ranked = sqlx::query_as!(
            SearchHit,
            r#"
            SELECT
                id,
                title,
                source,
                project,
                created_at,
                uploaded_by,
                ts_headline(
                    'english',
                    content,
                    websearch_to_tsquery('english', $1),
                    'MaxFragments=2,MaxWords=28,MinWords=8,ShortWord=3,FragmentDelimiter= … '
                ) AS "snippet!"
            FROM knowledge_documents
            WHERE content_tsv @@ websearch_to_tsquery('english', $1)
              AND ($2::text IS NULL OR project = $2)
            ORDER BY
                ts_rank_cd(content_tsv, websearch_to_tsquery('english', $1)) DESC,
                created_at DESC
            LIMIT $3
            "#,
            query,
            project,
            limit
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))?;

        if !ranked.is_empty() {
            return Ok(ranked);
        }

        // Why: no ranked hits is either a genuine miss or a query the English
        // tokenizer threw away entirely — a partial word, punctuation, a bare
        // stopword. Substring matching separates the two, at the cost of one
        // extra query on a path that already returned nothing.
        self.search_like(query, project, limit).await
    }

    async fn search_like(
        &self,
        query: &str,
        project: Option<&str>,
        limit: i64,
    ) -> Result<Vec<SearchHit>, KnowledgeBankError> {
        let pool = self.read()?;
        sqlx::query_as!(
            SearchHit,
            r#"
            SELECT
                id,
                title,
                source,
                project,
                created_at,
                uploaded_by,
                left(content, $4) AS "snippet!"
            FROM knowledge_documents
            WHERE (title ILIKE $1 ESCAPE '\' OR content ILIKE $1 ESCAPE '\')
              AND ($2::text IS NULL OR project = $2)
            ORDER BY created_at DESC
            LIMIT $3
            "#,
            like_pattern(query),
            project,
            limit,
            SNIPPET_CHARS
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))
    }

    /// The newest documents, optionally scoped to a project — the answer to an
    /// empty query.
    ///
    /// # Errors
    /// [`KnowledgeBankError::Internal`] if the query fails.
    pub async fn newest(
        &self,
        project: Option<&str>,
        limit: i64,
    ) -> Result<Vec<SearchHit>, KnowledgeBankError> {
        let pool = self.read()?;
        sqlx::query_as!(
            SearchHit,
            r#"
            SELECT
                id,
                title,
                source,
                project,
                created_at,
                uploaded_by,
                left(content, $3) AS "snippet!"
            FROM knowledge_documents
            WHERE ($1::text IS NULL OR project = $1)
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            project,
            limit,
            SNIPPET_CHARS
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))
    }

    /// Newest-first listing, optionally filtered by project and source.
    ///
    /// # Errors
    /// [`KnowledgeBankError::Internal`] if the query fails.
    pub async fn list_documents(
        &self,
        project: Option<&str>,
        source: Option<&str>,
    ) -> Result<Vec<DocumentSummary>, KnowledgeBankError> {
        let pool = self.read()?;
        sqlx::query_as!(
            DocumentSummary,
            r#"
            SELECT
                id,
                title,
                source,
                project,
                created_at,
                char_length(content) AS "size!"
            FROM knowledge_documents
            WHERE ($1::text IS NULL OR project = $1)
              AND ($2::text IS NULL OR source = $2)
            ORDER BY created_at DESC
            LIMIT $3
            "#,
            project,
            source,
            MAX_LIST_LIMIT
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))
    }

    /// Persist a document. `content_tsv` is generated by Postgres, so the row
    /// is searchable the moment this returns.
    ///
    /// # Errors
    /// [`KnowledgeBankError::Internal`] if the insert fails.
    pub async fn insert(
        &self,
        document: NewDocument<'_>,
    ) -> Result<UploadedDocument, KnowledgeBankError> {
        let pool = self.write()?;
        sqlx::query_as!(
            UploadedDocument,
            r#"
            INSERT INTO knowledge_documents (title, source, project, content, uploaded_by)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, created_at
            "#,
            document.title,
            document.source,
            document.project,
            document.content,
            document.uploaded_by
        )
        .fetch_one(pool.as_ref())
        .await
        .map_err(|e| KnowledgeBankError::Internal(e.to_string()))
    }

    /// Total document count. Used only for the startup log line.
    ///
    /// # Errors
    /// [`KnowledgeBankError::Internal`] if the query fails.
    pub async fn count(&self) -> Result<i64, KnowledgeBankError> {
        let pool = self.read()?;
        sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM knowledge_documents"#)
            .fetch_one(pool.as_ref())
            .await
            .map_err(|e| KnowledgeBankError::Internal(e.to_string()))
    }
}
