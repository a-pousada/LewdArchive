# LewdArchive

A server-based artist tracker with RSS ingestion, Discord communities, private collections and compatible webhooks.

Built with **Rust / Axum**, **PostgreSQL**, and **React / TypeScript**. Self-hosted, with installation administration and Discord communities, with a dark interface inspired by manga collections.

## Discord communities

The optional Discord bot sends artwork embeds to server channels, with likes, private post/artist favorites, and opt-in artist mentions. Members sign in at `/community`; the installation operations dashboard stays at `/`. See [bot setup and commands](docs/discord-bot.md). Compatible webhooks are managed per server. Trending ranks posts by likes, saves or combined engagement over rolling 7/30/365-day publication windows, on the website and with `/archive trending`. Managers can customize artist photos and official links.

## Quick start

Requires Docker Compose **2.24+** and Node.js to generate the configuration. Alternatively, copy `.env.example` to `.env` and fill in the required secrets manually.

```bash
node scripts/init-env.mjs
docker compose up -d --build
```

Open **http://localhost:8080** and sign in with `API_TOKEN` from `.env`. The generator creates random credentials with restricted file permissions and never overwrites an existing file.

The stack includes the application, PostgreSQL, **RSSHub with Chromium**, and Redis for RSSHub's cache. RSSHub is available locally at **http://localhost:1200**. Database and cache ports are not published.

1. Open **Settings → Add webhook** and configure one or more webhook destinations (Discord, Fluxer, etc.).
2. Click **New feed**, choose Pawchive, Pixiv, or Bluesky, and enter the artist's ID or handle. Use **Custom — full URL** for other RSS/Atom feeds, including Xcancel.
3. Configure announcements for your server. Download media manually from an individual post when needed.
4. Use `/` for installation operations. Configure Discord, then open `/community` to manage artists and browse posts in your server.

The first sync imports existing posts **without notifications**. Later syncs create jobs only for newly discovered post identifiers. Historical posts can be downloaded manually.

To test a server webhook, open a post’s **Files** panel in Community and choose **Send to server webhooks**. Monitor the result in **Delivery activity**. Global and server notification switches must be enabled.

## Features

- RSS and Atom parsing, including feeds provided by RSSHub.
- Per-feed polling from 5 minutes to 24 hours, pause/resume, and on-demand synchronization.
- Conditional requests using `ETag` and `Last-Modified`, persistent deduplication, and visible feed errors.
- Server-scoped artwork, private favorites, likes, comments and trending rankings.
- Multiple images and videos extracted from feed HTML, enclosures, and Media RSS.
- Discord-compatible webhook messages (Discord, Fluxer, etc.) with title, author, description, original link, date, up to four images, and video links. Mentions are disabled.
- Persistent job queue with retries, rate-limit handling, and manual recovery.
- Automatic or manual downloads with file-size limits, streaming, and authenticated access.
- Global controls for pausing downloads and notifications.
- Access-key authentication, HttpOnly cookies, URL validation, private-network restrictions, and redirect checks.

## Import and export

Open **Community → Artists → Import OPML** to upload an export from Miniflux or another RSS reader. Review the preview, choose automation settings for new feeds, and import. Existing subscriptions in that server are skipped without changing their settings. Invalid entries are reported. Files may contain up to 1,000 feeds and be up to 2 MiB.

**Export OPML** downloads your feed names, URLs, categories, website URLs, sources, and paused state. This is a subscription transfer, not a backup of posts, files, webhook secrets, or Miniflux-specific rules. See [feed transfer](docs/feed-transfer.md).

## Multiple webhook destinations

In **Settings**, add, edit, pause, or remove webhooks. Each has a destination label, optional message display name and HTTPS avatar URL, and an embed color with a live preview. Webhooks support Discord, Discord clones (like Fluxer), and compatible HTTP(S) endpoints. Blank identity fields use destination defaults. These settings customize LewdArchive messages without changing the webhook's defaults for other applications.

