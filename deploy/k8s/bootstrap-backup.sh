#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

export KUBECONFIG="${KUBECONFIG:-/etc/rancher/k3s/k3s.yaml}"
namespace=knotree-accounts
backup_file="${BACKUP_ENV_FILE:-}"

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

for command_name in k3s stat grep; do
  command -v "$command_name" >/dev/null || fail "missing required command: $command_name"
done
encryption_status="$(k3s secrets-encrypt status)" || fail 'Could not check k3s secrets encryption status.'
grep -qx 'Encryption Status: Enabled' <<< "$encryption_status" && \
  grep -qx 'Current Rotation Stage: reencrypt_finished' <<< "$encryption_status" || \
  fail 'Enable and finish k3s Secret encryption before creating the R2 backup secret.'
[[ -n "$backup_file" && -f "$backup_file" && ! -L "$backup_file" ]] || \
  fail 'Set BACKUP_ENV_FILE to a protected file with R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY, R2_ENDPOINT_URL, and R2_BUCKET.'
mode="$(stat -c '%a' -- "$backup_file")"
[[ "$mode" == 400 || "$mode" == 600 ]] || fail 'The R2 credentials file must have mode 400 or 600.'

declare -A values=()
while IFS= read -r line || [[ -n "$line" ]]; do
  [[ -z "$line" || "$line" == \#* ]] && continue
  [[ "$line" == *=* ]] || fail 'Invalid R2 credentials file: expected KEY=VALUE lines.'
  key="${line%%=*}"
  value="${line#*=}"
  case "$key" in
    R2_ACCESS_KEY_ID|R2_SECRET_ACCESS_KEY|R2_ENDPOINT_URL|R2_BUCKET) ;;
    *) fail "Invalid R2 credentials file key: $key" ;;
  esac
  [[ -n "$value" && -z "${values[$key]+set}" ]] || fail "Missing or duplicate value for $key."
  [[ "$value" != *$'\n'* && "$value" != *$'\r'* ]] || fail "Invalid multiline value for $key."
  values[$key]="$value"
done < "$backup_file"

for key in R2_ACCESS_KEY_ID R2_SECRET_ACCESS_KEY R2_ENDPOINT_URL R2_BUCKET; do
  [[ -n "${values[$key]+set}" ]] || fail "R2 credentials file is missing $key."
done
[[ "${values[R2_ENDPOINT_URL]}" =~ ^https://[A-Za-z0-9.-]+(:[0-9]+)?$ ]] || fail 'R2_ENDPOINT_URL must be an HTTPS endpoint without a path.'
[[ "${values[R2_BUCKET]}" =~ ^[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]$ ]] || fail 'R2_BUCKET is not a valid bucket name.'

kube() {
  k3s kubectl "$@"
}
kube get namespace "$namespace" >/dev/null
if kube get secret knotree-accounts-backup -n "$namespace" >/dev/null 2>&1; then
  fail "Secret $namespace/knotree-accounts-backup already exists; refusing to overwrite it."
fi

tmp="$(mktemp -d -t knotree-accounts-backup-secret.XXXXXX)"
trap 'unset values; rm -rf "$tmp"' EXIT
for key in R2_ACCESS_KEY_ID R2_SECRET_ACCESS_KEY R2_ENDPOINT_URL R2_BUCKET; do
  printf '%s' "${values[$key]}" > "$tmp/$key"
  chmod 600 "$tmp/$key"
done
unset values
kube create secret generic knotree-accounts-backup -n "$namespace" \
  --from-file=R2_ACCESS_KEY_ID="$tmp/R2_ACCESS_KEY_ID" \
  --from-file=R2_SECRET_ACCESS_KEY="$tmp/R2_SECRET_ACCESS_KEY" \
  --from-file=R2_ENDPOINT_URL="$tmp/R2_ENDPOINT_URL" \
  --from-file=R2_BUCKET="$tmp/R2_BUCKET" >/dev/null

printf 'Created the dedicated R2 backup secret in namespace %s. Its values were not displayed.\n' "$namespace"
