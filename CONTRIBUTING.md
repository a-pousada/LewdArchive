# Development

Use a current stable Rust toolchain, Node.js 22.12+ (24 is used in Docker and CI), and PostgreSQL 17+. Both Cargo and npm lockfiles are committed.

## Local setup

```bash
node scripts/init-env.mjs
docker compose -f compose.yaml -f compose.dev.yaml up -d postgres
npm ci --prefix frontend
npm run build --prefix frontend
set -a
source .env
set +a
cargo run --locked
```

Open http://localhost:8080. The Rust server serves the built frontend. For frontend development, run `npm run dev --prefix frontend` in another terminal; Vite opens port 5173 and proxies API requests to port 8080.

To use the bundled RSSHub from a native backend, start `rsshub` through Compose, set `RSSHUB_BASE_URL=http://localhost:1200`, and include `localhost` in `ALLOW_PRIVATE_FEED_HOSTS`. Containerized application instances use the internal hostname `rsshub` instead.

On NixOS, an ephemeral development environment can supply the tools:

```bash
nix --extra-experimental-features 'nix-command flakes' shell \
  nixpkgs#cargo nixpkgs#rustc nixpkgs#rustfmt nixpkgs#clippy \
  nixpkgs#postgresql nixpkgs#pkg-config nixpkgs#openssl nixpkgs#nodejs
```

## Checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm run format:check --prefix frontend
npm run build --prefix frontend
node --test scripts/init-env.test.mjs
```

Format frontend code with `npm run format --prefix frontend`. Use English for UI text, logs, errors, comments, tests, and documentation. Keep secrets, runtime data, dependencies, and build output out of Git.

## Integration and browser tests

Run a separate application instance against an **empty, disposable database**. Set `ALLOW_PRIVATE_FEED_HOSTS=127.0.0.1` for the fixture feed server, and set `API_TOKEN` to the same test-only key for the application and test processes. The browser tests also require a built frontend.

```bash
node scripts/integration.mjs
cd frontend
npx playwright install chromium
npm run test:e2e
```

Set `TEST_BASE_URL` to target another address (default `http://127.0.0.1:8080`). Set `CHROMIUM_PATH` to use an existing Chromium executable. Run test suites sequentially when sharing a database.

Integration checks cover authentication, feed CRUD, silent initial imports, conditional requests, new posts, deduplication, queue behavior, SSRF restrictions, favorites, read status, and search. Browser tests exercise desktop and mobile views, RSSHub templates, feed editing, and manual webhook feedback. Test fixtures are created and removed automatically; do not run these checks against production.

The manual Discord queue test additionally requires `psql` on `PATH` (or `PSQL_PATH`), a `DATABASE_URL` whose database name ends with `_test`, no existing feeds, and no saved webhook:

```bash
node scripts/test-discord.mjs
```

It holds the scheduler lock while checking configuration errors, paused notifications, concurrent requests, and replay of completed or failed jobs. Its fake webhook is never contacted. No real Discord webhook is needed for these tests.

## Structure and conventions

Keep API validation in `backend/src/api.rs`, feed normalization in `parser.rs`, HTTP restrictions in `network.rs`, and background processing in `worker.rs`. UI features live under `frontend/src/features`; reusable RSSHub route logic lives in `feedTemplates.ts`.

Database schema changes belong in new versioned migrations. Do not modify migrations already applied to an installation. Preserve public API contracts when changing frontend behavior, and update the reference documents when contracts change.
