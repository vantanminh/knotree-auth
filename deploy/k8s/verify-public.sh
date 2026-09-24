#!/usr/bin/env bash
set -Eeuo pipefail

command -v curl >/dev/null || {
  printf 'missing required command: curl\n' >&2
  exit 1
}

for origin in https://accounts-api.knotree.com https://accounts.knotree.com; do
  for path in /health /ready; do
    status="$(curl --fail --silent --show-error --output /dev/null --write-out '%{http_code}' \
      --retry 5 --retry-all-errors --retry-delay 2 "$origin$path")"
    [[ "$status" == 200 ]] || {
      printf 'Public endpoint %s%s returned HTTP %s, expected 200.\n' "$origin" "$path" "$status" >&2
      exit 1
    }
  done
done

printf 'Both production hostnames return HTTP 200 for /health and /ready.\n'
