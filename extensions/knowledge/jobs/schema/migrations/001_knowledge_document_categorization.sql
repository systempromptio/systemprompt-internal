-- Categorization columns on knowledge_documents. IF EXISTS guards the
-- table: it belongs to the knowledge-bank extension, and ordering between
-- extension schemas and these migrations is not guaranteed; the job also
-- re-applies this idempotently at run time.
ALTER TABLE IF EXISTS knowledge_documents ADD COLUMN IF NOT EXISTS metadata JSONB;
ALTER TABLE IF EXISTS knowledge_documents ADD COLUMN IF NOT EXISTS category TEXT;
ALTER TABLE IF EXISTS knowledge_documents ADD COLUMN IF NOT EXISTS structured JSONB;
ALTER TABLE IF EXISTS knowledge_documents ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'raw';
