-- Keep the existing installation-owner archive and compatible webhooks intact.
-- Community subscriptions reference shared feed ingestion, never grant access to owner APIs.
CREATE TABLE community_users (
    id uuid PRIMARY KEY,
    platform text NOT NULL,
    external_id text NOT NULL,
    display_name text NOT NULL,
    avatar_url text NOT NULL DEFAULT '',
    UNIQUE(platform, external_id)
);
CREATE TABLE community_sessions (
    token_hash text PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES community_users(id) ON DELETE CASCADE,
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX community_sessions_user_idx ON community_sessions(user_id);
CREATE INDEX community_sessions_expiry_idx ON community_sessions(expires_at);
CREATE TABLE oauth_states (
    token_hash text PRIMARY KEY,
    expires_at timestamptz NOT NULL
);
CREATE TABLE login_guilds (
    user_id uuid NOT NULL REFERENCES community_users(id) ON DELETE CASCADE,
    external_id text NOT NULL,
    name text NOT NULL,
    can_manage boolean NOT NULL,
    PRIMARY KEY(user_id,external_id)
);
CREATE TABLE communities (
    id uuid PRIMARY KEY,
    platform text NOT NULL,
    external_id text NOT NULL,
    name text NOT NULL,
    channel_id text,
    channel_name text NOT NULL DEFAULT '',
    enabled boolean NOT NULL DEFAULT true,
    mentions_enabled boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(platform,external_id)
);
CREATE TABLE community_artists (
    id uuid PRIMARY KEY,
    community_id uuid NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    feed_id uuid NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    name text NOT NULL CHECK(char_length(name) BETWEEN 1 AND 120),
    enabled boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(community_id,feed_id),
    UNIQUE(id,community_id)
);
CREATE INDEX community_artists_feed_idx ON community_artists(feed_id);
CREATE TABLE artist_preferences (
    user_id uuid NOT NULL REFERENCES community_users(id) ON DELETE CASCADE,
    artist_id uuid NOT NULL REFERENCES community_artists(id) ON DELETE CASCADE,
    favorite boolean NOT NULL DEFAULT false,
    notify boolean NOT NULL DEFAULT false,
    PRIMARY KEY(user_id,artist_id)
);
CREATE INDEX artist_preferences_artist_idx ON artist_preferences(artist_id);
CREATE TABLE post_preferences (
    user_id uuid NOT NULL REFERENCES community_users(id) ON DELETE CASCADE,
    community_id uuid NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    entry_id uuid NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    liked boolean NOT NULL DEFAULT false,
    favorite boolean NOT NULL DEFAULT false,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(user_id,community_id,entry_id)
);
CREATE INDEX post_preferences_community_entry_idx ON post_preferences(community_id,entry_id);
CREATE INDEX post_preferences_entry_idx ON post_preferences(entry_id);
CREATE TABLE community_deliveries (
    id uuid PRIMARY KEY,
    community_id uuid NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    artist_id uuid NOT NULL,
    entry_id uuid NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    channel_id text NOT NULL,
    recipient_ids text[] NOT NULL DEFAULT '{}',
    batch integer NOT NULL DEFAULT 0,
    status text NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','processing','done','failed')),
    attempts integer NOT NULL DEFAULT 0,
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    locked_at timestamptz,
    last_error text,
    message_id text,
    created_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY(artist_id,community_id) REFERENCES community_artists(id,community_id) ON DELETE CASCADE,
    UNIQUE(community_id,entry_id,batch)
);
CREATE INDEX community_deliveries_entry_idx ON community_deliveries(entry_id);
CREATE INDEX community_deliveries_artist_idx ON community_deliveries(artist_id,community_id);
CREATE INDEX community_deliveries_due_idx ON community_deliveries(next_attempt_at) WHERE status='pending';
CREATE TABLE interaction_jobs (
    external_id text PRIMARY KEY,
    payload jsonb NOT NULL,
    result jsonb,
    status text NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','processing','done','failed')),
    attempts integer NOT NULL DEFAULT 0,
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    locked_at timestamptz,
    expires_at timestamptz NOT NULL DEFAULT now()+interval '14 minutes'
);
CREATE INDEX interaction_jobs_due_idx ON interaction_jobs(next_attempt_at) WHERE status='pending';
