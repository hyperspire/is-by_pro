#!/usr/bin/env sh
set -eu

HOST="${HEALTHCHECK_HOST:-127.0.0.1}"
PORT="${HEALTHCHECK_PORT:-443}"
PATHNAME="${HEALTHCHECK_PATH:-/}"
URL="https://${HOST}:${PORT}${PATHNAME}"

curl --silent --show-error --fail --insecure --max-time 10 "$URL" >/dev/null
