#!/usr/bin/env bash
set -Eeuo pipefail

export KUBECONFIG="${KUBECONFIG:-/etc/rancher/k3s/k3s.yaml}"
script_dir="$(cd "$(dirname "$0")" && pwd)"
fragment="$script_dir/kong-accounts-route.fragment.yml"
temp_dir="$(mktemp -d -t knotree-accounts-kong.XXXXXX)"

cleanup() {
  rm -rf "$temp_dir"
}
trap cleanup EXIT

for command_name in k3s python3 awk; do
  command -v "$command_name" >/dev/null || {
    printf 'missing required command: %s\n' "$command_name" >&2
    exit 1
  }
done
python3 -c 'import yaml' >/dev/null 2>&1 || {
  printf 'python3-yaml is required to validate Kong declarative configuration\n' >&2
  exit 1
}

kube() {
  k3s kubectl "$@"
}

namespace=knotree-accounts
kube get service knotree-accounts -n "$namespace" >/dev/null
kube get endpointslice -n "$namespace" \
  -l kubernetes.io/service-name=knotree-accounts \
  -o json |
  python3 -c 'import json,sys; d=json.load(sys.stdin); sys.exit(0 if any(e.get("addresses") for i in d.get("items",[]) for e in i.get("endpoints",[]) if e.get("conditions",{}).get("ready",True)) else 1)' || {
    printf 'Knotree Accounts API has no ready endpoints; refusing to change Kong\n' >&2
    exit 1
  }

kube get configmap kong-dbless-config -n kong \
  -o jsonpath='{.data.kong\.yml}' > "$temp_dir/current.yml"
test -s "$temp_dir/current.yml" || {
  printf 'Kong declarative config is empty; refusing to change it\n' >&2
  exit 1
}

route_state="$(python3 - "$temp_dir/current.yml" <<'PY'
import sys
import yaml

with open(sys.argv[1], encoding="utf-8") as source:
    config = yaml.safe_load(source)
if not isinstance(config, dict) or not isinstance(config.get("services"), list) or "consumers" not in config:
    raise SystemExit("Kong declarative config must contain top-level services and consumers")
services = config["services"]
matches = [service for service in services if service.get("name") == "knotree-accounts-api"]
named_routes = [route for service in services for route in service.get("routes", [])
               if route.get("name") == "knotree-accounts-api-origin"]
host_routes = [route for service in services for route in service.get("routes", [])
              if "accounts-api.knotree.com" in route.get("hosts", [])]
if not matches and not named_routes and not host_routes:
    print("insert")
    raise SystemExit(0)
if len(matches) == 1 and len(named_routes) == 1 and len(host_routes) == 1:
    service = matches[0]
    route = named_routes[0]
    if (service.get("url") == "http://knotree-accounts.knotree-accounts.svc.cluster.local:80"
            and route in service.get("routes", [])
            and route.get("hosts") == ["accounts-api.knotree.com"]
            and route.get("paths") == ["/"]):
        print("present")
        raise SystemExit(0)
raise SystemExit("Kong already has a conflicting Knotree Accounts service or route")
PY
)"
if [[ "$route_state" == present ]]; then
  printf 'Kong already contains the Knotree Accounts API route\n'
  exit 0
fi

awk -v fragment="$fragment" '
  $0 == "consumers:" {
    while ((getline line < fragment) > 0) print line
    close(fragment)
  }
  { print }
' "$temp_dir/current.yml" > "$temp_dir/updated.yml"

python3 - "$temp_dir/updated.yml" <<'PY'
import sys
import yaml

with open(sys.argv[1], encoding="utf-8") as source:
    config = yaml.safe_load(source)

services = config.get("services", [])
matches = [s for s in services if s.get("name") == "knotree-accounts-api"]
assert len(matches) == 1, "expected exactly one Knotree Accounts Kong service"
service = matches[0]
assert service.get("url") == "http://knotree-accounts.knotree-accounts.svc.cluster.local:80"
routes = [r for r in service.get("routes", []) if r.get("name") == "knotree-accounts-api-origin"]
assert len(routes) == 1, "expected exactly one Knotree Accounts API route"
assert routes[0].get("hosts") == ["accounts-api.knotree.com"]
assert routes[0].get("paths") == ["/"]
PY

kube create configmap kong-dbless-config -n kong \
  --from-file=kong.yml="$temp_dir/updated.yml" \
  --dry-run=client -o yaml |
  kube apply -f - >/dev/null

kube rollout restart deployment/kong-kong -n kong >/dev/null
kube rollout status deployment/kong-kong -n kong --timeout=180s
printf 'Kong now routes accounts-api.knotree.com to %s/knotree-accounts\n' "$namespace"
