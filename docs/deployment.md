# Deployment

## API

Build `docker/Dockerfile`. The image is multi-stage, runs as uid 10001, and expects `SIGINT` for graceful shutdown. `deploy/k8s/api.yaml` is a starting Deployment and Service. Supply env from a secret named `knotree-accounts`. Give the container enough memory for Argon2id (64 MiB per hash, plus the process).

Probes: `GET /health` and `GET /ready`.

## Frontend

`frontend` builds a static React Router application. `wrangler.jsonc` runs `worker/index.ts` first. Set the Worker var `API_ORIGIN` to the private API origin. The worker proxies API paths and adds security headers. HTML is `no-store`.

Do not point the session cookie at a shared parent domain.

## Environments

Development, staging, and production use different databases, OAuth client secrets, email credentials, JWT keys, and TOTP encryption keys.
