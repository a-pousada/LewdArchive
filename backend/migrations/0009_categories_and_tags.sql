-- Categories remain server-owned; existing OPML category labels are preserved.
CREATE TABLE community_category_routes (
    community_id uuid NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    category text NOT NULL CHECK (char_length(category) BETWEEN 1 AND 500),
    channel_id text NOT NULL,
    channel_name text NOT NULL DEFAULT '',
    PRIMARY KEY (community_id, category)
);
CREATE INDEX community_artists_category_idx ON community_artists(community_id, category);
ALTER TABLE webhooks ADD COLUMN category_filter text CHECK (char_length(category_filter) <= 500);

CREATE TABLE community_post_tags (
    community_id uuid NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    entry_id uuid NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    artist_id uuid NOT NULL,
    tags text[] NOT NULL DEFAULT '{}' CHECK (cardinality(tags) <= 30),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (community_id, entry_id),
    FOREIGN KEY (artist_id, community_id) REFERENCES community_artists(id, community_id) ON DELETE CASCADE
);
CREATE INDEX community_post_tags_entry_idx ON community_post_tags(entry_id);
CREATE INDEX community_post_tags_artist_idx ON community_post_tags(artist_id, community_id);
