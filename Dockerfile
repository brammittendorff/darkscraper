# Stage 1: Build
FROM rust:1.93-bookworm AS builder

# Install build dependencies (cmake/clang for BoringSSL used by rquest)
RUN apt-get update && apt-get install -y \
    cmake \
    pkg-config \
    libssl-dev \
    perl \
    nasm \
    golang-go \
    clang \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock* ./
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/networks/Cargo.toml crates/networks/Cargo.toml
COPY crates/parser/Cargo.toml crates/parser/Cargo.toml
COPY crates/storage/Cargo.toml crates/storage/Cargo.toml
COPY crates/frontier/Cargo.toml crates/frontier/Cargo.toml
COPY crates/search/Cargo.toml crates/search/Cargo.toml
COPY crates/discovery/Cargo.toml crates/discovery/Cargo.toml

# Create dummy source files to pre-compile dependencies
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs \
    && mkdir -p crates/core/src && touch crates/core/src/lib.rs \
    && mkdir -p crates/networks/src && touch crates/networks/src/lib.rs \
    && mkdir -p crates/parser/src && touch crates/parser/src/lib.rs \
    && mkdir -p crates/storage/src && touch crates/storage/src/lib.rs \
    && mkdir -p crates/storage/migrations && touch crates/storage/migrations/001_init.sql \
    && mkdir -p crates/frontier/src && touch crates/frontier/src/lib.rs \
    && mkdir -p crates/search/src && touch crates/search/src/lib.rs \
    && mkdir -p crates/discovery/src && touch crates/discovery/src/lib.rs

# Build dependencies only (this layer is cached)
RUN cargo build --release 2>/dev/null || true

# Copy actual source code
COPY src/ src/
COPY crates/ crates/
COPY config/ config/

# Force cargo to recompile our crates (not deps) by touching source files
RUN find src/ crates/ -name '*.rs' -exec touch {} + \
    && cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/darkscraper /usr/local/bin/darkscraper
COPY config/default.toml /etc/darkscraper/default.toml

RUN mkdir -p /data

ENTRYPOINT ["darkscraper"]
CMD ["--help"]
