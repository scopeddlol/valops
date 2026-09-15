# ---------------------------------------------------------------------------
# Stage 1 — build the single-page app
# ---------------------------------------------------------------------------
FROM node:22-alpine AS web

WORKDIR /web

# Install dependencies first so a source-only change reuses the npm layer.
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci

COPY frontend/ ./
RUN npm run build

# ---------------------------------------------------------------------------
# Stage 2 — build the Rust binary
# ---------------------------------------------------------------------------
FROM rust:1-bookworm AS server

WORKDIR /src

# Warm the dependency cache against a stub main, so editing application code
# does not trigger a full recompile of the dependency tree.
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -rf src

COPY backend/src ./src
COPY backend/migrations ./migrations
# Cargo skips rebuilding when mtimes look unchanged; touching main.rs forces it.
RUN touch src/main.rs && cargo build --release

# ---------------------------------------------------------------------------
# Stage 3 — runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# Run as a non-root user; the data volume is chowned to match.
RUN useradd --system --create-home --uid 10001 valops

WORKDIR /app
COPY --from=server /src/target/release/valops /usr/local/bin/valops
COPY --from=web /web/dist /app/static

RUN mkdir -p /app/data && chown -R valops:valops /app
USER valops

ENV VALOPS_DATABASE_URL=sqlite:///app/data/valops.db \
    VALOPS_STATIC_DIR=/app/static \
    VALOPS_SEED_DEMO=true \
    PORT=8080

EXPOSE 8080
VOLUME ["/app/data"]

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -fsS "http://127.0.0.1:${PORT}/api/health" || exit 1

CMD ["valops"]
