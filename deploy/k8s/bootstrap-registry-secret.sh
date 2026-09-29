#!/usr/bin/env bash
set -Eeuo pipefail
echo 'Manual server provisioning is disabled. Set GitHub Actions K8S_CONFIG_JSON/K8S_SECRETS_JSON and run production CI.' >&2
exit 1
