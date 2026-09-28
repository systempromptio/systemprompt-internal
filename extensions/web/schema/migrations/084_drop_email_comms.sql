-- Drop the tables the deleted email and comms MCP servers left behind.
--
-- Both crates are gone and nothing in the tree reads or writes these tables,
-- but no migration dropped them, so every established database still carries
-- them and the boot-time residue audit reports them as undeclared. A fresh
-- install never had them; on it this is a no-op.
DROP TABLE IF EXISTS email_outbox CASCADE;
DROP TABLE IF EXISTS comms_reads CASCADE;
DROP TABLE IF EXISTS comms_messages CASCADE;
DROP TABLE IF EXISTS comms_channel_members CASCADE;
DROP TABLE IF EXISTS comms_channels CASCADE;
