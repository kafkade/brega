# syntax=docker/dockerfile:1

# ── Builder ───────────────────────────────────────────────────────────
FROM rust:1.88-bookworm AS builder

WORKDIR /build
COPY . .

RUN cargo build --release -p brega-server \
    && strip target/release/brega-server

# ── Runtime ───────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Non-root user for the server process.
RUN useradd --system --create-home --shell /usr/sbin/nologin brega

COPY --from=builder /build/target/release/brega-server /usr/local/bin/brega-server
COPY --from=builder /build/crates/brega-server/LICENSE /usr/share/doc/brega-server/LICENSE

# Persistent data volume.
RUN mkdir -p /var/lib/brega-server && chown brega:brega /var/lib/brega-server
VOLUME /var/lib/brega-server

USER brega

ENV BREGA_BIND=0.0.0.0:8080
ENV BREGA_DATA_DIR=/var/lib/brega-server
ENV RUST_LOG=info

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD ["brega-server", "--help"]

ENTRYPOINT ["brega-server"]
