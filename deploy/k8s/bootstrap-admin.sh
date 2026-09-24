#!/usr/bin/env bash
set -Eeuo pipefail

export KUBECONFIG="${KUBECONFIG:-/etc/rancher/k3s/k3s.yaml}"
namespace=knotree-accounts

if [[ $# -ne 1 || ! "$1" =~ ^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-8][0-9a-fA-F]{3}-[89aAbB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}$ ]]; then
  printf 'Usage: %s <verified-user-uuid>\n' "$0" >&2
  exit 2
fi
command -v k3s >/dev/null || {
  printf 'missing required command: k3s\n' >&2
  exit 1
}
kube() {
  k3s kubectl "$@"
}

kube get deployment knotree-accounts -n "$namespace" >/dev/null
kube rollout status deployment/knotree-accounts -n "$namespace" --timeout=180s >/dev/null
query="SELECT EXISTS (SELECT 1 FROM users u JOIN user_emails e ON e.user_id = u.id WHERE u.id = '$1'::uuid AND u.status = 'active' AND e.verified_at IS NOT NULL)"
verified="$(kube exec -n "$namespace" postgres-0 -c postgres -- /bin/sh -ec '
password="$(cat /run/secrets/postgres/password)"
PGPASSWORD="$password" psql -h 127.0.0.1 -U knotree_accounts -d knotree_accounts -tAc "$1"
' sh "$query")"
[[ "$verified" == t ]] || {
  printf 'The supplied UUID does not belong to a user with a verified email.\n' >&2
  exit 1
}
if kube get configmap knotree-accounts-admin -n "$namespace" >/dev/null 2>&1; then
  printf 'The super-admin bootstrap ConfigMap already exists; refusing to replace the configured admin.\n' >&2
  exit 1
fi
kube create configmap knotree-accounts-admin -n "$namespace" \
  --from-literal=SUPER_ADMIN_USER_ID="$1" >/dev/null
kube rollout restart deployment/knotree-accounts -n "$namespace" >/dev/null
kube rollout status deployment/knotree-accounts -n "$namespace" --timeout=600s
printf 'Configured the verified user as the production super-admin.\n'
