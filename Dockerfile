# syntax=docker/dockerfile:1
# The server is compiled on the build machine's own architecture and cross-compiled to the image's:
# rustc run under emulation (qemu on an Apple Silicon Mac) crashes.
FROM --platform=$BUILDPLATFORM rust:1-bookworm AS build
ARG TARGETARCH
RUN case "$TARGETARCH" in \
      amd64) triple=x86_64-unknown-linux-gnu gnu=x86-64-linux-gnu deb=amd64 ;; \
      arm64) triple=aarch64-unknown-linux-gnu gnu=aarch64-linux-gnu deb=arm64 ;; \
      *) echo "unsupported TARGETARCH $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && echo "$triple" > /target \
    && rustup target add "$triple" \
    && if [ "$(rustc -vV | sed -n 's/^host: //p')" != "$triple" ]; then \
         apt-get update && apt-get install -y --no-install-recommends cmake "gcc-$gnu" "g++-$gnu" "libc6-dev-$deb-cross" \
         && rm -rf /var/lib/apt/lists/*; \
       fi
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
ARG CARGO_BUILD_JOBS=4
ENV CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    triple=$(cat /target) && gnu=$(echo "${triple%%-unknown-*}-linux-gnu") && TRIPLE=$(echo "$triple" | tr a-z- A-Z_) \
    && if command -v "$gnu-gcc" >/dev/null; then \
         export "CARGO_TARGET_${TRIPLE}_LINKER=$gnu-gcc" "CC_$(echo "$triple" | tr - _)=$gnu-gcc" "CXX_$(echo "$triple" | tr - _)=$gnu-g++" "AR_$(echo "$triple" | tr - _)=$gnu-ar"; \
       fi \
    && cargo build --release -p server --target "$triple" && cp "target/$triple/release/server" /server

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
# Hand-placed spawns, vehicles and items (area.json) and the world's scenario (scenario.json, if it has
# one): scripts/deploy.sh fills .deploy/, and the server reads both next to world.msgpack.
COPY .deploy/ /app/
ENV RAGNAROK_DIST=/app/dist \
    RAGNAROK_WORLD=/app/world.msgpack \
    RAGNAROK_ACME_CACHE=/acme
USER 10001
EXPOSE 80/tcp 443/tcp 443/udp
ENTRYPOINT ["/app/server"]
