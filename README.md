# Knotree Authentication

Central identity provider for the Knotree ecosystem. Production origin: `https://accounts.knotree.com`.

Knotree applications do not keep their own passwords. Sign-in starts on the application and finishes here with OAuth 2.1 authorization code and PKCE. The browser session cookie is host-only on the accounts origin.

## Local development

```bash
cp .env.example .env
docker compose up -d postgres
cd backend && cargo run
cd frontend && npm install && npm run dev
```

- Accounts UI: http://127.0.0.1:5173
- API: http://127.0.0.1:8080
- Health: `GET /health` and `GET /ready`
- Development mail: `GET /api/v1/dev/mailbox?email=person@example.com`

Create the database `knotree_accounts` if you are not using Compose. The API applies migrations on startup. Tests use `knotree_accounts_test` via `TEST_DATABASE_URL`.

An example relying party is in `examples/knotree-study`.

## Layout

- `backend/` Rust API (Axum, SQLx, PostgreSQL)
- `frontend/` React, React Router, TypeScript, Tailwind, Base UI, Cloudflare Worker
- `docs/` architecture, threat model, OAuth, MFA, email, admin, and integration guide

## Checks

```bash
cd backend && cargo fmt --all --check && cargo clippy --all-targets --features test-util -- -D warnings && cargo test --features test-util
cd frontend && npm test && npm run typecheck && npm run build
```
