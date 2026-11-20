ALTER TABLE communities ADD COLUMN ai_tags_enabled boolean NOT NULL DEFAULT false;
ALTER TABLE community_post_tags ADD COLUMN source text NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'ai'));

CREATE TABLE community_tag_jobs (
    community_id uuid NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    entry_id uuid NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    artist_id uuid NOT NULL,
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'done', 'failed')),
    attempts integer NOT NULL DEFAULT 0,
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    locked_at timestamptz,
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (community_id, entry_id),
    FOREIGN KEY (artist_id, community_id) REFERENCES community_artists(id, community_id) ON DELETE CASCADE
);
CREATE INDEX community_tag_jobs_due_idx ON community_tag_jobs(next_attempt_at) WHERE status='pending';
CREATE INDEX community_tag_jobs_entry_idx ON community_tag_jobs(entry_id);
CREATE INDEX community_tag_jobs_artist_idx ON community_tag_jobs(artist_id, community_id);
