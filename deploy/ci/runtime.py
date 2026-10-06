#!/usr/bin/env python3
"""GitHub-owned runtime configuration. Never log configuration or secret values."""
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit


class Invalid(Exception):
    pass


UUID_PATTERN = re.compile(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-8][0-9a-fA-F]{3}-[89aAbB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}$")
POSTGRES_IDENTIFIER = re.compile(r"^[A-Za-z_][A-Za-z0-9_]{0,62}$")


def unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise Invalid("Duplicate configuration key: " + key)
        result[key] = value
    return result


def decode(value, name):
    try:
        result = json.loads(value, object_pairs_hook=unique)
    except (ValueError, TypeError):
        raise Invalid(name + " must contain valid JSON") from None
    if not isinstance(result, dict):
        raise Invalid(name + " must be a JSON object")
    return result


def validate(contract, config, secrets):
    for name, values, required, optional in [
        ("K8S_CONFIG_JSON", config, contract["config"], contract.get("optional_config", [])),
        ("K8S_SECRETS_JSON", secrets, contract["secrets"], contract.get("optional_secrets", [])),
    ]:
        missing = sorted(set(required) - values.keys())
        if missing:
            raise Invalid(name + " missing: " + ", ".join(missing))
        unknown = sorted(values.keys() - set(required) - set(optional))
        if unknown:
            raise Invalid(name + " unknown keys: " + ", ".join(unknown))
        for key, value in values.items():
            if not isinstance(value, str) or (not value.strip() and key not in contract.get("allow_empty", [])):
                raise Invalid(name + " empty/invalid: " + key)
            if name == "K8S_CONFIG_JSON" and any(ord(c) < 32 for c in value):
                raise Invalid("Configuration contains control characters: " + key)
    for key, length in contract.get("min_length", {}).items():
        if len(secrets.get(key, "")) < length:
            raise Invalid("Secret too short: " + key)
    if config.keys() & secrets.keys():
        raise Invalid("Config and secret keys overlap")
    for key, expected in contract.get("fixed", {}).items():
        if config.get(key) != expected:
            raise Invalid("Invalid production setting: " + key)
    for key in contract.get("https", []):
        url = urlsplit(config[key])
        if url.scheme != "https" or not url.hostname or url.username or url.password or url.query or url.fragment:
            raise Invalid("Expected HTTPS URL: " + key)
    if secrets.get("POSTGRES_PASSWORD"):
        url = urlsplit(secrets["DATABASE_URL"])
        if url.scheme not in ("postgres", "postgresql") or unquote(url.password or "") != secrets["POSTGRES_PASSWORD"]:
            raise Invalid("DATABASE_URL must use POSTGRES_PASSWORD")
    identity = contract.get("database_identity")
    if identity:
        user = config[identity["user"]]
        database = config[identity["database"]]
        if not POSTGRES_IDENTIFIER.fullmatch(user) or not POSTGRES_IDENTIFIER.fullmatch(database):
            raise Invalid("PostgreSQL user/database must be simple identifiers")
        url = urlsplit(secrets["DATABASE_URL"])
        if (url.scheme not in ("postgres", "postgresql")
                or unquote(url.username or "") != user
                or unquote(url.path.removeprefix("/")) != database):
            raise Invalid("DATABASE_URL must use POSTGRES_USER and POSTGRES_DB")
    for key in contract.get("uuid_config", []):
        if key in config and not UUID_PATTERN.fullmatch(config[key]):
            raise Invalid("Invalid UUID configuration: " + key)
    for group in contract.get("secret_groups", []):
        if any(secrets.get(k) for k in group) and not all(secrets.get(k) for k in group):
            raise Invalid("Incomplete secret group: " + ", ".join(group))
    for group in contract.get("cross_groups", []):
        combined = {**config, **secrets}
        if any(combined.get(k) for k in group) and not all(combined.get(k) for k in group):
            raise Invalid("Incomplete configuration/secret group: " + ", ".join(group))
    if config.get("STORAGE_BACKEND") == "r2" and not all(secrets.get(k) for k in ["R2_ENDPOINT", "R2_BUCKET", "R2_ACCESS_KEY_ID", "R2_SECRET_ACCESS_KEY"]):
        raise Invalid("R2 storage requires all four R2 secrets")
    if config.get("PULL_MODE") == "edge" and not all(secrets.get(k) for k in ["EDGE_DOWNLOAD_URL", "EDGE_DOWNLOAD_SECRET"]):
        raise Invalid("Redirect pull mode requires both edge download secrets")
    if "DATABASE_CREDENTIALS_ENCRYPTION_KEY" in secrets:
        try:
            key = secrets["DATABASE_CREDENTIALS_ENCRYPTION_KEY"]
            raw = base64.b64decode(key + "=" * (-len(key) % 4), altchars=b"-_", validate=True)
        except ValueError:
            raise Invalid("Invalid DATABASE_CREDENTIALS_ENCRYPTION_KEY") from None
        if len(raw) != 32:
            raise Invalid("DATABASE_CREDENTIALS_ENCRYPTION_KEY must encode 32 bytes")
    errors = value_errors(config, secrets)
    if errors:
        raise Invalid("\n".join(errors))
    return config, secrets


