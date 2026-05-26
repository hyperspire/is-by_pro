#!/usr/bin/env sh
set -eu

HOST="is-by.pro"
PORT="443"
HOME_PATHNAME="/"
HOME_URL="https://${HOST}:${PORT}${HOME_PATHNAME}"
OAUTH_PATHNAME="/v1/auth/github"
OAUTH_URL="https://${HOST}:${PORT}${OAUTH_PATHNAME}"

# Baseline: site must be reachable.
curl --silent --show-error --fail --insecure --max-time 10 "$HOME_URL" >/dev/null

# OAuth start endpoint must redirect to GitHub with the expected callback URL.
OAUTH_HEADERS="$(curl --silent --show-error --insecure --max-time 10 -D - -o /dev/null "$OAUTH_URL")"

OAUTH_STATUS="$(printf "%s" "$OAUTH_HEADERS" | awk '/^HTTP\// { code=$2 } END { print code }')"
case "$OAUTH_STATUS" in
	302|303)
		;;
	*)
		echo "OAuth check failed: expected 302/303 from ${OAUTH_URL}, got ${OAUTH_STATUS:-<none>}" >&2
		exit 1
		;;
esac

OAUTH_LOCATION="$(
	printf "%s" "$OAUTH_HEADERS" \
		| grep -i '^location:' \
		| head -n 1 \
		| sed -e 's/\r$//' -e 's/^[Ll]ocation:[[:space:]]*//'
)"

printf "%s" "$OAUTH_LOCATION" | grep -F "https://github.com/login/oauth/authorize?" >/dev/null || {
	echo "OAuth check failed: redirect Location is not GitHub authorize URL" >&2
	exit 1
}

printf "%s" "$OAUTH_LOCATION" | grep -F "redirect_uri=https://is-by.pro/v1/auth/github/callback" >/dev/null || {
	echo "OAuth check failed: redirect_uri does not match expected callback URL" >&2
	exit 1
}

OAUTH_COOKIE="$(
	printf "%s" "$OAUTH_HEADERS" \
		| grep -i '^set-cookie:[[:space:]]*gh_oauth_state=' \
		| head -n 1 \
		| sed -e 's/\r$//'
)"

printf "%s" "$OAUTH_COOKIE" | grep -F "HttpOnly" >/dev/null || {
	echo "OAuth check failed: gh_oauth_state cookie missing HttpOnly flag" >&2
	exit 1
}

printf "%s" "$OAUTH_COOKIE" | grep -F "Secure" >/dev/null || {
	echo "OAuth check failed: gh_oauth_state cookie missing Secure flag" >&2
	exit 1
}
