-- Reconciliation ledger for outbound mail.
--
-- The failure this exists for: SMTP accepts the message and the Odoo chatter
-- write then fails, leaving a mail that was really sent with no trace anywhere.
-- The row is written BEFORE the send and updated after, so a crash between the
-- two leaves evidence rather than silence.
--
-- Mirrors the shape `knowledge_email_ingest` uses for the inbound direction:
-- the RFC5322 Message-ID is the natural key, because it is the one identifier
-- that exists on the wire, in Odoo's `mail.message.message_id`, and in any
-- reply that later quotes it via In-Reply-To.
CREATE TABLE IF NOT EXISTS email_outbox (
    message_id        TEXT PRIMARY KEY,
    user_id           TEXT        NOT NULL,
    recipients        TEXT[]      NOT NULL,
    subject           TEXT        NOT NULL,
    res_model         TEXT,
    res_id            BIGINT,
    -- queued -> sent -> logged, or queued -> failed. `sent` with a non-null
    -- res_model and a null odoo_message_id is exactly the reconciliation case.
    status            TEXT        NOT NULL DEFAULT 'queued',
    odoo_message_id   BIGINT,
    error             TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sent_at           TIMESTAMPTZ,
    logged_at         TIMESTAMPTZ
);

-- The reconciliation sweep: sent, anchored, but never logged.
CREATE INDEX IF NOT EXISTS email_outbox_unlogged_idx
    ON email_outbox (status, sent_at)
    WHERE res_model IS NOT NULL AND odoo_message_id IS NULL;
