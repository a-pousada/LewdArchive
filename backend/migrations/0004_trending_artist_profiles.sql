-- Artist presentation belongs to a server subscription, not the shared feed.
ALTER TABLE community_artists
    ADD COLUMN avatar_url text NOT NULL DEFAULT '',
    ADD COLUMN official_links jsonb NOT NULL DEFAULT '[]'
        CHECK (jsonb_typeof(official_links) = 'array' AND jsonb_array_length(official_links) <= 8);
CREATE INDEX entries_feed_published_idx ON entries(feed_id, published_at DESC, id DESC);
CREATE INDEX post_preferences_engaged_idx ON post_preferences(community_id, entry_id)
    INCLUDE (liked, favorite) WHERE liked OR favorite;
