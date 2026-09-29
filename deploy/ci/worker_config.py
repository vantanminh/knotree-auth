#!/usr/bin/env python3
"""Validate Worker settings supplied by GitHub; no file/server fallback."""
import json
import os
from pathlib import Path
import re
import sys
from urllib.parse import urlsplit


def main():
    missing = [k for k in ["CLOUDFLARE_API_TOKEN", "CLOUDFLARE_ACCOUNT_ID", "WORKER_CONFIG_JSON"] if not os.environ.get(k, "").strip()]
    if missing:
        raise ValueError("Missing GitHub Worker settings: " + ", ".join(missing))
    if not re.fullmatch(r"[a-fA-F0-9]{32}", os.environ["CLOUDFLARE_ACCOUNT_ID"]):
        raise ValueError("Invalid CLOUDFLARE_ACCOUNT_ID")
    config = json.loads(os.environ["WORKER_CONFIG_JSON"])
    if not config.get("name") or config.get("main") != "worker/index.ts" or not config.get("compatibility_date"):
        raise ValueError("Worker name, compatibility_date and main are required")
    if config.get("assets", {}).get("directory") != "./dist":
        raise ValueError("Worker assets.directory must be ./dist")
    origin = urlsplit(config.get("vars", {}).get("API_ORIGIN", ""))
    if origin.scheme != "https" or not origin.hostname or origin.username or origin.password or origin.query or origin.fragment:
        raise ValueError("Worker API_ORIGIN must be HTTPS")
    routes = config.get("routes", [])
    if len(routes) != 1:
        raise ValueError("Worker requires exactly one route")
    route = routes[0]
    pattern = route.get("pattern", "")
    hostname = re.compile(r"[A-Za-z0-9.-]+")
    if route.get("custom_domain") is True:
        if not hostname.fullmatch(pattern):
            raise ValueError("Worker custom domain must be a hostname")
    elif hostname.fullmatch(route.get("zone_name", "")) and pattern.endswith("/*") and hostname.fullmatch(pattern[:-2]):
        pass
    else:
        raise ValueError("Worker route must be one custom domain or one zone route")
    Path(sys.argv[1]).write_text(json.dumps(config))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, TypeError) as error:
        print("Worker deploy preflight failed: " + str(error), file=sys.stderr)
        sys.exit(1)
