-- Store the HTML variant alongside the plain-text email body.
ALTER TABLE email_logs
    ADD COLUMN body_html TEXT NOT NULL DEFAULT '';
