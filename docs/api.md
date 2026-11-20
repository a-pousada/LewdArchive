# HTTP API

Installation authentication uses `POST /api/login` with `{ "token": "..." }`, `POST /api/logout` and `GET /api/session`. The access key never grants a Discord identity. Community authentication uses separate OAuth sessions.

## Installation administration

All `/api/admin/*` endpoints require the installation administrator key or session.

- `GET /overview`: server, member, subscription, storage, comment and queue totals.
- `GET /servers`: connected server configuration summaries.
- `GET /services`: database connectivity, integration configuration and latest feed check.
- `GET /queues`: latest 100 bot, webhook and download jobs.
- `POST /queues/{kind}/{id}/retry`: retry a failed bot, webhook or download job.
- `GET/PUT /settings`: global `downloads_enabled` and `notifications_enabled` switches.

The former installation `/feeds`, `/entries`, `/webhooks`, `/downloads`, `/stats`, `/settings` and `/jobs` endpoints are removed and return 404. The root UI is an operational dashboard, not a personal archive.

## Community

All server routes begin `/api/community/servers/{server}`. Membership is checked live; management requires server ownership or Administrator permission.

- `GET/POST /artists`: list or track artists.
- `DELETE /artists/{artist}`: untrack for this server.
- `PATCH /artists/{artist}`: personal favorite/mention preferences.
- `PUT /artists/{artist}/profile`: server artist name, HTTPS photo and official links.
- `PUT /artists/{artist}/delivery`: optional `enabled` switch.
- `POST /artists/{artist}/sync`: request a feed refresh.
- `GET /posts`: paginated server library; personal favorites/likes filters.
- `PATCH /posts/{post}`: personal like/save preferences.
- `GET /trending`: ranking with `period=week|month|year`, `metric=likes|saves|engagement` and `page`.
- `GET /feeds/export`: server subscriptions as Miniflux-compatible OPML.
- `POST /feeds/import/preview`: `{ "opml": "..." }`, scoped duplicate detection.
- `POST /feeds/import`: OPML plus optional `enabled`, `poll_minutes`; existing shared feeds are reused without modifying other servers.
- `GET /posts/{post}/downloads`: stored files for a server post.
- `POST /posts/{post}/downloads`: administrator queues media storage.
- `GET /downloads/{id}/file`: member-authorized file download.
- `POST /posts/{post}/webhooks`: administrator queues a manual delivery to this server's enabled webhooks.

See [community management](community-management.md) for webhook CRUD, delivery activity, comments and privacy. Webhook secrets are never returned by list/create/update responses. Removed or inaccessible resources return 404; absent sessions return 401 and insufficient membership/administration returns 403.

## Categories and AI tags

See [configuration and delivery semantics](categories-and-ai.md).

- Artist creation accepts optional `category` (default empty). Profile updates accept `category`; omission preserves it. Labels are trimmed, case-sensitive and at most 500 characters.
- Post list/detail responses include `category`, `tags` and `tags_source` (`manual`, `ai`, or null). `GET /api/community/servers/{server}/posts` accepts optional `category` (empty matches uncategorized; omitted matches all) and `tag` (exact normalized tag), together with existing pagination/preferences.
- `GET /api/community/servers/{server}/categories`: member-only category names, artist counts and channel overrides, including suggested defaults.
- `PUT /api/community/servers/{server}/categories`: manager-only `{category, channel_id}`. A blank channel removes the override; otherwise the Discord channel must belong to this server.
- Webhook create/update accepts `category_filter`: null/omitted means all, empty means uncategorized only, otherwise an exact artist category. Responses include it. Manual sends respect the same filter.
- `GET /api/community/servers/{server}/ai`: manager-only `{enabled, images, provider:"wd", model}`. `enabled` means configured, not a live readiness probe. No endpoint URL is exposed.
- `POST /api/community/servers/{server}/posts/{post}/tags/suggest`: manager-only; no body required (`{}` accepted). Returns `{tags, saved:false}` from the first two images using local WD inference. 503 when disabled, 422 without images, 502 on inference failure. Suggestions do not persist. Legacy `images` fields are ignored; analysis is always image-based.
- `PUT /api/community/servers/{server}/posts/{post}/tags`: manager-only `{tags:string[]}`; returns normalized `{tags}`. An empty list clears tags and prevents late automatic inference from replacing that decision.
- Server settings accept optional `ai_tags_enabled`; omission preserves the flag. Enabling requires a configured provider. Default false. New-post classification analyzes images and does not backfill existing history. Text/video-only jobs finish without inference.
