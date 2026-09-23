import { createHash, randomBytes } from "node:crypto";
import { createServer } from "node:http";

const accounts = process.env.ACCOUNTS_URL ?? "http://127.0.0.1:5173";
const clientId = "knotree-study";
const redirectUri = "http://127.0.0.1:4174/auth/callback";
const pending = new Map();

function page(body) {
  return `<!doctype html><meta charset="utf-8"><title>Knotree Study</title><body style="font-family:sans-serif;max-width:36rem;margin:3rem auto">${body}</body>`;
}

createServer(async (req, res) => {
  const url = new URL(req.url, "http://127.0.0.1:4174");
  if (url.pathname === "/") {
    const verifier = randomBytes(32).toString("base64url");
    const challenge = createHash("sha256").update(verifier).digest("base64url");
    const state = randomBytes(16).toString("base64url");
    const nonce = randomBytes(16).toString("base64url");
    pending.set(state, { verifier, nonce });
    const authorize = new URL("/oauth/authorize", accounts);
    authorize.searchParams.set("client_id", clientId);
    authorize.searchParams.set("redirect_uri", redirectUri);
    authorize.searchParams.set("response_type", "code");
    authorize.searchParams.set("scope", "openid profile email");
    authorize.searchParams.set("state", state);
    authorize.searchParams.set("nonce", nonce);
    authorize.searchParams.set("code_challenge", challenge);
    authorize.searchParams.set("code_challenge_method", "S256");
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end(page(`<h1>Knotree Study</h1><p><a href="${authorize}">Sign in</a></p>`));
    return;
  }
  if (url.pathname === "/auth/callback") {
    const state = url.searchParams.get("state") ?? "";
    const code = url.searchParams.get("code") ?? "";
    const stored = pending.get(state);
    pending.delete(state);
    if (!stored || !code) {
      res.writeHead(400, { "content-type": "text/html; charset=utf-8" });
      res.end(page("<p>This sign-in attempt is not valid.</p>"));
      return;
    }
    const body = new URLSearchParams({
      grant_type: "authorization_code",
      code,
      redirect_uri: redirectUri,
      client_id: clientId,
      code_verifier: stored.verifier,
    });
    const token = await fetch(new URL("/oauth/token", accounts), {
      method: "POST",
      headers: { "content-type": "application/x-www-form-urlencoded" },
      body,
    });
    const json = await token.json();
    res.writeHead(token.ok ? 200 : 400, { "content-type": "text/html; charset=utf-8" });
    res.end(page(`<h1>Signed in to Knotree Study</h1><pre>${JSON.stringify({ ok: token.ok, token_type: json.token_type, expires_in: json.expires_in, has_id_token: Boolean(json.id_token) }, null, 2)}</pre>`));
    return;
  }
  res.writeHead(404).end();
}).listen(4174, "127.0.0.1");
