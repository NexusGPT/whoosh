//! The relay's whole store. By design it holds device tokens, opaque routing ids, push-job
//! bookkeeping and tracking events, and nothing else: no mailbox credential, no message content,
//! no recipient address. tests/schema.rs reads the created file and fails on any other column.

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS registration (
    device_token  TEXT NOT NULL,
    routing_id    TEXT NOT NULL,
    environment   TEXT NOT NULL CHECK (environment IN ('sandbox', 'production')),
    registered_at INTEGER NOT NULL,
    PRIMARY KEY (device_token, routing_id)
);
CREATE INDEX IF NOT EXISTS registration_by_routing ON registration (routing_id);

CREATE TABLE IF NOT EXISTS push_job (
    job_id       INTEGER PRIMARY KEY AUTOINCREMENT,
    signal_id    TEXT NOT NULL,
    device_token TEXT NOT NULL,
    routing_id   TEXT NOT NULL,
    environment  TEXT NOT NULL,
    opaque_ref   TEXT,
    state        TEXT NOT NULL CHECK (state IN ('queued', 'sent', 'rejected', 'failed')),
    apns_status  INTEGER,
    apns_reason  TEXT,
    enqueued_at  INTEGER NOT NULL,
    finished_at  INTEGER,
    UNIQUE (signal_id, device_token)
);
CREATE INDEX IF NOT EXISTS push_job_by_state ON push_job (state, job_id);

CREATE TABLE IF NOT EXISTS tracked_message (
    token      TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS tracking_event (
    event_id    INTEGER PRIMARY KEY AUTOINCREMENT,
    token       TEXT NOT NULL REFERENCES tracked_message (token),
    kind        TEXT NOT NULL CHECK (kind IN ('open', 'click')),
    occurred_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS tracking_event_by_token ON tracking_event (token, kind);
";
