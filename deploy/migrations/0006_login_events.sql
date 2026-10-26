-- Login language preference and auditable login metadata.
ALTER TABLE developers
    ADD COLUMN preferred_language TEXT NOT NULL DEFAULT 'zh'
    CHECK (preferred_language IN ('zh', 'en'));

ALTER TABLE sessions
    ADD COLUMN login_ip INET,
    ADD COLUMN login_user_agent TEXT,
    ADD COLUMN login_at TIMESTAMPTZ NOT NULL DEFAULT now();

CREATE TABLE login_events (
    event_id       TEXT PRIMARY KEY,
    developer_id   BIGINT NOT NULL REFERENCES developers(id) ON DELETE CASCADE,
    login_ip       INET,
    user_agent     TEXT NOT NULL DEFAULT '',
    language       TEXT NOT NULL DEFAULT 'zh' CHECK (language IN ('zh', 'en')),
    is_welcome     BOOLEAN NOT NULL DEFAULT FALSE,
    is_unusual     BOOLEAN NOT NULL DEFAULT FALSE,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX login_events_developer_created_idx
    ON login_events (developer_id, created_at DESC);
