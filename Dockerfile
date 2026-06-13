# Stage 1: Build the React frontend
FROM node:20-alpine AS frontend
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm ci
COPY frontend/ ./
# Outputs to ../static/dist (configured in vite.config.js)
RUN npm run build

# Stage 2: Build the Rust binary
FROM rust:1.88-slim AS builder
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy everything needed for the build
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
COPY tests/ ./tests/
# The `stress` workspace member must be present for cargo to load the
# workspace, but it is a dev/load-test tool and is not built into the image.
COPY stress/ ./stress/
# Frontend must be present before cargo build — embedded via include_str!
COPY --from=frontend /app/static/dist ./static/dist

# Build only the server binary; the stress crate is skipped.
RUN cargo build --release --bin chess

# Stage 3: Minimal runtime image
FROM debian:bookworm-slim
# ca-certificates required by reqwest for TLS (Google OAuth JWKS, MongoDB TLS)
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/chess /usr/local/bin/chess

EXPOSE 3000
CMD ["chess"]
