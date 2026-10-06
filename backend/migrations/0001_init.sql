CREATE TABLE settings (
    id boolean PRIMARY KEY DEFAULT true CHECK (id),
    webhook_url text NOT NULL DEFAULT '',
    downloads_enabled boolean NOT NULL DEFAULT true,
    notifications_enabled boolean NOT NULL DEFAULT true
);
INSERT INTO settings (id) VALUES (true);

CREATE TABLE feeds (
    id uuid PRIMARY KEY,
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 120),
    url text NOT NULL UNIQUE,
    source text NOT NULL DEFAULT 'rss',
    enabled boolean NOT NULL DEFAULT true,
    auto_download boolean NOT NULL DEFAULT false,
    notify boolean NOT NULL DEFAULT true,
    poll_minutes integer NOT NULL DEFAULT 15 CHECK (poll_minutes BETWEEN 5 AND 1440),
    initialized boolean NOT NULL DEFAULT false,
    etag text,
    last_modified text,
    last_checked_at timestamptz,
    next_check_at timestamptz NOT NULL DEFAULT now(),
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX feeds_due_idx ON feeds (next_check_at) WHERE enabled;

CREATE TABLE entries (
    id uuid PRIMARY KEY,
    feed_id uuid NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    guid text NOT NULL,
    title text NOT NULL,
    author text NOT NULL,
    url text NOT NULL,
    summary text NOT NULL DEFAULT '',
    media jsonb NOT NULL DEFAULT '[]' CHECK (jsonb_typeof(media) = 'array'),
    published_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    is_read boolean NOT NULL DEFAULT false,
    favorite boolean NOT NULL DEFAULT false,
    UNIQUE(feed_id, guid)
);
CREATE INDEX entries_published_idx ON entries (published_at DESC, id DESC);
CREATE INDEX entries_unread_idx ON entries (published_at DESC) WHERE NOT is_read;
CREATE INDEX entries_favorite_idx ON entries (published_at DESC) WHERE favorite;

CREATE TABLE jobs (
    id uuid PRIMARY KEY,
    entry_id uuid NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('discord', 'download')),
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'done', 'failed')),
    attempts integer NOT NULL DEFAULT 0,
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    locked_at timestamptz,
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(entry_id, kind)
);
CREATE INDEX jobs_pending_idx ON jobs (next_attempt_at) WHERE status = 'pending';
CREATE INDEX jobs_activity_idx ON jobs (created_at DESC);
CREATE TABLE downloads (
    id uuid PRIMARY KEY,
    entry_id uuid NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    media_url text NOT NULL,
    filename text NOT NULL,
    content_type text NOT NULL,
    size_bytes bigint NOT NULL CHECK (size_bytes >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(entry_id, media_url)
);
