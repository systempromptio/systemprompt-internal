-- Validity windows for the two core-owned access rows this extension bounds
-- (`access_control_rules`, `user_device_certs`). Twin of
-- schema/41_time_bound_access.sql.

CREATE TABLE IF NOT EXISTS access_control_rule_validity (
    rule_id TEXT PRIMARY KEY REFERENCES access_control_rules(id) ON DELETE CASCADE,
    valid_until TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_access_control_rule_validity_until
    ON access_control_rule_validity(valid_until);

CREATE TABLE IF NOT EXISTS user_device_cert_validity (
    device_id TEXT PRIMARY KEY REFERENCES user_device_certs(id) ON DELETE CASCADE,
    valid_until TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_user_device_cert_validity_until
    ON user_device_cert_validity(valid_until);