def value_errors(config, secrets):
    """Check every supplied value. Returns every problem, not only the first."""
    errors = []
    combined = {**config, **secrets}
    account = secrets.get("CLOUDFLARE_ACCOUNT_ID", "")
    if account and (len(account) != 32 or any(c not in "0123456789abcdefABCDEF" for c in account)):
        errors.append("CLOUDFLARE_ACCOUNT_ID must be a 32-character hexadecimal ID")
    memory = config.get("ARGON2_MEMORY_KIB", "")
    if memory:
        try:
            if int(memory) < 19456:
                errors.append("ARGON2_MEMORY_KIB must be at least 19456 in production")
        except ValueError:
            errors.append("ARGON2_MEMORY_KIB must be an integer")
    for name in ("ARGON2_ITERATIONS", "ARGON2_PARALLELISM"):
        value = config.get(name, "")
        if value:
            try:
                if int(value) <= 0:
                    raise ValueError
            except ValueError:
                errors.append(name + " must be a positive integer")
    bind = config.get("BIND_ADDR", "")
    if bind and not re.fullmatch(r".+:\d+$", bind):
        errors.append("BIND_ADDR must be host:port")
    if "TOTP_ENCRYPTION_KEYS" in secrets:
        errors.extend(totp_errors(secrets["TOTP_ENCRYPTION_KEYS"], config.get("TOTP_ENCRYPTION_KEY_VERSION", "1")))
    if "JWT_PRIVATE_KEY_PEM" in secrets:
        errors.extend(pem_errors("JWT_PRIVATE_KEY_PEM", secrets["JWT_PRIVATE_KEY_PEM"], "PRIVATE KEY"))
    previous = secrets.get("JWT_PREVIOUS_PUBLIC_KEY_PEM", "")
    if previous:
        errors.extend(pem_errors("JWT_PREVIOUS_PUBLIC_KEY_PEM", previous, "PUBLIC KEY"))
    for name in ("POSTGRES_CA_PEM", "POSTGRES_TLS_CERT_PEM", "POSTGRES_TLS_KEY_PEM"):
        if secrets.get(name):
            errors.extend(pem_errors(name, secrets[name], None))
    email = config.get("EMAIL_FROM", "")
    if email and ("@" not in email or any(c.isspace() for c in email)):
        errors.append("EMAIL_FROM must be an email address")
    origins = config.get("CORS_ORIGINS", "")
    if origins:
        for origin in [part.strip() for part in origins.split(",") if part.strip()]:
            url = urlsplit(origin)
            if url.scheme not in ("http", "https") or not url.hostname:
                errors.append("CORS_ORIGINS contains an invalid URL: " + origin)
    for name in (
        "SESSION_TTL_HOURS", "SESSION_IDLE_HOURS", "ADMIN_SESSION_HOURS", "ADMIN_IDLE_MINUTES",
        "STEP_UP_MINUTES", "ACCESS_TOKEN_SECONDS", "REFRESH_TOKEN_DAYS", "AUTH_CODE_SECONDS",
        "EMAIL_OTP_SECONDS", "VERIFICATION_HOURS", "RESET_MINUTES",
    ):
        value = combined.get(name, "")
        if value:
            try:
                if int(value) <= 0:
                    raise ValueError
            except ValueError:
                errors.append(name + " must be a positive integer")
    return errors


def totp_errors(spec, active):
    try:
        version = int(active)
    except ValueError:
        return ["TOTP_ENCRYPTION_KEY_VERSION must be an integer"]
    seen = set()
    errors = []
    for part in [item.strip() for item in spec.split(",") if item.strip()]:
        if ":" not in part:
            errors.append("TOTP_ENCRYPTION_KEYS must be version:base64")
            continue
        raw_version, material = part.split(":", 1)
        try:
            key_version = int(raw_version)
        except ValueError:
            errors.append("TOTP_ENCRYPTION_KEYS has an invalid version")
            continue
        try:
            decoded = decode_b64(material)
        except ValueError:
            errors.append("TOTP_ENCRYPTION_KEYS is not base64")
            continue
        if len(decoded) != 32:
            errors.append("TOTP_ENCRYPTION_KEYS must encode 32 bytes")
            continue
        seen.add(key_version)
    if not errors and version not in seen:
        errors.append("TOTP_ENCRYPTION_KEYS is missing the active version")
    return errors


