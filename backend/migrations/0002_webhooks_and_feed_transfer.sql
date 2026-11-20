ALTER TABLE feeds ADD COLUMN category text NOT NULL DEFAULT '' CHECK (char_length(category) <= 500);
ALTER TABLE feeds ADD COLUMN site_url text NOT NULL DEFAULT '';

CREATE TABLE webhooks (
    id uuid PRIMARY KEY,
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 120),
    url text NOT NULL UNIQUE,
    username text NOT NULL DEFAULT '' CHECK (char_length(username) <= 80),
    avatar_url text NOT NULL DEFAULT '',
    embed_color integer NOT NULL DEFAULT 14978709 CHECK (embed_color BETWEEN 0 AND 16777215),
    enabled boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now()
);
-- Preserve the original destination and its existing delivery history.
INSERT INTO webhooks (id, name, url, username)
SELECT '00000000-0000-0000-0000-000000000001', 'Default webhook', webhook_url, 'LewdArchive'
FROM settings WHERE webhook_url <> '';
ALTER TABLE jobs ADD COLUMN webhook_id uuid REFERENCES webhooks(id) ON DELETE CASCADE;
UPDATE jobs SET webhook_id = '00000000-0000-0000-0000-000000000001'
WHERE kind = 'discord' AND EXISTS (SELECT 1 FROM webhooks);
ALTER TABLE jobs DROP CONSTRAINT jobs_entry_id_kind_key;
ALTER TABLE jobs ADD CONSTRAINT jobs_delivery_key UNIQUE NULLS NOT DISTINCT (entry_id, kind, webhook_id);
ALTER TABLE jobs ADD CONSTRAINT jobs_download_no_webhook CHECK (kind = 'discord' OR webhook_id IS NULL);
CREATE INDEX jobs_webhook_idx ON jobs (webhook_id) WHERE webhook_id IS NOT NULL;
ALTER TABLE settings DROP COLUMN webhook_url;
