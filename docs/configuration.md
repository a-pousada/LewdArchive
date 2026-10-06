# Configuration

The application reads process environment variables. Docker Compose loads `.env` automatically; native execution requires exporting its values before starting the backend. `.env.example` documents all supported settings. `node scripts/init-env.mjs` creates a populated copy with random credentials and never overwrites an existing configuration.

## Environment variables

| Variable                   | Description                                                                                                                                      |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `API_TOKEN`                | Required owner access key: at least 32 characters containing letters, numbers, hyphens, or underscores.                                          |
| `POSTGRES_PASSWORD`        | Required PostgreSQL password for Compose. Use hexadecimal values to avoid URL-encoding issues; the generator does this automatically.            |
| `DATABASE_URL`             | PostgreSQL connection URL for native execution. Compose supplies the internal connection string.                                                 |
| `BIND_ADDRESS`             | Native HTTP bind address; default `0.0.0.0:8080`. Docker uses this address inside the container and publishes it on host loopback.               |
| `DOWNLOAD_DIR`             | Native download directory; default `./data/downloads`. Docker uses `/data/downloads` on the persistent media volume.                             |
| `FRONTEND_DIR`             | Native static asset directory; default `frontend/dist`. Docker supplies `/app/frontend/dist`.                                                    |
| `MAX_DOWNLOAD_MB`          | Per-file limit in MiB; default 100, clamped to 1–2048. This is not a total storage quota.                                                        |
| `COOKIE_SECURE`            | Set to `true` when serving over HTTPS. Default `false` for local HTTP.                                                                           |
| `RSSHUB_BASE_URL`          | Default instance for new feed templates. Empty uses `http://rsshub:1200` in Compose or `https://rsshub.app` natively.                            |
| `ALLOW_PRIVATE_FEED_HOSTS` | Exact comma-separated private hostnames allowed for feed requests. Compose automatically adds `rsshub`. Does not permit private media downloads. |
| `RUST_LOG`                 | Log filter; default `lewdarchive=info,tower_http=info`.                                                                                          |

When configuring manually, copy `.env.example`, fill in both secrets, and replace `CHANGE_ME` in `DATABASE_URL` with the database password for native development. Never commit `.env` or `.env.rsshub`.

Changes to `.env` require recreating the application container:

```bash
docker compose up -d app
```

Changing `POSTGRES_PASSWORD` does not change the password of an already initialized PostgreSQL volume. Coordinate credential rotation with the database itself before updating application credentials.

## Docker Compose

`compose.yaml` is the standard Compose entry point. It provides:

| Service    | Host access      | Persistent volume                     |
| ---------- | ---------------- | ------------------------------------- |
| `app`      | `127.0.0.1:8080` | `media_data`                          |
| `postgres` | Internal only    | `postgres_data`                       |
| `rsshub`   | `127.0.0.1:1200` | None; configuration via `.env.rsshub` |
| `redis`    | Internal only    | `rsshub_cache`                        |

Services have health checks and restart policies. The application runs as a non-root user. The optional RSSHub environment file requires Compose 2.24 or newer.

For local backend development, the override publishes PostgreSQL on loopback:

```bash
docker compose -f compose.yaml -f compose.dev.yaml up -d postgres
```

## External or private RSSHub instances

Set `RSSHUB_BASE_URL` to an HTTP(S) base URL without credentials, query parameters, or fragments. A path prefix is allowed. Set any route-specific credentials in RSSHub, not in the LewdArchive feed URL.

For a private instance other than the bundled service, add its exact hostname to `ALLOW_PRIVATE_FEED_HOSTS`. The hostname must be resolvable and reachable from the application. Containers in different Compose projects need a shared network if they communicate through Docker hostnames.

This exception applies only to feeds. Private media URLs remain blocked. Public sources can use temporary links or reject hotlinking; failures are reported per feed or job, and the original post remains accessible through its link.

## Updates and logs

```bash
docker compose logs -f app
docker compose up -d --build
```

To update RSSHub:

```bash
docker compose pull rsshub
docker compose up -d rsshub
```

The backend applies embedded migrations at startup. Keep database and media backups before updates. Do not run `docker compose down -v` unless you intend to delete persistent data.

## Backups and retention

Back up PostgreSQL with `pg_dump` and copy the `media_data` volume. For a consistent database/media snapshot, pause the application while taking both backups. The Redis cache can be rebuilt.

Feed deletion cascades through posts, jobs, and download metadata, but leaves physical files in the media volume. Those files no longer appear in the interface. Cleanup and retention are operator-managed. Abrupt shutdowns can also leave incomplete or orphaned files.

The UI shows the latest 100 jobs and 200 downloads. The post library is paginated. Feed polling runs every 10 seconds and processes one batch at a time; large downloads can delay the next cycle.
