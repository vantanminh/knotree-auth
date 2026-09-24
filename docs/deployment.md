# Production deployment

Knotree Accounts runs as a Cloudflare Worker frontend and a private Rust API in
the `knotree-accounts` Kubernetes namespace. The Worker calls
`https://accounts-api.knotree.com`; Cloudflare proxies that API hostname to the
existing edge IP, nginx-edge forwards it to Kong, and Kong routes it to the
ClusterIP Service. PostgreSQL is internal to the namespace and uses TLS.

The cluster is a single-node k3s server. PostgreSQL uses an 8 GiB `local-path`
volume, so node loss also takes the live database offline. Daily offsite dumps
reduce data loss risk; they do not make the database highly available.

## One-time host setup

Run these commands on the k3s host as root. The cluster must have k3s Secret
encryption enabled; `bootstrap-secrets.sh` checks that encryption is enabled
and fully re-encrypted before it creates application secrets. Never put secret
values in Git, a shell command argument, or chat.

Create protected input files with an editor that does not put their contents in
shell history:

```sh
install -d -m 700 /root/knotree-accounts-input
umask 077
vi /root/knotree-accounts-input/cloudflare-email-api-token
vi /root/knotree-accounts-input/r2.env
chmod 600 /root/knotree-accounts-input/cloudflare-email-api-token /root/knotree-accounts-input/r2.env
```

Create a Cloudflare API token with **Email Sending: Edit** access, scoped to
the account that owns the sender domain. The email token file contains only
that token. Set `CLOUDFLARE_ACCOUNT_ID` to the account's 32-character ID.
`r2.env` contains these four lines, with values from a dedicated Cloudflare
R2 token and bucket:

```text
R2_ACCESS_KEY_ID=...
R2_SECRET_ACCESS_KEY=...
R2_ENDPOINT_URL=https://<account-id>.r2.cloudflarestorage.com
R2_BUCKET=knotree-accounts
```

Create a dedicated private R2 bucket and token with object read/write access to
that bucket only. Configure a lifecycle rule to delete objects under
`knotree-accounts/postgres/` after 35 days. The bucket should not be shared with
application uploads.

Bootstrap the namespace secrets and GHCR pull credentials:

```sh
export CLOUDFLARE_ACCOUNT_ID='<your-32-character-account-id>'
export CLOUDFLARE_EMAIL_API_TOKEN_FILE=/root/knotree-accounts-input/cloudflare-email-api-token
export BACKUP_ENV_FILE=/root/knotree-accounts-input/r2.env
./deploy/k8s/bootstrap-secrets.sh
./deploy/k8s/bootstrap-registry-secret.sh
./deploy/k8s/bootstrap-backup.sh
```

The runtime bootstrap generates the database password, PostgreSQL CA and TLS
certificate, JWT signing key, TOTP encryption key, and metrics token. It stores
the recovery material in `/root/knotree-accounts-private` with mode 0600; keep
an encrypted offline copy of that directory. The scripts refuse to overwrite
existing secrets or keys. Secret encryption at rest is enabled on the current
k3s cluster.

## Publish and deploy the API

Pushing to `main` runs the existing backend, frontend, and E2E CI checks. Only
after all three pass does GitHub Actions build and publish the API image to
GHCR with a commit tag, immutable digest, SBOM, and provenance. Copy the digest
from the workflow summary and deploy by digest:

```sh
./deploy/k8s/deploy.sh ghcr.io/vantanminh/knotree-auth@sha256:<64-hex-digest>
```

The deployment script requires the runtime, PostgreSQL TLS, GHCR pull, and R2
backup secrets. It applies the network policies and database, waits for the
database and two API replicas to become ready, runs and verifies an initial
offsite backup, enables the daily backup job, then updates Kong. The backup
runs daily at 02:00 UTC, stores a SHA-256 file
beside each PostgreSQL dump, and does not expose the database outside the
cluster.

To roll back the API, redeploy the previously recorded image digest. Check
database migration compatibility before rolling back an image after a schema
migration. Do not delete the PostgreSQL PVC during recovery.

## DNS and Worker

Create a proxied Cloudflare A record for `accounts-api.knotree.com` pointing to
`15.235.210.66`. Deploying the Worker custom domain creates its DNS record and
TLS certificate through Cloudflare. The Worker config sets `API_ORIGIN` to
the API hostname.

The repository has a manual `worker-deploy` GitHub Actions workflow. Configure
the repository secret `CLOUDFLARE_API_TOKEN` and variable
`CLOUDFLARE_ACCOUNT_ID`, then run that workflow on `main`. The API token needs
Workers Scripts edit access and DNS edit access for the Knotree zone. The
Worker workflow builds the frontend on GitHub and deploys it; no frontend
artifact is built on the k3s node.

Before using production email, onboard `knotree.com` under Cloudflare
**Compute > Email Service > Email Sending**. Review the SPF, DKIM, DMARC, and
`cf-bounce` MX records Cloudflare proposes, then confirm the domain is enabled
for sending. The sender is `Knotree Accounts <accounts@knotree.com>`. The Worker is deployed only
after the API route and API DNS record are ready. Once the Worker deployment
completes, run
`./deploy/k8s/verify-public.sh` from an environment with network access to
confirm that both hostnames return healthy API responses over HTTPS.

## Initial administrator

Create the first account through `https://accounts.knotree.com`, verify its
email, and obtain that account's UUID from the operator's trusted database
query or authenticated account record. Then run:

```sh
./deploy/k8s/bootstrap-admin.sh <verified-user-uuid>
```

The script confirms that the UUID belongs to a user with a verified email,
creates a separate admin ConfigMap, and restarts the API. Production ignores
`SUPER_ADMIN_EMAIL`; until this step, the admin API remains closed.

## Backup recovery

The backup job validates each dump with `pg_restore --list`, uploads the dump
and checksum to R2, and leaves retention to the bucket lifecycle policy.
Periodically download a dump to an isolated restore environment, run
`sha256sum -c`, restore it into a fresh PostgreSQL 16 database with
`pg_restore --exit-on-error`, then start the matching API image against that
database and verify readiness and account access. Never run a restore against
the production database as a test.

Keep the JWT and TOTP keys from `/root/knotree-accounts-private` with the
database backups. Losing the TOTP key prevents decryption of enrolled
authenticator secrets; losing the JWT signing key invalidates signing-key
continuity. The PostgreSQL CA private key is required to renew its server
certificate.
