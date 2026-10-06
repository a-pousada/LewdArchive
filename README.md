# LewdArchive

A personal archive for following artists through RSS, collecting their posts, and receiving updates on Discord.

Built with **Rust / Axum**, **PostgreSQL**, and **React / TypeScript**. Self-hosted, single-owner, with a dark interface inspired by manga collections.

## Quick start

Requires Docker Compose **2.24+** and Node.js to generate the configuration. Alternatively, copy `.env.example` to `.env` and fill in the required secrets manually.

```bash
node scripts/init-env.mjs
docker compose up -d --build
```

Open **http://localhost:8080** and sign in with `API_TOKEN` from `.env`. The generator creates random credentials with restricted file permissions and never overwrites an existing file.

The stack includes the application, PostgreSQL, **RSSHub with Chromium**, and Redis for RSSHub's cache. RSSHub is available locally at **http://localhost:1200**. Database and cache ports are not published.

1. Open **Settings** and save your Discord webhook URL.
2. Click **New feed**, choose Pawchive, Pixiv, or Bluesky, and enter the artist's ID or handle. Use **Custom — full URL** for other RSS/Atom feeds, including Xcancel.
3. Choose whether each feed should send notifications and download files automatically.
4. Browse posts in **Library**, manage connections in **Feeds**, and check deliveries in **Activity**.

The first sync imports existing posts **without notifications or automatic downloads**. Later syncs create jobs only for newly discovered post identifiers. Historical posts can be downloaded manually.

To test a webhook, open a post and click **Test on Discord**. This queues a delivery, including for historical posts or feeds with automatic notifications disabled. A saved webhook and enabled global notifications are required. Follow the result in **Activity**. Pending deliveries are not duplicated; completed or failed deliveries can be sent again.

## Features

- RSS and Atom parsing, including feeds provided by RSSHub.
- Per-feed polling from 5 minutes to 24 hours, pause/resume, and on-demand synchronization.
- Conditional requests using `ETag` and `Last-Modified`, persistent deduplication, and visible feed errors.
- Searchable, paginated library with source/feed filters, read status, and favorites.
- Multiple images and videos extracted from feed HTML, enclosures, and Media RSS.
- Discord messages with title, author, description, original link, date, up to four images, and video links. Mentions are disabled.
- Persistent job queue with retries, rate-limit handling, and manual recovery.
- Automatic or manual downloads with file-size limits, streaming, and authenticated access.
- Global controls for pausing downloads and notifications.
- Access-key authentication, HttpOnly cookies, URL validation, private-network restrictions, and redirect checks.

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
- The installation has one owner access key, not multiple user accounts.
- The webhook URL is stored in PostgreSQL and never returned by the API. Leave the UI field empty to preserve it; use the removal checkbox to delete it.
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
    worker.rs             Polling, queue, Discord, downloads
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

Further reference: [architecture](docs/architecture.md) · [HTTP API](docs/api.md).
