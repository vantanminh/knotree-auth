# GitHub-owned production configuration

Production deploy reads only GitHub Actions Secrets/Variables. No VPS `.env`, protected credential input file, copied pull secret, or server-generated key/password is used. Kubernetes Secrets are runtime outputs delivered by CI, not configuration inputs.

Set these under **Settings → Secrets and variables → Actions** (or organization values restricted to this repository):

| GitHub location | Name | Value |
| --- | --- | --- |
| Variable | `K8S_CONFIG_JSON` | Complete JSON from `deploy/ci/config.example.json`; all values are strings. Review domains and feature flags. |
| Secret | `K8S_SECRETS_JSON` | Complete JSON from `deploy/ci/secrets.example.json`, with every required empty placeholder filled. |
| Secret | `KUBE_CONFIG` | Production kubeconfig whose API server is `https://15.235.210.66:6443`. Same secret used by the other app workflows. |

`GHCR_USERNAME` and `GHCR_TOKEN` inside the secret JSON must be persistent credentials with package read access; the short-lived Actions `GITHUB_TOKEN` is used to publish images only. They are not copied from another server namespace.

`deploy/ci/contract.json` is the authoritative required/optional key list. Missing JSON, missing keys, blank required values, duplicate JSON keys, unknown keys, unsafe production settings and incomplete optional feature secret groups fail the **deploy-preflight** job before image publication or any Kubernetes write. PR checks use synthetic fixtures and never require production secrets. The production workflow runs automatically on the default branch; Accounts no longer silently skips deploy when `K3S_DEPLOY_ENABLED` is unset.

GitHub CI applies only the target namespace's ConfigMap/Secrets with kubectl and the `KUBE_CONFIG` secret, then removes the temporary kubeconfig on exit. Values are never passed as shell command arguments, sourced, printed, or uploaded as artifacts. Runtime checksums trigger only the affected application rollout when configuration changes. The target workload's existing PVC/data is preserved.

For an existing installation, seed GitHub with the exact current database password/URL and encryption keys. The deploy compares protected live credentials before any Kubernetes write and fails if they differ; updating a Secret is not a PostgreSQL password rotation or data re-encryption. Use a separate reviewed rotation flow for these changes. The deploy never silently reuses server values when a GitHub value is missing.

Local verification: `python3 -m unittest discover -s deploy/ci -p 'test_*.py'`. Real image builds and backend tests remain GitHub CI tasks. Local code/contract checks alone do not prove deployment.

## Accounts inputs

Also set GitHub Secret `CLOUDFLARE_API_TOKEN`, Variables `CLOUDFLARE_ACCOUNT_ID` and `WORKER_CONFIG_JSON` (copy `worker.example.json`). The API workflow validates Worker settings and automatically calls the Worker deploy workflow after successful API deployment. The independent Worker dispatch also fails early on missing settings. Worker and API public URLs must match the supplied configuration.

The secret JSON supplies Email Sending credentials, persistent JWT/TOTP keys, Postgres password and TLS CA/server certificate/private key, metrics token, and R2 backup credentials. Store PEM strings as JSON strings with escaped newlines; keep values in GitHub Secrets. Create these keys/certificates once in a trusted environment and put them in GitHub, not in a file on the VPS. The deploy creates both the TLS/runtime secrets and the R2 backup secret directly from CI. JWT/TOTP and PostgreSQL TLS material are protected against implicit replacement.

Optional `SUPER_ADMIN_USER_ID`/`SUPER_ADMIN_EMAIL` are GitHub config JSON keys. The old server bootstrap scripts now fail with directions to GitHub configuration. First install still requires k3s Secret encryption to be enabled and its re-encryption completed; CI fails if that cluster prerequisite is absent and does not restart or reconfigure k3s automatically. The initial R2 backup must succeed before Kong is changed. `PUBLIC_API_ORIGIN` uses the currently supported Accounts origin route `https://accounts-api.knotree.com`; DNS for this origin must exist. Cloudflare handles the frontend custom domain from the CI Worker configuration.
