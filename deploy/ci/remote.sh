#!/usr/bin/env bash
set -Eeuo pipefail
script_dir="$(cd "$(dirname "$0")" && pwd)"
payload="${1:?CI runtime payload required}"
image="${2:?CI digest required}"
[[ "$image" =~ ^ghcr\.io/vantanminh/knotree-auth@sha256:[a-f0-9]{64}$ ]] || exit 1
export KUBECONFIG=/etc/rancher/k3s/k3s.yaml
status="$(k3s secrets-encrypt status)"
grep -qx 'Encryption Status: Enabled' <<< "$status"
grep -qx 'Current Rotation Stage: reencrypt_finished' <<< "$status"
python3 "$script_dir/runtime.py" apply "$payload"
export CI_RUNTIME_CHECKSUM
CI_RUNTIME_CHECKSUM="$(python3 "$script_dir/runtime.py" checksum "$payload")"
bash "$script_dir/../k8s/deploy.sh" "$image"
