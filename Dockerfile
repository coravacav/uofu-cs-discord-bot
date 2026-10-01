# syntax=docker/dockerfile:1.7
#
# Multi-stage build for the Kingfisher Discord bot.
# Runtime state (config.toml, .env, db/, debug.json, extracts/) is NOT baked in;
# docker-compose.yml bind-mounts the repo checkout at /app, which is the bot's cwd.

ARG RUST_VERSION=1.98
ARG DEBIAN_RELEASE=trixie

FROM rust:${RUST_VERSION}-slim-${DEBIAN_RELEASE} AS build
# The image's gcc builds the bundled SQLite and ring; OpenSSL is for Serenity's native-tls.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
# Cache mounts keep the cargo registry and target dir between builds, so only
# changed crates recompile.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/src/target,sharing=locked \
    cargo build --release --locked --bin bot \
    && cp target/release/bot /usr/local/bin/bot

FROM debian:${DEBIAN_RELEASE}-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3t64 tzdata sqlite3 \
    && rm -rf /var/lib/apt/lists/*
ARG UID=1000
ARG GID=1000
RUN groupadd --gid ${GID} bot && useradd --uid ${UID} --gid ${GID} --no-create-home --shell /usr/sbin/nologin bot
COPY --from=build /usr/local/bin/bot /usr/local/bin/bot
ENV TZ=America/Chicago \
    RUST_BACKTRACE=1
WORKDIR /app
USER bot
ENTRYPOINT ["/usr/local/bin/bot"]