New posts go to enabled destinations whose category filter matches the artist. Existing destinations default to all categories. Each destination has its own job and retries; Activity identifies the channel. A retry for a failed channel does not resend to successful channels. Pausing a destination keeps existing jobs queued; it is excluded from new deliveries. Posts discovered before any webhook is configured wait for a destination.

The existing single webhook and its delivery history are migrated automatically on startup. An in-flight message may still finish after a pause or deletion.

## Categories and AI tags

Organize artists into 2D Illustration, 3D Art, Photography, Mixed Media or custom categories. Route each category to a Discord channel, filter webhook destinations, and browse posts by category or tag. Optional local WD SwinV2 v3 inference runs on CPU and offers reviewed image suggestions and opt-in automatic tagging of new posts. Manual edits remain authoritative. See [setup and behavior](docs/categories-and-ai.md), [model choice and CPU timings](docs/wd-tagger.md), and [review findings and next ideas](docs/project-review.md).

## RSSHub

The feed builder uses the bundled instance at `http://rsshub:1200` automatically. For example, Pawchive + Fanbox + artist ID `22445` produces:

```text
http://rsshub:1200/pawchive/fanbox/22445
```

Use the internal hostname in feeds registered with the Docker application. `localhost` inside a container refers to that container, not your computer. The same route can be inspected from your browser at `http://localhost:1200/pawchive/fanbox/22445`.

Set `RSSHUB_BASE_URL` in `.env` to change the default instance, or override **RSSHub instance** when adding an individual feed. Existing feed URLs do not change. Native execution defaults to `https://rsshub.app` when the setting is empty.

Route-specific credentials belong in RSSHub. Copy `.env.rsshub.example` to `.env.rsshub`, add the variables required by your routes, and run `docker compose up -d rsshub`. The file is ignored by Git and loaded only by RSSHub. See [feed templates](docs/feed-templates.md) for supported routes and custom feeds.

## Configuration and operation

See [configuration](docs/configuration.md) for all environment variables, networking, updates, and backups. Your existing `.env` remains valid; new optional settings have defaults.

- The application listens on `127.0.0.1:8080` on the Docker host. Use an HTTPS reverse proxy and `COOKIE_SECURE=true` for remote access.
- The installation owner uses an access key; community members use Discord OAuth with server-scoped access. Favorites are private between members.
- Webhook URLs are stored in PostgreSQL and never returned by the API. Leave the URL field empty when editing to preserve it. Removing a destination also removes its queued deliveries and activity records.
- Back up both PostgreSQL and the media volume. Database backups alone do not include downloaded files.
- Removing a feed deletes its database records but preserves downloaded files on disk. There is no automatic retention cleanup or total disk quota.
- Notifications use at-least-once delivery. A retry may repeat a message if Discord accepted it but the response was lost.
- Feed availability and remote image rendering depend on the source. Posts removed before the next poll cannot be recovered from the feed.

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for local setup, tests, and repository conventions.

```text
backend/
  migrations/             Versioned database schema
  src/
    main.rs               Startup, routes, middleware, static assets
    api.rs                API handlers and validation
    models.rs             Shared data types
    parser.rs             Feed and media normalization
    network.rs            HTTP requests and network restrictions
    worker.rs             Polling, webhook queue, downloads
    community/            Server libraries, OAuth, preferences, bot queues
    platforms/            Discord adapter and future provider contract
frontend/
  src/features/           Login, post reader, feed form, settings
  src/App.tsx             Workspace navigation and management views
  src/components.tsx      Shared UI components
  src/api.ts              API client and TypeScript contracts
  src/feedTemplates.ts    RSSHub URL templates
  src/style.css           Theme and responsive layouts
  tests/                  Playwright tests
scripts/                  Environment setup and integration checks
docs/                     Configuration, architecture, API, templates
```

Community administration, scoped webhooks and post discussion: [management guide](docs/community-management.md).

Further reference: [architecture](docs/architecture.md) · [HTTP API](docs/api.md).
