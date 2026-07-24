FROM rust:1.96-slim AS builder

WORKDIR /opt

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > build.rs

# Leverage BuildKit cache mounts to cache downloaded crates and compiled artifacts
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/opt/target \
    cargo build --release

# Copy the actual application source code
COPY src ./src
COPY build.rs ./

# Touch main.rs to force Cargo to notice the source code change
RUN touch src/main.rs && \
    touch build.rs

# Build the real binary using the cached dependencies
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/opt/target \
    cargo build --release && \
    cp target/release/tg2fa /opt/tg2fa


FROM debian:trixie-slim

WORKDIR /opt

RUN groupadd -g 10001 appuser && \
    useradd -u 10001 -g appuser -m -s /sbin/nologin appuser

RUN apt-get update &&  \
    apt-get install -y ca-certificates libssl3 &&  \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /opt/tg2fa .

RUN chown -R appuser:appuser /opt/tg2fa

USER appuser

# Telegram service
EXPOSE 8080
# Auth service
EXPOSE 9090

# Execute the application
CMD ["./tg2fa"]
