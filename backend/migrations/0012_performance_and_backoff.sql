-- GIN index for tag filtering, feed error backoff counter, and partial index for post likes.
CREATE INDEX IF NOT EXISTS community_post_tags_tags_gin_idx
    ON community_post_tags USING GIN (tags);

ALTER TABLE feeds
    ADD COLUMN IF NOT EXISTS consecutive_errors integer NOT NULL DEFAULT 0 CHECK (consecutive_errors >= 0);

CREATE INDEX IF NOT EXISTS post_preferences_likes_idx
    ON post_preferences (community_id, entry_id) WHERE liked;
