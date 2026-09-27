# syntax=docker/dockerfile:1.7
#
# Multi-stage build for the Kingfisher Discord bot.
# Runtime state (config.toml, .env, db/, debug.json, extracts/) is NOT baked in;
# docker-compose.yml bind-mounts the repo checkout at /app, which is the bot's cwd.

ARG RUST_VERSION=1.98
ARG DEBIAN_RELEASE=trixie

FROM rust:${RUST_VERSION}-slim-${DEBIAN_RELEASE} AS build
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev clang libclang-dev g++ make cmake \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
# The Docker Desktop VM has ~8 GB RAM; compiling surrealdb-core with many
# parallel rustc jobs gets OOM-killed, so cap parallelism.
ARG CARGO_BUILD_JOBS=4
ENV CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}
COPY . .
# Cache mounts keep the cargo registry and target dir between builds, so only
# changed crates recompile (RocksDB etc. are built once).
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/src/target,sharing=locked \
    cargo build --release --locked --bin bot \
    && cp target/release/bot /usr/local/bin/bot

FROM debian:${DEBIAN_RELEASE}-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3t64 tzdata \
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
