# syntax=docker/dockerfile:1

ARG CATALOG_ARCHIVE=catalog.tar.gz

FROM lukemathwalker/cargo-chef:0.1.78-rust-1.97-slim-trixie AS chef
WORKDIR /src

FROM chef AS planner
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates crates
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json --package barnacle-bot
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates crates
RUN cargo build --release --locked --package barnacle-bot \
 && strip target/release/barnacle-bot

FROM debian:trixie-slim AS runtime
ARG CATALOG_ARCHIVE
LABEL org.opencontainers.image.source="https://github.com/SatanshuMishra/barnacle"
LABEL org.opencontainers.image.description="Barnacle, a Discord bot for World of Warships players"
LABEL org.opencontainers.image.licenses="Apache-2.0"

RUN apt-get update \
 && apt-get install -y --no-install-recommends sqlite3 \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /var/lib/barnacle --shell /usr/sbin/nologin barnacle \
 && mkdir -p /var/lib/barnacle /etc/barnacle \
 && chown barnacle:barnacle /var/lib/barnacle /etc/barnacle

COPY --from=builder /src/target/release/barnacle-bot /usr/local/bin/barnacle-bot
COPY migrations /opt/barnacle/migrations
COPY curation /opt/barnacle/curation
COPY catalog.version /opt/barnacle/catalog/current
ADD "$CATALOG_ARCHIVE" /opt/barnacle/catalog/
COPY docker/entrypoint.sh /usr/local/bin/barnacle-entrypoint

RUN chmod 0755 /usr/local/bin/barnacle-entrypoint \
 && catalog="$(tr -d '[:space:]' < /opt/barnacle/catalog/current)" \
 && test -f "/opt/barnacle/catalog/$catalog/catalog.json" \
 && test -d "/opt/barnacle/catalog/$catalog/silhouettes" \
 && barnacle-bot --version

USER barnacle
WORKDIR /var/lib/barnacle
ENTRYPOINT ["/usr/local/bin/barnacle-entrypoint"]
