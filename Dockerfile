# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
ARG CARGO_BUILD_JOBS=4
ENV CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release -p server && cp target/release/server /server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --no-create-home --shell /usr/sbin/nologin ragnarok \
    && mkdir -p /app /acme && chown 10001:10001 /acme
COPY --from=build /server /app/server
COPY client/dist /app/dist
# The built client holds the world (copied from client/public, or WORLD_DIR via scripts/deploy.sh).
COPY client/dist/world.msgpack /app/world.msgpack
COPY client/dist/tiles /app/tiles
ENV RAGNAROK_DIST=/app/dist \
    RAGNAROK_WORLD=/app/world.msgpack \
    RAGNAROK_ACME_CACHE=/acme
USER 10001
EXPOSE 80/tcp 443/tcp 443/udp
ENTRYPOINT ["/app/server"]
