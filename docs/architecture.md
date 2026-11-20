# Architecture

Rust/Axum serves a React/TypeScript application and PostgreSQL-backed APIs. The root application is an installation operations dashboard. All artwork, artist subscriptions, favorites, comments, webhook management and feed transfer live under Community.

`api.rs` contains common authentication, errors and health handling. `admin.rs` defines installation-only operational endpoints. `community/` owns server-scoped authorization and domain services. `platforms/` separates identity/membership and message transport from community operations. Discord is implemented; additional identity adapters can reuse server discussion and preferences.

Feeds and parsed entries are shared ingestion data. A `community_artists` subscription determines server visibility, identity, announcements and manual media access. Polling ignores feeds without subscriptions. Private preferences and discussions are scoped to a user and server. Downloads share stored media, but file requests recheck the current server subscription and membership.

The ingestion worker parses RSS/Atom, imports history silently, then enqueues new post deliveries for subscribed communities. Durable jobs carry per-destination retry state. The community worker handles bot deliveries and interactions. Installation notification/download switches are global pauses; individual server and artist switches remain independently enforced.

The former global archive APIs, read markers, personal-library UI and installation webhook dispatch code are removed. Applied SQL migrations are immutable history. Old installation-only rows remain preserved but inactive; the migration disables their webhooks and retires pending global notification jobs. No implicit reassignment to a server occurs.

Authentication boundaries: the installation key grants operations access only; Discord OAuth sessions grant current-member access to selected communities. Server management requires owner or Administrator status. All scoped operations use explicit SQL joins and predicates; no client-supplied author IDs are trusted.
