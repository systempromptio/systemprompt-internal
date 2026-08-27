-- Team comms: addressed messages between people and their agent sessions.
--
-- The governing rule is that a message must never appear in a conversation it
-- was not addressed to, so every row carries an explicit `delivery_class`
-- decided at send time rather than inferred by each reader:
--
--   inbox   -- stored, raises an unread count, never injected into a session
--   session -- addressed to one session; only that session may surface it
--   urgent  -- every live session of the recipient; governance holds only
--
-- `comms_reads` is keyed on (user_id, session_id, scope) rather than on the
-- user alone. Two of Ed's sessions each keep their own high-water mark, which
-- is what makes "unread" mean per-agent instead of per-person.

CREATE TABLE IF NOT EXISTS comms_channels (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    required_role TEXT,
    urgent BOOLEAN NOT NULL DEFAULT FALSE,
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS comms_channel_members (
    channel_id TEXT NOT NULL REFERENCES comms_channels(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    muted BOOLEAN NOT NULL DEFAULT FALSE,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (channel_id, user_id)
);

CREATE TABLE IF NOT EXISTS comms_messages (
    id TEXT PRIMARY KEY,
    channel_id TEXT REFERENCES comms_channels(id) ON DELETE CASCADE,
    sender_user_id TEXT NOT NULL,
    sender_session_id TEXT,
    sender_handle TEXT,
    recipient_user_id TEXT,
    recipient_session_id TEXT,
    delivery_class TEXT NOT NULL
        CHECK (delivery_class IN ('inbox', 'session', 'urgent')),
    body TEXT NOT NULL,
    thread_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT comms_messages_has_destination
        CHECK (channel_id IS NOT NULL OR recipient_user_id IS NOT NULL)
);

CREATE TABLE IF NOT EXISTS comms_reads (
    user_id TEXT NOT NULL,
    session_id TEXT NOT NULL DEFAULT '',
    scope TEXT NOT NULL,
    last_read_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, session_id, scope)
);

CREATE INDEX IF NOT EXISTS idx_comms_messages_recipient
    ON comms_messages (recipient_user_id, created_at DESC)
    WHERE recipient_user_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_comms_messages_session
    ON comms_messages (recipient_session_id, created_at DESC)
    WHERE recipient_session_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_comms_messages_channel
    ON comms_messages (channel_id, created_at DESC)
    WHERE channel_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_comms_messages_thread
    ON comms_messages (thread_id, created_at)
    WHERE thread_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_comms_channel_members_user
    ON comms_channel_members (user_id);
