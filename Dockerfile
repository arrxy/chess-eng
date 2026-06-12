# Stage 1: Build the React frontend
FROM node:20-alpine AS frontend
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm ci
COPY frontend/ ./
# Outputs to ../static/dist (configured in vite.config.js)
RUN npm run build

# Stage 2: Build the Rust binary
FROM rust:1.85-slim AS builder
WORKDIR /app

# Cache dependencies — rebuild only when Cargo files change
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "fn main(){}" > src/main.rs \
    && cargo build --release \
    && rm src/main.rs

# Copy source and the built frontend (required before cargo build — embedded via include_str!)
COPY src/ ./src/
COPY tests/ ./tests/
COPY --from=frontend /app/static/dist ./static/dist

# Incremental build (touch main.rs so cargo detects the change)
RUN touch src/main.rs && cargo build --release

# Stage 3: Minimal runtime image
FROM debian:bookworm-slim
# ca-certificates required by reqwest for TLS (Google OAuth JWKS, MongoDB TLS)
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/chess /usr/local/bin/chess

EXPOSE 3000
CMD ["chess"]
