#!/usr/bin/env bash
set -Eeuo pipefail
script_dir="$(cd "$(dirname "$0")" && pwd)"
payload="${1:?CI runtime payload required}"
image="${2:?CI digest required}"
[[ "$image" =~ ^ghcr\.io/vantanminh/knotree-auth@sha256:[a-f0-9]{64}$ ]] || exit 1
python3 "$script_dir/runtime.py" apply "$payload"
export CI_RUNTIME_PAYLOAD="$payload"
export CI_RUNTIME_CHECKSUM
CI_RUNTIME_CHECKSUM="$(python3 "$script_dir/runtime.py" checksum "$payload")"
bash "$script_dir/../k8s/deploy.sh" "$image"
