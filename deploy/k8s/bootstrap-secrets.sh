#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

export KUBECONFIG="${KUBECONFIG:-/etc/rancher/k3s/k3s.yaml}"
namespace=knotree-accounts
resend_file="${RESEND_API_KEY_FILE:-}"
private_dir="${PRIVATE_KEY_DIR:-/root/knotree-accounts-private}"

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

for command_name in k3s openssl stat grep; do
  command -v "$command_name" >/dev/null || fail "missing required command: $command_name"
done

encryption_status="$(k3s secrets-encrypt status)" || fail 'Could not check k3s secrets encryption status.'
grep -qx 'Encryption Status: Enabled' <<< "$encryption_status" || \
  fail 'Enable k3s secrets encryption before creating production application secrets.'
grep -qx 'Current Rotation Stage: reencrypt_finished' <<< "$encryption_status" || \
  fail 'Finish k3s secret encryption before creating production application secrets.'

[[ -n "$resend_file" && -f "$resend_file" && ! -L "$resend_file" ]] || \
  fail 'Set RESEND_API_KEY_FILE to a protected file containing the Resend production API key.'
mode="$(stat -c '%a' -- "$resend_file")"
[[ "$mode" == 400 || "$mode" == 600 ]] || \
  fail 'The Resend API key file must have mode 400 or 600.'
resend_key="$(cat -- "$resend_file")"
[[ -n "$resend_key" && "$resend_key" != *$'\n'* && "$resend_key" != *[[:space:]]* ]] || \
  fail 'The Resend API key file must contain one non-empty token with no whitespace.'

kube() {
  k3s kubectl "$@"
}

kube apply -f "$(dirname "$0")/namespace.yaml" >/dev/null
for secret_name in postgres-ca postgres-server-tls knotree-accounts-runtime; do
  if kube get secret "$secret_name" -n "$namespace" >/dev/null 2>&1; then
    fail "Secret $namespace/$secret_name already exists; refusing to rotate or overwrite production keys."
  fi
done

tmp="$(mktemp -d -t knotree-accounts-secrets.XXXXXX)"
trap 'rm -rf "$tmp"' EXIT
chmod 700 "$tmp"

[[ ! -L "$private_dir" ]] || fail 'PRIVATE_KEY_DIR must not be a symbolic link.'
mkdir -p -- "$private_dir"
chmod 700 -- "$private_dir"
for key_file in postgres-ca.key postgres-ca.crt jwt-private.pem totp-encryption-keys postgres-password; do
  [[ ! -e "$private_dir/$key_file" ]] || fail "Protected key file already exists at $private_dir/$key_file; refusing to overwrite it."
done

postgres_password="$(openssl rand -hex 32)"
totp_material="$(openssl rand -base64 32 | tr -d '\n')"
metrics_token="$(openssl rand -hex 32)"

printf '%s' "$postgres_password" > "$tmp/postgres-password"
printf 'postgresql://knotree_accounts:%s@postgres.knotree-accounts.svc.cluster.local:5432/knotree_accounts?sslmode=verify-full&sslrootcert=/run/secrets/postgres-ca/ca.crt' \
  "$postgres_password" > "$tmp/database-url"
printf '1:%s' "$totp_material" > "$tmp/totp-keys"
printf '%s' "$metrics_token" > "$tmp/metrics-token"
printf '%s' "$resend_key" > "$tmp/resend-api-key"
unset postgres_password totp_material metrics_token resend_key

openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:3072 -out "$tmp/jwt-private.pem" >/dev/null 2>&1
openssl req -x509 -newkey rsa:3072 -nodes -days 3650 -sha256 \
  -subj '/CN=Knotree Accounts PostgreSQL CA' \
  -addext 'basicConstraints=critical,CA:TRUE' \
  -addext 'keyUsage=critical,keyCertSign,cRLSign' \
  -keyout "$tmp/ca.key" -out "$tmp/ca.crt" >/dev/null 2>&1
openssl req -new -newkey rsa:3072 -nodes -sha256 \
  -subj '/CN=postgres.knotree-accounts.svc.cluster.local' \
  -keyout "$tmp/postgres.key" -out "$tmp/postgres.csr" >/dev/null 2>&1
cat > "$tmp/postgres.ext" <<'EOF'
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
subjectAltName=DNS:postgres,DNS:postgres.knotree-accounts,DNS:postgres.knotree-accounts.svc,DNS:postgres.knotree-accounts.svc.cluster.local
EOF
openssl x509 -req -in "$tmp/postgres.csr" -CA "$tmp/ca.crt" -CAkey "$tmp/ca.key" \
  -CAcreateserial -days 825 -sha256 -extfile "$tmp/postgres.ext" \
  -out "$tmp/postgres.crt" >/dev/null 2>&1
chmod 600 "$tmp"/*

install -m 600 "$tmp/ca.key" "$private_dir/postgres-ca.key"
install -m 600 "$tmp/ca.crt" "$private_dir/postgres-ca.crt"
install -m 600 "$tmp/jwt-private.pem" "$private_dir/jwt-private.pem"
install -m 600 "$tmp/totp-keys" "$private_dir/totp-encryption-keys"
install -m 600 "$tmp/postgres-password" "$private_dir/postgres-password"

kube create secret generic postgres-ca -n "$namespace" \
  --from-file=ca.crt="$tmp/ca.crt" >/dev/null
kube create secret generic postgres-server-tls -n "$namespace" \
  --from-file=ca.crt="$tmp/ca.crt" \
  --from-file=tls.crt="$tmp/postgres.crt" \
  --from-file=tls.key="$tmp/postgres.key" >/dev/null
kube create secret generic knotree-accounts-runtime -n "$namespace" \
  --from-file=POSTGRES_PASSWORD="$tmp/postgres-password" \
  --from-file=DATABASE_URL="$tmp/database-url" \
  --from-file=JWT_PRIVATE_KEY_PEM="$tmp/jwt-private.pem" \
  --from-file=TOTP_ENCRYPTION_KEYS="$tmp/totp-keys" \
  --from-file=RESEND_API_KEY="$tmp/resend-api-key" \
  --from-file=METRICS_TOKEN="$tmp/metrics-token" >/dev/null

printf 'Created production secrets in namespace %s. Secret values were not displayed.\n' "$namespace"
