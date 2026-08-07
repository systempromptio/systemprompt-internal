-- Email ingestion ledger for the knowledge bank.
--
-- The ledger is the durable dedupe boundary: IMAP \Seen flags can be reset by
-- any mail client, so a Message-ID row here — not mailbox state — is what
-- makes re-ingestion impossible.
CREATE TABLE IF NOT EXISTS knowledge_email_ingest (
    message_id TEXT PRIMARY KEY,
    document_id UUID,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
