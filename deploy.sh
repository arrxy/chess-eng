#!/bin/bash
set -e

REPO_DIR="/root/server/chess-eng"
BINARY="/opt/chess/chess"
ENV_SRC="$REPO_DIR/.env"
ENV_DEST="/opt/chess/.env"

echo "==> Pulling latest code..."
cd "$REPO_DIR"
git pull

echo "==> Building frontend..."
cd "$REPO_DIR/frontend"
npm install --no-audit --no-fund
npm run build

echo "==> Building release..."
cd "$REPO_DIR"
cargo build --release

echo "==> Stopping service..."
systemctl stop chess

echo "==> Installing binary..."
cp target/release/chess "$BINARY"
chmod +x "$BINARY"

echo "==> Copying .env..."
cp "$ENV_SRC" "$ENV_DEST"

echo "==> Starting service..."
systemctl start chess
systemctl status chess --no-pager

echo "==> Done. Live at https://chess.aritro.me"
