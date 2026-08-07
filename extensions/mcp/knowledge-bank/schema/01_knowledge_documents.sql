-- The knowledge bank's one table: uploaded documents and the full-text index
-- `search_project_context` ranks against.
--
-- `content_tsv` is a stored generated column rather than a trigger-maintained
-- one so title and content can never drift out of sync with the vector that
-- indexes them — Postgres recomputes it on every write, and there is no code
-- path that can forget to.
--
-- `project` is nullable on purpose: it is a collection tag, not a foreign key.
-- A document that belongs to no particular project is still a document worth
-- searching, and the search and listing filters treat NULL as "unscoped"
-- rather than excluding it from unfiltered queries.

CREATE TABLE IF NOT EXISTS knowledge_documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT NOT NULL,
    source TEXT NOT NULL,
    project TEXT,
    content TEXT NOT NULL,
    content_tsv tsvector GENERATED ALWAYS AS (
        to_tsvector('english', coalesce(title, '') || ' ' || content)
    ) STORED,
    uploaded_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Categorization columns the knowledge jobs pipeline fills in. Declared
    -- here because this crate's list query reads them: the jobs extension's
    -- ALTER ... IF EXISTS migration no-ops when it runs before this table
    -- exists, which is exactly what a fresh database does.
    metadata JSONB,
    category TEXT,
    structured JSONB,
    status TEXT NOT NULL DEFAULT 'raw'
);

-- Ranked search reads this and nothing else.
CREATE INDEX IF NOT EXISTS idx_knowledge_documents_content_tsv
    ON knowledge_documents USING GIN (content_tsv);

-- Both the project-scoped listing and the newest-first empty-query fallback
-- are ordered by created_at, so the tag leads the index and the timestamp
-- descends behind it.
CREATE INDEX IF NOT EXISTS idx_knowledge_documents_project_created_at
    ON knowledge_documents (project, created_at DESC);
