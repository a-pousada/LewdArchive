# Feed templates

In **New feed**, choose a service. The form combines the RSSHub base URL, route, and artist identifier, previews the result, and sends the full URL and source to the existing feed API. Existing feed URLs are never rewritten.

| Service  | Required input                        | Generated route                                         |
| -------- | ------------------------------------- | ------------------------------------------------------- |
| Pawchive | Fanbox or Patreon + numeric artist ID | `/pawchive/fanbox/22445` or `/pawchive/patreon/22445`   |
| Pixiv    | Numeric artist ID                     | `/pixiv/user/15288095`                                  |
| Bluesky  | Full handle                           | `/bsky/profile/artist.bsky.social`                      |
| Custom   | Complete RSS/Atom URL + source label  | Preserves the supplied URL, including query parameters. |

RSSHub route implementations: [Pawchive](https://github.com/DIYgod/RSSHub/blob/master/lib/routes/pawchive/index.tsx), [Pixiv](https://github.com/DIYgod/RSSHub/blob/master/lib/routes/pixiv/user.ts), and [Bluesky](https://github.com/DIYgod/RSSHub/blob/master/lib/routes/bsky/posts.ts). Pixiv requires the appropriate credentials in RSSHub. Templates construct URLs; upstream configuration and availability still determine whether a feed can be fetched.

## Instance selection

The authenticated `GET /api/feed-options` endpoint returns the base configured by `RSSHUB_BASE_URL`. Compose defaults to `http://rsshub:1200`. Native execution defaults to `https://rsshub.app` when unset or empty.

The **RSSHub instance** field can override the base for a new feed without changing the installation default or other feeds. Path prefixes are preserved: `https://example.com/rsshub/` produces `https://example.com/rsshub/pawchive/fanbox/22445`.

Bases must use HTTP(S), with no credentials, query parameters, or fragments. Private instances other than the bundled `rsshub` hostname must also be permitted through `ALLOW_PRIVATE_FEED_HOSTS`.

Invalid identifiers prevent form submission. Bluesky handles accept an optional leading `@` and are normalized to lowercase. Numeric IDs are kept as strings.

## Custom feeds and maintenance

Choose **Custom — full URL** for Xcancel, unlisted RSSHub routes, or any other compatible RSS/Atom source. Select a source label to organize the library; labels do not transform the URL.

Route construction and validation live in `frontend/src/feedTemplates.ts`. Tests in `frontend/tests/feedTemplates.spec.ts` cover generated URLs, invalid inputs, persistence, and existing-feed editing on desktop and mobile.
