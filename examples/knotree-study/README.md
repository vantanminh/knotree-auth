# Knotree Study example

Public OAuth client on `http://127.0.0.1:4174`.

```bash
ACCOUNTS_URL=http://127.0.0.1:5173 node server.mjs
```

Open the page and choose Sign in. The server stores the PKCE verifier, sends the browser to Knotree Accounts, and exchanges the code on `/auth/callback`. It does not keep a password.
