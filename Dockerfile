# syntax=docker/dockerfile:1

FROM node:24-alpine AS web
WORKDIR /web
COPY frontend/package*.json ./
RUN --mount=type=cache,target=/root/.npm npm ci
COPY frontend/ ./
RUN npm run build

FROM rust:1-bookworm AS server
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY backend/Cargo.toml ./backend/Cargo.toml
# Pre-build external dependencies so source edits under backend/src do not recompile all crates.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    mkdir -p backend/src \
    && printf 'fn main() {}\n' > backend/src/main.rs \
    && cargo build --release --locked \
    && rm -rf backend/src target/release/.fingerprint/lewdarchive-* target/release/lewdarchive*
COPY backend/ ./backend/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    touch backend/src/main.rs \
    && cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home archive
WORKDIR /app
COPY --from=server /build/target/release/lewdarchive /usr/local/bin/lewdarchive
COPY --from=web /web/dist ./frontend/dist
RUN mkdir -p /data/downloads && chown -R archive:archive /data /app
USER archive
ENV DOWNLOAD_DIR=/data/downloads \
    FRONTEND_DIR=/app/frontend/dist \
    BIND_ADDRESS=0.0.0.0:8080
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s CMD curl -fsS http://127.0.0.1:8080/health || exit 1
CMD ["lewdarchive"]
