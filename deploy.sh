#!/bin/bash
set -e

REMOTE_USER="root"
REMOTE_HOST="167.172.153.128"
REMOTE_REPO="/root/server/chess-eng"
REMOTE_PATH="/opt/chess/chess"
REMOTE_ENV="/opt/chess/.env"
ENV_FILE=".env"

echo "==> Copying .env to server..."
scp "$ENV_FILE" "$REMOTE_USER@$REMOTE_HOST:$REMOTE_ENV"

echo "==> Pushing code to server..."
rsync -az --exclude target --exclude .git \
    ./ "$REMOTE_USER@$REMOTE_HOST:$REMOTE_REPO/"

echo "==> Building on server..."
ssh "$REMOTE_USER@$REMOTE_HOST" "cd $REMOTE_REPO && cargo build --release"

echo "==> Stopping service..."
ssh "$REMOTE_USER@$REMOTE_HOST" "systemctl stop chess"

echo "==> Installing binary..."
ssh "$REMOTE_USER@$REMOTE_HOST" "cp $REMOTE_REPO/target/release/chess $REMOTE_PATH && chmod +x $REMOTE_PATH"

echo "==> Starting service..."
ssh "$REMOTE_USER@$REMOTE_HOST" "systemctl start chess && systemctl status chess --no-pager"

echo "==> Done. Live at https://chess.aritro.me"
