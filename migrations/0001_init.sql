-- WindowsIsland 插件开放平台 — 初始模式
-- 开发者 / 插件 / 申请 / 审计 / 站点配置 / 邮件记录 / 会话 / 待注册 / 宿主权限表

CREATE TABLE developers (
    id                      BIGSERIAL PRIMARY KEY,
    github_user_id          BIGINT UNIQUE,
    github_login            TEXT NOT NULL DEFAULT '',
    developer_id            TEXT NOT NULL UNIQUE
                            CHECK (developer_id ~ '^[a-z][a-z0-9_]*$' AND char_length(developer_id) <= 64),
    display_name            TEXT NOT NULL CHECK (char_length(display_name) <= 200),
    email                   TEXT,
    avatar_url              TEXT,
    is_admin                BOOLEAN NOT NULL DEFAULT FALSE,
    is_banned               BOOLEAN NOT NULL DEFAULT FALSE,
    is_system               BOOLEAN NOT NULL DEFAULT FALSE,
    github_token_encrypted  TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE plugins (
    id                      BIGSERIAL PRIMARY KEY,
    developer_id            BIGINT NOT NULL REFERENCES developers(id),
    plugin_id               TEXT NOT NULL UNIQUE,
    repository_url          TEXT NOT NULL,
    homepage_url            TEXT,
    docs_url                TEXT,
    status                  TEXT NOT NULL DEFAULT 'draft'
                            CHECK (status IN ('draft','pending','published','rejected','cancelled','stopped')),
    current_version         TEXT,
    current_submission_id   BIGINT,
    updates_halted          BOOLEAN NOT NULL DEFAULT FALSE,
    updates_halted_reason   TEXT,
    stop_reason             TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_plugins_developer ON plugins(developer_id);
CREATE INDEX idx_plugins_status ON plugins(status);

CREATE TABLE submissions (
    id                BIGSERIAL PRIMARY KEY,
    plugin_id         BIGINT NOT NULL REFERENCES plugins(id),
    version           TEXT NOT NULL,
    manifest_json     JSONB NOT NULL,
    manifest_name     TEXT NOT NULL,
    description       TEXT NOT NULL,
    permissions       TEXT[] NOT NULL DEFAULT '{}',
    download_url      TEXT NOT NULL,
    sha256            TEXT NOT NULL,
    min_host_version  TEXT NOT NULL,
    contract_version  INTEGER NOT NULL,
    status            TEXT NOT NULL DEFAULT 'pending'
                      CHECK (status IN ('pending','approved','rejected','cancelled')),
    is_new_version    BOOLEAN NOT NULL DEFAULT FALSE,
    review_reason     TEXT,
    ai_report_json    JSONB,
    ai_state          TEXT NOT NULL DEFAULT 'skipped'
                      CHECK (ai_state IN ('skipped','ok','failed')),
    submitted_by      BIGINT NOT NULL REFERENCES developers(id),
    reviewed_by       BIGINT REFERENCES developers(id),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    reviewed_at       TIMESTAMPTZ
);
CREATE INDEX idx_submissions_plugin ON submissions(plugin_id);
CREATE INDEX idx_submissions_status ON submissions(status);
CREATE INDEX idx_submissions_created ON submissions(created_at);

-- plugins.current_submission_id 需在 submissions 建表后补挂外键（两表互相引用）
ALTER TABLE plugins
    ADD CONSTRAINT fk_plugins_current_submission
    FOREIGN KEY (current_submission_id) REFERENCES submissions(id);

-- 审计日志：只增不改（数据库层禁止 UPDATE / DELETE）
CREATE TABLE audit_logs (
    id             BIGSERIAL PRIMARY KEY,
    actor          TEXT NOT NULL,
    actor_role     TEXT NOT NULL CHECK (actor_role IN ('developer','admin','system')),
    action         TEXT NOT NULL,
    plugin_id      TEXT,
    submission_id  BIGINT,
    from_status    TEXT,
    to_status      TEXT,
    reason         TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_audit_plugin ON audit_logs(plugin_id);
CREATE INDEX idx_audit_actor ON audit_logs(actor);
CREATE INDEX idx_audit_created ON audit_logs(created_at);

CREATE OR REPLACE FUNCTION forbid_audit_mutation() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'audit_logs is append-only';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER audit_logs_no_update
    BEFORE UPDATE OR DELETE ON audit_logs
    FOR EACH ROW EXECUTE FUNCTION forbid_audit_mutation();

-- 站点配置：敏感项（SMTP 密码、AI Key 等）加密后存 value_encrypted
CREATE TABLE site_configs (
    key              TEXT PRIMARY KEY,
    value_encrypted  TEXT,
    is_secret        BOOLEAN NOT NULL DEFAULT FALSE,
    updated_by       TEXT,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE email_logs (
    id          BIGSERIAL PRIMARY KEY,
    to_address  TEXT NOT NULL,
    template    TEXT NOT NULL,
    subject     TEXT NOT NULL DEFAULT '',
    body        TEXT NOT NULL DEFAULT '',
    plugin_id   TEXT,
    status      TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued','sent','failed')),
    attempts    INTEGER NOT NULL DEFAULT 0,
    error       TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    sent_at     TIMESTAMPTZ
);
CREATE INDEX idx_email_logs_created ON email_logs(created_at);

CREATE TABLE sessions (
    token_hash      TEXT PRIMARY KEY,
    developer_id    BIGINT NOT NULL REFERENCES developers(id) ON DELETE CASCADE,
    csrf_token      TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at      TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_sessions_developer ON sessions(developer_id);

-- GitHub 首次登录后、developerId 设定前的临时注册记录
CREATE TABLE pending_registrations (
    token             TEXT PRIMARY KEY,
    github_user_id    BIGINT NOT NULL UNIQUE,
    github_login      TEXT NOT NULL,
    email             TEXT,
    avatar_url        TEXT,
    display_name      TEXT,
    token_encrypted   TEXT NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 宿主权限表（平台内置副本，需与宿主侧同步更新）
CREATE TABLE known_permissions (
    name         TEXT PRIMARY KEY,
    risk         TEXT NOT NULL CHECK (risk IN ('low','medium','high')),
    description  TEXT NOT NULL DEFAULT ''
);

INSERT INTO known_permissions (name, risk, description) VALUES
    ('notifications', 'low',    '发送系统通知'),
    ('storage',       'low',    '插件私有存储读写'),
    ('clipboard',     'medium', '读写剪贴板'),
    ('network',       'high',   '任意网络访问（进程内无法强制隔离）'),
    ('process',       'high',   '读取进程信息 / 进程标识（宿主会剥离进程标识）'),
    ('filesystem',    'high',   '任意文件系统访问（进程内无法强制隔离）'),
    ('automation',    'high',   'UI 自动化能力（进程内无法强制隔离）'),
    ('native',        'high',   '原生调用（进程内无法强制隔离）');

-- 官方开发者身份 winiofficial（系统身份，无 GitHub 登录）
INSERT INTO developers (github_user_id, github_login, developer_id, display_name, is_system)
VALUES (NULL, '', 'winiofficial', 'WindowsIsland 官方', TRUE);

-- 站点默认配置
INSERT INTO site_configs (key, value_encrypted, is_secret) VALUES
    ('site_name',              'WindowsIsland 插件中心', FALSE),
    ('announcement',           '',                        FALSE),
    ('page_size',              '20',                      FALSE),
    ('public_catalog_enabled', 'true',                    FALSE),
    ('catalog_base_url',       'https://plugins.windowsisland.example', FALSE),
    ('smtp_host',              '',                        FALSE),
    ('smtp_port',              '587',                     FALSE),
    ('smtp_username',          '',                        FALSE),
    ('smtp_password',          '',                        TRUE),
    ('smtp_encryption',        'starttls',                FALSE),
    ('smtp_from_address',      '',                        FALSE),
    ('smtp_from_name',         '',                        FALSE),
    ('ai_enabled',             'false',                   FALSE),
    ('ai_base_url',            '',                        FALSE),
    ('ai_model',               '',                        FALSE),
    ('ai_api_key',             '',                        TRUE),
    ('ai_timeout_sec',         '60',                      FALSE);
