ALTER TABLE webhooks ADD COLUMN community_id uuid REFERENCES communities(id) ON DELETE CASCADE;
CREATE INDEX webhooks_community_idx ON webhooks(community_id);
CREATE TABLE post_comments (
    id uuid PRIMARY KEY,
    community_id uuid NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    artist_id uuid NOT NULL,
    entry_id uuid NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES community_users(id) ON DELETE CASCADE,
    body text NOT NULL CHECK (char_length(btrim(body)) BETWEEN 1 AND 2000),
    created_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (artist_id,community_id) REFERENCES community_artists(id,community_id) ON DELETE CASCADE
);
CREATE INDEX post_comments_thread_idx ON post_comments(community_id,entry_id,created_at DESC,id DESC);
CREATE INDEX post_comments_artist_idx ON post_comments(artist_id,community_id);
CREATE INDEX post_comments_entry_idx ON post_comments(entry_id);
CREATE INDEX post_comments_user_recent_idx ON post_comments(user_id,created_at DESC);
