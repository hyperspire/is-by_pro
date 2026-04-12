#!/usr/bin/env sh
set -eu

SERVICE_NAME="is-by_pro.service"

if [ "$(id -u)" -ne 0 ]; then
  echo "Please run as root: sudo $0"
  exit 1
fi

systemctl stop "$SERVICE_NAME"
systemctl disable "$SERVICE_NAME" || true
systemctl --no-pager --full status "$SERVICE_NAME" || true

echo "Service stopped: $SERVICE_NAME"
