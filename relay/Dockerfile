# Whoosh relay image. Build from the relay/ folder:   docker build -t whoosh-relay relay/
# Run:  docker run -p 8080:8080 -v relay-data:/data --env-file relay/.env whoosh-relay
# The build context is relay/ alone: this folder has its own Cargo.lock and reads nothing outside itself.
# Nothing here names a host, key or account: every value comes from the environment (relay/README.md).

FROM rust:1-bookworm AS build
# cmake is needed by the TLS library's C build; the SQLite build uses the compiler already in this image.
RUN apt-get update && apt-get install -y --no-install-recommends cmake && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/* \
    && useradd --system --home /data --shell /usr/sbin/nologin relay && mkdir -p /data && chown relay /data
COPY --from=build /src/target/release/relay /usr/local/bin/relay
USER relay
# Listen on every interface inside the container and keep the database on the /data volume.
# Put a TLS-terminating proxy or your platform's HTTPS edge in front: the relay speaks plain HTTP.
ENV RELAY_BIND=0.0.0.0:8080 RELAY_DB=/data/relay.sqlite3
VOLUME ["/data"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s CMD curl -fsS http://127.0.0.1:8080/v1/health >/dev/null || exit 1
ENTRYPOINT ["/usr/local/bin/relay"]
