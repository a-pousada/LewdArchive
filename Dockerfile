FROM node:24-alpine AS web
WORKDIR /web
COPY frontend/package*.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

FROM rust:1-bookworm AS server
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY backend/ ./backend/
RUN cargo build --release --locked

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
