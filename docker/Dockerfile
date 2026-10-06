FROM rust:1-bookworm AS build
WORKDIR /src
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
COPY backend/migrations ./migrations
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 --home /nonexistent --shell /usr/sbin/nologin knotree \
    && apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /data/client-logos \
    && chown -R knotree /data
COPY --from=build /src/target/release/knotree-accounts /usr/local/bin/knotree-accounts
USER knotree
ENV CLIENT_LOGO_DIR=/data/client-logos
VOLUME /data/client-logos
EXPOSE 8080
STOPSIGNAL SIGINT
ENTRYPOINT ["knotree-accounts"]
