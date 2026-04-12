#!/usr/bin/env sh
set -eu

HOST="is-by.pro"
PORT="443"
PATHNAME="/"
URL="https://${HOST}:${PORT}${PATHNAME}"

curl --silent --show-error --fail --insecure --max-time 10 "$URL" >/dev/null
