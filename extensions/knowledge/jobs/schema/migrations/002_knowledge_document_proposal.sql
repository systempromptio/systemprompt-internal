-- Odoo projection columns on knowledge_documents. IF EXISTS guards the table
-- (it belongs to the knowledge-bank extension); the jobs re-apply this
-- idempotently at run time, as they do for 001.
ALTER TABLE IF EXISTS knowledge_documents ADD COLUMN IF NOT EXISTS proposal JSONB;
ALTER TABLE IF EXISTS knowledge_documents ADD COLUMN IF NOT EXISTS proposal_revision INTEGER NOT NULL DEFAULT 0;
ALTER TABLE IF EXISTS knowledge_documents ADD COLUMN IF NOT EXISTS proposal_call_id TEXT;
ALTER TABLE IF EXISTS knowledge_documents ADD COLUMN IF NOT EXISTS proposal_error TEXT;
ALTER TABLE IF EXISTS knowledge_documents ADD COLUMN IF NOT EXISTS skip_reason TEXT;
ALTER TABLE IF EXISTS knowledge_documents ADD COLUMN IF NOT EXISTS apply_attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE IF EXISTS knowledge_documents ADD COLUMN IF NOT EXISTS next_attempt_at TIMESTAMPTZ;
ALTER TABLE IF EXISTS knowledge_documents ADD COLUMN IF NOT EXISTS applied JSONB;
ALTER TABLE IF EXISTS knowledge_documents ADD COLUMN IF NOT EXISTS decided_by TEXT;
ALTER TABLE IF EXISTS knowledge_documents ADD COLUMN IF NOT EXISTS decided_at TIMESTAMPTZ;
CREATE INDEX IF NOT EXISTS idx_knowledge_documents_status_created_at
    ON knowledge_documents (status, created_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_knowledge_documents_proposal_call_id
    ON knowledge_documents (proposal_call_id) WHERE proposal_call_id IS NOT NULL;
-- Documents categorized before the prompt emitted crm_intent go back through
-- the one prompt rather than through a second, proposal-only prompt.
UPDATE knowledge_documents
SET status = 'raw'
WHERE status = 'categorized' AND (structured -> 'crm_intent') IS NULL;