def decode_b64(material):
    cleaned = material.strip().strip('"').strip("'").replace(" ", "+")
    cleaned = "".join(cleaned.split())
    padded = cleaned + "=" * (-len(cleaned) % 4)
    return base64.b64decode(padded, altchars=b"-_", validate=True)


def pem_errors(name, value, label):
    if "\x00" in value:
        return [name + " contains a NUL byte"]
    text = value.strip().strip('"').strip("'")
    if "BEGIN" not in text:
        try:
            text = decode_b64(text).decode()
        except (ValueError, UnicodeError):
            return [name + " is not a PEM " + (label or "block")]
    pem = text.replace("\\n", "\n").replace("\r", "").strip()
    begin = pem.find("-----BEGIN ")
    if begin != -1:
        after = begin + len("-----BEGIN ")
        label_end = pem.find("-----", after)
        if label_end != -1:
            header_end = label_end + 5
            end = pem.find("-----END ", header_end)
            if end == -1:
                pem = pem[:header_end] + pem[header_end:].replace(" ", "+")
            else:
                pem = pem[:header_end] + pem[header_end:end].replace(" ", "+") + pem[end:]
    if label:
        begin = "-----BEGIN " + label + "-----"
        end = "-----END " + label + "-----"
        if not pem.startswith(begin) or not pem.endswith(end):
            return [name + " is not a PEM " + label]
    elif "-----BEGIN " not in pem or "-----END " not in pem:
        return [name + " is not a PEM"]
    body = "".join(line for line in pem.splitlines() if not line.startswith("-----"))
    try:
        base64.b64decode(body, validate=True)
    except ValueError:
        return [name + " PEM body is not base64"]
    return []


def resource(kind, name, namespace, values, secret_type="Opaque"):
    obj = {"apiVersion": "v1", "kind": kind, "metadata": {"name": name, "namespace": namespace}}
    if kind == "Secret":
        obj.update(type=secret_type, data={k: base64.b64encode(v.encode()).decode() for k, v in values.items()})
    else:
        obj["data"] = values
    return obj


def manifests(contract, config, secrets):
    ns = contract["namespace"]
    runtime = {k: v for k, v in secrets.items() if k not in contract.get("auxiliary_secrets", [])}
    objects = [resource("ConfigMap", contract["configmap"], ns, config), resource("Secret", contract["runtime_secret"], ns, runtime)]
    docker = json.dumps({"auths": {"ghcr.io": {"auth": base64.b64encode((secrets["GHCR_USERNAME"] + ":" + secrets["GHCR_TOKEN"]).encode()).decode()}}})
    objects.append(resource("Secret", contract["pull_secret"], ns, {".dockerconfigjson": docker}, "kubernetes.io/dockerconfigjson"))
    for item in contract.get("extra_secrets", []):
        objects.append(resource("Secret", item["name"], ns, {dest: secrets[source] for dest, source in item["keys"].items()}))
    return objects


def kube(args, data=None, absent=False):
    command = ["kubectl"] if os.environ.get("KUBECONFIG") else ["k3s", "kubectl"]
    result = subprocess.run([*command, *args], input=data, text=True, capture_output=True)
    if result.returncode:
        # kubectl errors may embed submitted secret data. Never forward stderr.
        raise Invalid("Kubernetes operation failed (details withheld to protect secrets)")
    if absent and not result.stdout.strip():
        return None
    return result.stdout


def apply(contract, config, secrets):
    objects = manifests(contract, config, secrets)
    ns = contract["namespace"]
    preserve_env = contract.get("preserve_statefulset_env")
    if preserve_env:
        namespace = kube(["get", "namespace", ns, "--ignore-not-found", "-o", "name"], absent=True)
        if namespace:
            current = kube(["get", "statefulset", preserve_env["name"], "-n", ns,
                            "--ignore-not-found", "-o", "json"], absent=True)
            if current:
                statefulset = json.loads(current)
                container = next((item for item in statefulset.get("spec", {}).get("template", {}).get("spec", {}).get("containers", [])
                                  if item.get("name") == preserve_env["container"]), None)
                actual = {item.get("name"): item.get("value") for item in (container or {}).get("env", [])}
                for env_name, config_key in preserve_env["values"].items():
                    if actual.get(env_name) != config[config_key]:
                        raise Invalid("Refusing implicit PostgreSQL identity change: " + env_name)
            else:
                if preserve_env.get("required"):
                    raise Invalid("Existing PostgreSQL StatefulSet is required before runtime update")
                pvc_json = kube(["get", "pvc", "-n", ns, "-o", "json"])
                if json.loads(pvc_json).get("items"):
                    raise Invalid("PostgreSQL StatefulSet is missing while PVC data remains")
    # Validate all immutable credentials BEFORE any mutation. Changing a Secret
    # does not rotate a live PostgreSQL password or re-encrypt stored data.
    for name, keys in contract["preserve"].items():
        current = kube(["get", "secret", name, "-n", ns, "--ignore-not-found", "-o", "json"], absent=True)
        if current:
            existing = json.loads(current).get("data", {})
            desired = next(o["data"] for o in objects if o["kind"] == "Secret" and o["metadata"]["name"] == name)
            for key in keys:
                if key in existing and existing[key] != desired[key]:
                    raise Invalid("Refusing implicit credential rotation: " + name + "/" + key)
    namespace = {"apiVersion": "v1", "kind": "Namespace", "metadata": {"name": ns}}
    kube(["apply", "-f", "-"], json.dumps(namespace))
    # No last-applied annotation containing secret material. Force ownership
    # only for these explicitly CI-owned config/secret objects.
    for obj in objects:
        kube(["apply", "--server-side", "--force-conflicts", "--field-manager=github-runtime", "-f", "-"], json.dumps(obj))
        # Replace only data, preserving metadata/resource identity. This removes
        # stale keys owned by earlier manual/Helm managers without deleting Secrets.
        kube(["patch", obj["kind"].lower(), obj["metadata"]["name"], "-n", ns,
              "--type=json", "--patch-file=/dev/stdin"],
             json.dumps([{"op": "replace", "path": "/data", "value": obj["data"]}]))


