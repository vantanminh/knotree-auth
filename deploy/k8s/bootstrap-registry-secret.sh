#!/usr/bin/env bash
set -Eeuo pipefail

export KUBECONFIG="${KUBECONFIG:-/etc/rancher/k3s/k3s.yaml}"
source_namespace=knotree
target_namespace=knotree-accounts

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
    printf 'Enable and finish k3s Secret encryption before copying production registry credentials.\n' >&2
    exit 1
  }
kube() {
  k3s kubectl "$@"
}

kube apply -f "$(dirname "$0")/namespace.yaml" >/dev/null
if kube get secret registry-credentials -n "$target_namespace" >/dev/null 2>&1; then
  printf 'Secret %s/registry-credentials already exists; refusing to overwrite it.\n' "$target_namespace" >&2
  exit 1
fi

kube get secret registry-credentials -n "$source_namespace" -o json |
  python3 -c '
import json, sys
secret = json.load(sys.stdin)
if secret.get("type") != "kubernetes.io/dockerconfigjson" or ".dockerconfigjson" not in secret.get("data", {}):
    raise SystemExit("source registry secret is not a dockerconfigjson secret")
secret["metadata"] = {
    "name": "registry-credentials",
    "namespace": "knotree-accounts",
    "labels": {"app.kubernetes.io/part-of": "knotree-accounts"},
}
for key in ("stringData", "immutable"):
    secret.pop(key, None)
json.dump(secret, sys.stdout)
' |
  kube create -f - >/dev/null

printf 'Copied the existing GHCR pull secret into namespace %s without displaying its contents.\n' "$target_namespace"
