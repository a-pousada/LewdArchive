# Discord bot and community libraries

LewdArchive can run a Discord bot alongside its existing compatible webhooks. The community portal is available at `/community`. It uses Discord sign-in; members never need the installation access key.

## Set up the application

1. Create an application and bot in the [Discord Developer Portal](https://discord.com/developers/applications). Enable Guild Install with the `bot` and `applications.commands` scopes.
2. Fill in the optional variables in `.env`:
   - `DISCORD_BOT_TOKEN`: the bot token.
   - `DISCORD_APPLICATION_ID`: application ID.
   - `DISCORD_PUBLIC_KEY`: public key from General Information.
   - `DISCORD_CLIENT_SECRET`: OAuth2 client secret.
   - `PUBLIC_BASE_URL`: the public HTTPS origin, for example `https://archive.example.com`.
   - `COOKIE_SECURE=true` for HTTPS. Community configuration also enables secure cookies automatically when the public origin uses HTTPS.
3. In OAuth2, add this exact redirect URI: `https://archive.example.com/api/community/auth/callback`.
4. Deploy the updated app with `docker compose up -d --build`. Migration 0003 runs automatically and preserves the existing archive and webhook destinations.
5. Set the Interactions Endpoint URL to `https://archive.example.com/api/community/interactions`. Discord validates this endpoint with a signed ping. The reverse proxy must preserve the raw body and signature headers, and must not put an extra login screen in front of it.
6. Preview command registration locally:
   ```sh
   node scripts/register-discord-commands.mjs
   ```
   Then register the command using Node 22+:
   ```sh
   node --env-file=.env scripts/register-discord-commands.mjs --apply
   ```
   This upserts only `/archive`; other application commands are preserved. For development, set `DISCORD_TEST_GUILD_ID` to register in one server. Use `--nsfw` when the command should be age-restricted, and configure appropriate Discord channel restrictions for the content you track.
7. Open `/community`, sign in, invite the bot and connect your server. Choose an announcement channel in Server settings, or run `/archive setup channel:#art`.
8. Add artists on the website using the RSSHub builder or a custom feed URL, or run `/archive track name:Artist url:https://example.com/feed.xml`.

The bot needs **View Channel**, **Send Messages**, and **Embed Links** in the selected text or announcement channel. Server management actions require server ownership or **Administrator**. It does not need Administrator itself, Message Content intent, or a Gateway connection. It can appear offline in the member list because commands arrive through HTTP.

Leaving `DISCORD_BOT_TOKEN` empty disables the bot and OAuth login. The installation dashboard remains accessible. Bot credentials stay on the backend; do not put them in frontend variables.

## User experience and privacy

Each server has its own tracked artists, announcement channel, likes and preferences. The website library is available to **all current members of that server**, independently of the channel's permission overwrites. Choose the server accordingly; a private announcement channel does not make the website library private to that channel.

Users can:

- Like posts. Aggregate counts are visible to other members in that server; no list of people who liked a post is exposed.
- Save posts and favorite artists in personal collections. Collections and individual preferences are returned only for the authenticated user.
- Select **Notify me** for a specific artist in a specific server. Favoriting alone never enables mentions.
- Browse the same collections on the website or through Discord commands.

New-post embeds include title, author, source link, timestamp, up to four images and video links when available. Their buttons are **Like**, **Save post**, **Favorite artist**, and **Notify me**. Button responses are private and offer an undo action. All slash-command responses are also ephemeral, including favorite lists.

| Command | Purpose |
| --- | --- |
| `/archive setup channel` | Set this server's announcement channel; manager only |
| `/archive track name url` | Add a server artist from a feed; manager only |
| `/archive untrack artist` | Remove a server artist; manager only |
| `/archive trending [period] [metric] [page]` | Rank posts by likes, saves or combined engagement |
| `/archive latest [page]` | Browse posts and server like counts |
| `/archive artists [page]` | Browse artist IDs and your mention settings |
| `/archive favorites kind [page]` | Browse your saved posts or favorite artists |
| `/archive like post [enabled]` | Set or remove your like |
| `/archive favorite-post post [enabled]` | Save or unsave a post |
| `/archive favorite-artist artist [enabled]` | Favorite or unfavorite an artist |
| `/archive notify artist [enabled]` | Opt into or out of artist mentions |
| `/archive help` | Show command help |

Commands accept the IDs shown by `latest` and `artists`. The optional `enabled` argument defaults to true; use false to remove a preference. These are explicit operations, so retrying a command cannot accidentally toggle a preference twice.

Server lists are discovered during login. Sign out and back in to discover newly joined servers. Membership and management permissions are checked with Discord again whenever server data is accessed, so a cached login list never grants access by itself. If Discord cannot verify membership, access is denied until verification succeeds.

## Delivery and administration

The first feed sync imports existing history silently. New posts are queued once per server and post; opting into mentions does not replay older posts. Feed fetching is shared across servers using the same URL, but subscriptions and preferences are separate. A tracked source is currently one artist/feed connection; grouping several source accounts into a canonical artist is a possible future extension.

The worker snapshots opted-in users when a new post arrives and checks their preferences again before sending. Opting out removes pending mentions. An artist with more than 50 recipients gets multiple bounded batches. Each batch includes its own embed. Roles, `@everyone`, and `@here` are never allowed mentions. Discord ultimately controls whether an opted-in user receives a push notification.

Server notifications can be paused without deleting queued work. The installation-wide notification switch also pauses bot deliveries. Disabling server mentions suppresses mentions while still allowing artwork embeds. Queued deliveries retain their original channel; channel changes apply to future posts.

Delivery activity shows pending, completed and failed jobs. Failures retry with backoff, respect HTTP 429 retry delays, and stop after eight attempts. Managers can retry failed deliveries. If a bot loses channel permissions or is removed, deliveries fail visibly instead of moving to a different server or channel.

Delivery is **at least once**: stable Discord nonces reduce duplicates during short retries, but Discord's deduplication window does not guarantee exactly-once delivery after long outages or lost responses.

Untracking removes the server subscription, artist favorites, mention preferences and its delivery jobs. It leaves the shared feed and downloaded files for the installation administrator to manage, and does not affect other servers. Saved-post preferences are inaccessible while their source is untracked and become available if that same feed is tracked again. The installation owner is a trusted administrator with database access; “private” describes application visibility between members, not encryption against the operator.

The operational dashboard is at `/`. OPML transfer, manual post downloads and webhooks are server-scoped. The administrator controls global automation switches; feed ingestion and media storage are shared internally.

## Architecture and future providers

- `community/service.rs`: shared subscriptions and personal preferences, independent of Discord request payloads.
- `community/auth.rs`: Discord OAuth, one-use state and opaque user sessions.
- `community/api.rs`: member API with live server authorization.
- `community/interactions.rs`: signed Discord commands/buttons.
- `community/worker.rs`: durable command completion and bot deliveries.
- `platforms/mod.rs`: membership, notification rendering and transport contract.
- `platforms/discord.rs`: Discord REST adapter and embed rendering.

A future Fluxer bot needs its own authentication/command adapter and an implementation of the platform contract. Platform keys already namespace identities and servers. This release does not implement Fluxer bot login or commands; existing Fluxer-compatible webhooks remain supported.

Favorites and tenant restrictions are enforced by the application services and scoped SQL joins, not PostgreSQL RLS. Do not expose database credentials or the owner API key to members. OAuth access tokens are used only during sign-in and are not persisted. Session and OAuth state tokens are stored as SHA-256 hashes. Interaction tokens are retained only for deferred replies, cleared after success, and deleted after expiry.

## Validation and references

The PostgreSQL isolation test creates its own temporary schema in a dedicated test database:

```sh
COMMUNITY_TEST_DATABASE_URL=postgres://.../archive_test \
  cargo test --locked server_and_user_isolation -- --ignored
```

It exercises authenticated HTTP routes, cross-server access denial, manager checks, private favorites, public counts, shared ingestion, delivery deduplication and pending mention opt-outs. Browser tests cover the community portal on desktop and mobile. These tests use a mock transport and do not send real Discord messages.

Protocol references: [OAuth2](https://docs.discord.com/developers/topics/oauth2), [signed interactions and deferred replies](https://docs.discord.com/developers/interactions/receiving-and-responding), [application commands](https://docs.discord.com/developers/interactions/application-commands), and [message delivery](https://docs.discord.com/developers/resources/message).

## Trending and artist profiles

Open **Trending** in a server library or run:

```
/archive trending period:week metric:likes
/archive trending period:month metric:saves
/archive trending period:year metric:engagement
```

The optional period defaults to week; the metric defaults to likes. Windows are rolling **7, 30 and 365 days**, based on the post's publication timestamp, excluding future-dated posts. Counts reflect current likes and saves on those posts, including engagement that occurred outside the selected window; this is not a historical event/activity report. Removing a like or save updates the next ranking. Engagement is the unweighted sum of likes and saves.

Only posts with a positive value for the selected metric appear. Ties use newest publication time, then post ID, for deterministic ordering. Rankings are isolated by server and paginated (24 posts on the website, five per command). Save totals are aggregate counts; saver identities and other users' private collections are never returned. Slash replies remain ephemeral.

Server managers can choose **Artists → Edit profile** to change the display name, set a direct HTTPS photo URL or remove the photo, and add up to eight official links with custom labels. Links must use HTTPS and cannot contain URL credentials. These edits affect only this server's artist subscription, leaving the shared feed and other servers unchanged. Images are referenced remotely, not uploaded or downloaded to the server. Review that linked pages belong to the artist.

New bot messages use Unicode emoji buttons: ❤️ Like, 🔖 Save post, ⭐ Favorite artist and 🔔 Notify me. Save uses Discord's primary button style. Already posted messages are not edited retroactively.

Migration 0004 adds profile fields and ranking indexes. After updating the app, rerun `node --env-file=.env scripts/register-discord-commands.mjs --apply` to add the trending subcommand to Discord.
