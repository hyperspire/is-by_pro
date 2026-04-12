#!/usr/bin/env sh
set -eu

SERVICE_NAME="is-by_pro.service"
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO_DIR="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"

if [ "$(id -u)" -ne 0 ]; then
  echo "Please run as root: sudo $0"
  exit 1
fi

if [ ! -x "$REPO_DIR/target/release/is-by_pro" ]; then
  echo "Release binary not found. Building..."
  cargo build --release --manifest-path "$REPO_DIR/Cargo.toml"
fi

install -d -m 0755 /usr/local/bin
install -m 0755 "$REPO_DIR/target/release/is-by_pro" /usr/local/bin/is-by_pro

install -d -m 0755 /usr/local/bin/webroot
cp -a "$REPO_DIR/webroot/." /usr/local/bin/webroot/

install -d -m 0755 /usr/local/bin/ssl
cp -a "$REPO_DIR/ssl/." /usr/local/bin/ssl/

install -d -m 0755 /usr/local/bin/.env
cp -a "$REPO_DIR/.env/." /usr/local/bin/.env/

install -m 0644 "$REPO_DIR/systemd/$SERVICE_NAME" "/etc/systemd/system/$SERVICE_NAME"

systemctl daemon-reload
systemctl enable --now "$SERVICE_NAME"
systemctl --no-pager --full status "$SERVICE_NAME" || true

echo "Service started: $SERVICE_NAME"
