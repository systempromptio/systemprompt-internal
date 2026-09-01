-- What each approved proposal wrote into Odoo, one row per action.
--
-- The ledger row is claimed before the Odoo call and finished after it, so a
-- retry after a crash mid-proposal skips the actions already `done` instead
-- of creating a second lead. `rfc5322_id` is the inbound Message-ID, which is
-- also stored on the Odoo `mail.message` and is the second, Odoo-side line of
-- defence against posting the same email twice.
CREATE TABLE IF NOT EXISTS knowledge_odoo_projection (
    document_id UUID NOT NULL,
    revision INTEGER NOT NULL,
    action_index INTEGER NOT NULL,
    kind TEXT NOT NULL,
    res_model TEXT NOT NULL,
    res_id BIGINT,
    odoo_message_id BIGINT,
    rfc5322_id TEXT NOT NULL,
    applied_by TEXT NOT NULL,
    odoo_login TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'done', 'failed', 'excluded')),
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    applied_at TIMESTAMPTZ,
    PRIMARY KEY (document_id, revision, action_index)
);

CREATE INDEX IF NOT EXISTS idx_knowledge_odoo_projection_rfc5322_id
    ON knowledge_odoo_projection (rfc5322_id);
