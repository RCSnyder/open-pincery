# Stage 1: Build
FROM rust:1.88-bookworm AS builder

WORKDIR /app

# Disable fat LTO inside the Docker builder — default LTO + codegen-units=1
# from Cargo.toml peaks ~5GB of RAM during the final link, which OOM-kills
# rustc inside Docker Desktop's WSL2 VM (signal 7 / SIGBUS). The shipped
# release profile is still used by the tag-triggered release workflow;
# this env override only applies to local `docker compose build`.
ENV CARGO_PROFILE_RELEASE_LTO=off
ENV CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16

# Cache dependency builds
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs && echo "fn main() {}" > src/bin/pcy.rs
RUN cargo build --release && cargo clean -p open-pincery --release && rm -rf src

# Build actual source
COPY src/ src/
COPY migrations/ migrations/
COPY static/ static/
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Non-root runtime user (AC-22)
RUN groupadd --system --gid 10001 pcy \
 && useradd --system --uid 10001 --gid pcy --home-dir /app --shell /usr/sbin/nologin pcy

COPY --from=builder --chown=pcy:pcy /app/target/release/open-pincery /usr/local/bin/open-pincery
COPY --from=builder --chown=pcy:pcy /app/target/release/pcy /usr/local/bin/pcy
COPY --from=builder --chown=pcy:pcy /app/migrations /app/migrations
COPY --from=builder --chown=pcy:pcy /app/static /app/static

# Pre-create the pcy config dir so it's owned by the non-root runtime user
# when a named volume is mounted on top of it (see PCY_CONFIG_PATH in
# docker-compose.yml — used by the ./pcy wrapper for session persistence).
RUN mkdir -p /app/.pcy && chown -R pcy:pcy /app/.pcy

WORKDIR /app
USER pcy

ENV OPEN_PINCERY_HOST=0.0.0.0
ENV OPEN_PINCERY_PORT=8080

EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=3s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["open-pincery"]
