#!/bin/bash
set -e

REMOTE_USER="root"
REMOTE_HOST="167.172.153.128"
REMOTE_PATH="/opt/chess/chess"
REMOTE_ENV="/opt/chess/.env"
BINARY="target/release/chess"
ENV_FILE=".env"

echo "==> Building release..."
cargo build --release

echo "==> Copying binary and .env to server..."
scp "$BINARY" "$REMOTE_USER@$REMOTE_HOST:$REMOTE_PATH"
scp "$ENV_FILE" "$REMOTE_USER@$REMOTE_HOST:$REMOTE_ENV"

echo "==> Restarting service..."
ssh "$REMOTE_USER@$REMOTE_HOST" "systemctl restart chess && systemctl status chess --no-pager"

echo "==> Done. Live at https://chess.aritro.me"
