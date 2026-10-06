# HTTP API

Routes use the `/api` prefix unless stated otherwise. Responses are JSON; operational errors have the shape `{ "error": "message" }`. Axum's JSON and UUID extractors can also return 4xx validation responses. Protected routes accept a session cookie or `Authorization: Bearer <API_TOKEN>`.

| Method       | Route                      | Behavior                                                                          |
| ------------ | -------------------------- | --------------------------------------------------------------------------------- |
| GET          | `/health` (no prefix)      | Application and PostgreSQL health; public.                                        |
| POST         | `/login`                   | Accepts `{ "token": "..." }` and creates a session cookie.                        |
| POST         | `/logout`                  | Expires the session cookie.                                                       |
| GET          | `/session`                 | Verifies authentication.                                                          |
| GET / POST   | `/feeds`                   | Lists / creates feeds.                                                            |
| GET          | `/feed-options`            | Returns `{ "rsshub_base_url": "http://rsshub:1200" }` for the builder.            |
| PUT / DELETE | `/feeds/{uuid}`            | Updates / deletes a feed.                                                         |
| POST         | `/feeds/{uuid}/sync`       | Schedules a check for an enabled feed.                                            |
| POST         | `/sync`                    | Schedules all enabled feeds.                                                      |
| GET          | `/entries`                 | Paginated posts; filters: `q`, `feed_id`, `source`, `unread`, `favorite`, `page`. |
| PATCH        | `/entries/{uuid}`          | Accepts optional `is_read` and `favorite` booleans.                               |
| POST         | `/entries/read-all`        | Marks the entire library as read.                                                 |
| POST         | `/entries/{uuid}/download` | Queues a manual download; requires media.                                         |
| POST         | `/entries/{uuid}/discord`  | Queues a manual Discord delivery; see below.                                      |
| GET          | `/stats`                   | Feed, post, favorite, file, and queue totals.                                     |
| GET / PUT    | `/settings`                | Reads / updates global settings.                                                  |
| GET          | `/jobs`                    | Latest 100 jobs.                                                                  |
| POST         | `/jobs/{uuid}/retry`       | Resets a failed job for another attempt.                                          |
| GET          | `/downloads`               | Latest 200 downloaded files.                                                      |
| GET          | `/downloads/{uuid}/file`   | Downloads an authenticated attachment.                                            |

## Feeds

POST and PUT accept the same object:

```json
{
  "name": "My artist",
  "url": "https://rss.example.com/artist/feed",
  "source": "pixiv",
  "enabled": true,
  "auto_download": false,
  "notify": true,
  "poll_minutes": 15
}
```

Supported sources are `rss`, `pixiv`, `bluesky`, `xcancel`, and `pawchive`. The source is an organizational label; `url` must identify an RSS/Atom feed. Polling intervals range from 5 to 1440 minutes.

A feed URL is immutable after creation. Create another feed to change its identity. Duplicate URLs return 409. Deletion cascades through entries, jobs, and download metadata while preserving physical files.

## Manual Discord delivery

`POST /api/entries/{uuid}/discord` returns 202 with a `job_id` when queued. It uses the same worker, message formatting, and retries as automatic notifications. Per-feed notification settings do not prevent manual delivery.

- 400: no configured webhook or global notifications are paused.
- 404: entry does not exist.
- 409: a delivery for this entry is already pending or processing.

Completed or failed jobs can be queued again. A 202 response confirms queueing, not delivery to Discord. Read `/jobs` to follow the result.

## Settings

```json
{
  "downloads_enabled": true,
  "notifications_enabled": true,
  "webhook_url": "https://discord.com/api/webhooks/ID/TOKEN"
}
```

Omitting `webhook_url` preserves the existing secret. An empty string removes it. GET returns `webhook_configured`, never the URL. Global flags pause queue consumption without deleting jobs.

## Pagination

GET `/entries` returns `{ "items": [], "total": 0, "page": 1, "pages": 0 }`. Pages contain up to 24 entries, ordered by publication date and UUID descending. Search covers title and author; filters combine. Jobs and downloads are limited recent lists without pagination.
