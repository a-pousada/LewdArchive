# Administration and community discussion

The installation access key (`API_TOKEN`, stored in the owner's `la_session` cookie or supplied as a bearer token) protects the root dashboard and all installation APIs. A Discord session (`la_user`) does not grant installation access, even for a server owner. Only the installation administrator controls global settings, polling policy, storage and installation statistics. Keep this key private.

Server owners and members with Administrator permission manage their server under `/community`: track/untrack feeds, edit artist profiles, request a feed refresh, pause an artist's announcements, configure the bot channel and manage additional webhooks. Manage Server alone is insufficient. Membership and administrator permissions are checked on every request. Pausing artist announcements keeps the shared feed and library intact and does not change another server's settings.

## Webhooks

Leave the bot channel blank to use webhooks only. The server announcement switch pauses both transports.

Server webhooks are scoped by `webhooks.community_id`; NULL is reserved for inactive historical installation destinations; these are no longer exposed or delivered. URLs are never serialized to the browser. Each server has independent destination names, message identities, colors, pause controls and delivery history. Omitting a URL on edit preserves the secret. The same destination URL can be used in independent scopes.

New posts enqueue deliveries only for enabled artist subscriptions and enabled destinations in that server. Initial history remains silent. The existing durable job queue provides retries and rate-limit backoff; the global notification switch remains a master pause. Worker claims recheck the server and subscription switches. Untracked artists cannot have pending webhook jobs delivered. Administrators can retry failed jobs from Delivery activity.

Server destinations must use public HTTPS endpoints. Delivery resolves and pins public IP addresses, rejects private/reserved addresses, disables proxies and redirects, and strips URLs from network errors. Historical installation destinations are disabled during migration and ignored by the worker.

## Comments

Comments belong to a server, subscribed artist and post. The same RSS post in two servers has two independent discussions. Every read and write checks current membership and that the post belongs to the server. Comments contain 1–2,000 characters, are rendered as plain text, and appear newest first in pages of 30. Posting is limited to five retained comments per minute per author across servers, with serialized author writes. Authors can delete their comments; server administrators can moderate any comment. Removal is permanent and requires an interface confirmation.

The author is taken exclusively from the authenticated session. Name, avatar and provider come from `community_users`, not request fields. The discussion module uses the platform-neutral membership boundary; adding a Fluxer authentication/identity adapter does not require a Discord-specific comment table. Fluxer sign-in itself is not implemented by this change. Comments are available from library cards and Trending; no comment notifications or external message fanout are sent.

## API

All routes below are under `/api/community/servers/{server}`.

- `GET/POST /webhooks`, `PUT/DELETE /webhooks/{id}`: administrators only.
- `GET /webhook-deliveries`, `POST /webhook-deliveries/{id}/retry`: scoped activity and failed delivery retry.
- `PUT /artists/{artist}/delivery`: `{ "enabled": false }` pauses server announcements for an artist.
- `POST /artists/{artist}/sync`: request a refresh, subject to installation polling being enabled.
- `GET /posts/{post}/comments?page=1`: comments, author identities and per-item `can_delete`.
- `POST /posts/{post}/comments`: `{ "body": "A comment" }`; authenticated author only.
- `DELETE /posts/{post}/comments/{id}`: author or server administrator only.

## Post pages and shared links

Post titles and cover images open `/community?server={server_uuid}&post={post_uuid}`. The page displays every media attachment, personal like/save actions, files and the server conversation below the gallery. Share post copies this service URL; if clipboard access is unavailable, a selectable link is shown. Opening a link does not grant membership. The single-post API checks current membership and the server subscription before returning content. The login flow remembers the selected post in tab-local session storage and restores it after OAuth; browsers disabling storage can reopen the shared URL after signing in.
