-- Installation-owned artist profiles are separate from server-owned subscriptions.
ALTER TABLE feeds
    ADD COLUMN avatar_url text NOT NULL DEFAULT '',
    ADD COLUMN official_links jsonb NOT NULL DEFAULT '[]'
        CHECK (jsonb_typeof(official_links) = 'array' AND jsonb_array_length(official_links) <= 8);
