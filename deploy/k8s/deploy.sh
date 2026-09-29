#!/usr/bin/env bash
set -Eeuo pipefail

export KUBECONFIG="${KUBECONFIG:-/etc/rancher/k3s/k3s.yaml}"
namespace=knotree-accounts
script_dir="$(cd "$(dirname "$0")" && pwd)"

if [[ $# -ne 1 || ! "$1" =~ ^ghcr\.io/vantanminh/knotree-auth@sha256:[a-f0-9]{64}$ ]]; then
  printf 'Usage: %s ghcr.io/vantanminh/knotree-auth@sha256:<64 lowercase hex chars>\n' "$0" >&2
  exit 2
fi
image="$1"

for command_name in k3s python3 grep; do
  command -v "$command_name" >/dev/null || {
    printf 'missing required command: %s\n' "$command_name" >&2
    exit 1
  }
done
encryption_status="$(k3s secrets-encrypt status)" || {
  printf 'Could not check k3s secrets encryption status.\n' >&2
  exit 1
}
grep -qx 'Encryption Status: Enabled' <<< "$encryption_status" && \
  grep -qx 'Current Rotation Stage: reencrypt_finished' <<< "$encryption_status" || {
    printf 'Enable and finish k3s Secret encryption before deploying this production workload.\n' >&2
    exit 1
  }
kube() {
  k3s kubectl "$@"
}

check_secret() {
  local name="$1"
  shift
  kube get secret "$name" -n "$namespace" -o json |
    python3 -c '
import json, sys
secret = json.load(sys.stdin)
required = set(sys.argv[1:])
missing = required - set(secret.get("data", {}))
if missing:
    raise SystemExit("required key missing from Kubernetes Secret: " + ", ".join(sorted(missing)))
' "$@"
}

# The namespace and secrets are provisioned explicitly by the bootstrap scripts.
kube get namespace "$namespace" >/dev/null
check_secret knotree-accounts-runtime POSTGRES_PASSWORD DATABASE_URL JWT_PRIVATE_KEY_PEM TOTP_ENCRYPTION_KEYS CLOUDFLARE_ACCOUNT_ID CLOUDFLARE_EMAIL_API_TOKEN METRICS_TOKEN
check_secret postgres-ca ca.crt
check_secret postgres-server-tls ca.crt tls.crt tls.key
check_secret registry-credentials .dockerconfigjson
check_secret knotree-accounts-backup R2_ACCESS_KEY_ID R2_SECRET_ACCESS_KEY R2_ENDPOINT_URL R2_BUCKET
active_backups="$(kube get jobs -n "$namespace" -l app.kubernetes.io/name=knotree-accounts-backup -o json | python3 -c '
import json, sys
jobs = json.load(sys.stdin).get("items", [])
print(sum(1 for job in jobs if job.get("status", {}).get("active", 0) > 0))
')"
[[ "$active_backups" == 0 ]] || {
  printf 'An offsite database backup is already running; wait for it to finish before deploying.\n' >&2
  exit 1
}

ready_nodes="$(kube get nodes -o json | python3 -c '
import json, sys
data = json.load(sys.stdin)
ready = [node["metadata"]["name"] for node in data.get("items", [])
         if any(c.get("type") == "Ready" and c.get("status") == "True" for c in node.get("status", {}).get("conditions", []))
         and not node.get("spec", {}).get("unschedulable", False)]
if not ready:
    raise SystemExit("no schedulable Ready Kubernetes nodes")
print("\n".join(ready))
')"
printf 'Schedulable Ready node(s): %s\n' "$(printf '%s' "$ready_nodes" | tr '\n' ' ')"

test -n "${CI_RUNTIME_CHECKSUM:?Run deployment through GitHub CI runtime provisioning}"
kube apply -f "$script_dir/network-policy.yaml" >/dev/null
kube apply -f "$script_dir/postgres.yaml" >/dev/null
kube apply -f "$script_dir/availability.yaml" >/dev/null
kube apply -f "$script_dir/backup.yaml" >/dev/null

tmp="$(mktemp -d -t knotree-accounts-deploy.XXXXXX)"
trap 'rm -rf "$tmp"' EXIT
python3 - "$script_dir/api.yaml" "$tmp/api.yaml" "$image" <<'PY'
from pathlib import Path
import sys

import os
source = Path(sys.argv[1]).read_text(encoding="utf-8")
source = source.replace("  template:\n    metadata:\n", "  template:\n    metadata:\n      annotations:\n        github-runtime/checksum: " + os.environ["CI_RUNTIME_CHECKSUM"] + "\n", 1)
marker = "__KNOTREE_ACCOUNTS_IMAGE__"
if source.count(marker) != 1:
    raise SystemExit("API manifest must contain exactly one image placeholder")
Path(sys.argv[2]).write_text(source.replace(marker, sys.argv[3]), encoding="utf-8")
PY
kube apply -f "$tmp/api.yaml" >/dev/null

kube rollout status statefulset/postgres -n "$namespace" --timeout=600s
kube rollout status deployment/knotree-accounts -n "$namespace" --timeout=600s
backup_job="knotree-accounts-backup-initial-$(date -u +%Y%m%d%H%M%S)"
kube create job "$backup_job" -n "$namespace" --from=cronjob/knotree-accounts-postgres-backup >/dev/null
backup_deadline=$((SECONDS + 3600))
while (( SECONDS < backup_deadline )); do
  backup_state="$(kube get job "$backup_job" -n "$namespace" -o json | python3 -c '
import json, sys
job = json.load(sys.stdin)
conditions = job.get("status", {}).get("conditions", [])
if any(c.get("type") == "Complete" and c.get("status") == "True" for c in conditions):
    print("complete")
elif any(c.get("type") == "Failed" and c.get("status") == "True" for c in conditions):
    print("failed")
else:
    print("running")
')"
  case "$backup_state" in
    complete) break ;;
    failed)
      printf 'Initial PostgreSQL backup or R2 upload failed (job %s); Kong was not changed.\n' "$backup_job" >&2
      exit 1
      ;;
    running) sleep 5 ;;
  esac
done
[[ "$backup_state" == complete ]] || {
  printf 'Timed out waiting for initial PostgreSQL backup job %s; Kong was not changed.\n' "$backup_job" >&2
  exit 1
}
kube patch cronjob knotree-accounts-postgres-backup -n "$namespace" \
  --type=merge -p '{"spec":{"suspend":false}}' >/dev/null
"$script_dir/configure-kong.sh"

printf 'Knotree Accounts API deployed from %s; Postgres backup is enabled and Kong is routing accounts-api.knotree.com.\n' "$image"