def main():
    contract = json.loads(Path(__file__).with_name("contract.json").read_text())
    command = sys.argv[1]
    if command == "prepare":
        missing = [k for k in ["KUBE_CONFIG", "K8S_CONFIG_JSON", "K8S_SECRETS_JSON", *contract.get("external_secrets", [])] if not os.environ.get(k, "").strip()]
        if missing:
            raise Invalid("Missing GitHub Actions Secrets/Variables: " + ", ".join(missing))
        kubeconfig = os.environ["KUBE_CONFIG"]
        if "https://15.235.210.66:6443" not in kubeconfig or not any(marker in kubeconfig for marker in ("token:", "client-certificate-data:")):
            raise Invalid("KUBE_CONFIG must target the production k3s API")
        supplied = decode(os.environ["K8S_SECRETS_JSON"], "K8S_SECRETS_JSON")
        for key in contract.get("external_secrets", []):
            if key in supplied:
                raise Invalid("Use the dedicated GitHub Actions Secret for " + key)
            supplied[key] = os.environ[key]
        config, secrets = validate(contract, decode(os.environ["K8S_CONFIG_JSON"], "K8S_CONFIG_JSON"), supplied)
        target = Path(sys.argv[2])
        target.mkdir(parents=True, exist_ok=True, mode=0o700)
        path = target / "runtime.json"
        path.write_text(json.dumps({"config": config, "secrets": secrets}))
        path.chmod(0o600)
    elif command == "render-postgres":
        packet = decode(Path(sys.argv[2]).read_text(), "runtime payload")
        config, secrets = validate(contract, packet["config"], packet["secrets"])
        del secrets
        source = Path(sys.argv[3]).read_text(encoding="utf-8")
        replacements = {
            "__KNOTREE_POSTGRES_USER__": config[contract["database_identity"]["user"]],
            "__KNOTREE_POSTGRES_DB__": config[contract["database_identity"]["database"]],
        }
        for marker, value in replacements.items():
            if marker not in source:
                raise Invalid("PostgreSQL manifest is missing a runtime marker: " + marker)
            source = source.replace(marker, value)
        Path(sys.argv[4]).write_text(source, encoding="utf-8")
    else:
        packet = decode(Path(sys.argv[2]).read_text(), "runtime payload")
        config, secrets = validate(contract, packet["config"], packet["secrets"])
        if command == "apply":
            apply(contract, config, secrets)
        elif command == "checksum":
            print(hashlib.sha256(json.dumps(packet, sort_keys=True).encode()).hexdigest())
        elif command == "values":
            env = {dest: config[src] for src, dest in contract["helm_env"].items()}
            values = {"env": env, "runtime": {"enabled": True, "configMap": contract["configmap"]}, "podAnnotations": {"github-runtime/checksum": hashlib.sha256(json.dumps(packet, sort_keys=True).encode()).hexdigest()}, "controlPlaneDatabase": {"user": config["POSTGRES_USER"], "database": config["POSTGRES_DB"]}, "ingress": {"host": urlsplit(config["PUBLIC_UI_URL"]).hostname, "apiHost": urlsplit(config["PUBLIC_API_URL"]).hostname}, "web": {"ingress": {"host": urlsplit(config["PUBLIC_UI_URL"]).hostname}}}
            Path(sys.argv[3]).write_text(json.dumps(values))
        else:
            raise Invalid("Unknown runtime command")


if __name__ == "__main__":
    try:
        main()
    except (Invalid, KeyError, OSError) as error:
        print("Deploy preflight failed: " + str(error), file=sys.stderr)
        sys.exit(1)
