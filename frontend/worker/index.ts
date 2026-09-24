interface Env {
  ASSETS: { fetch(request: Request): Promise<Response> };
  API_ORIGIN?: string;
}

const PROXY_PREFIXES = ["/api/", "/oauth/", "/.well-known/", "/health", "/ready"];

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (PROXY_PREFIXES.some((prefix) => url.pathname === prefix.replace(/\/$/, "") || url.pathname.startsWith(prefix))) {
      return proxy(request, url, env);
    }
    const asset = await env.ASSETS.fetch(request);
    return decorate(asset, true);
  },
};

async function proxy(request: Request, url: URL, env: Env): Promise<Response> {
  const origin = env.API_ORIGIN || "http://127.0.0.1:8080";
  const target = new URL(`${url.pathname}${url.search}`, origin);
  const headers = new Headers(request.headers);
  headers.set("x-forwarded-host", url.host);
  headers.set("x-forwarded-proto", url.protocol.replace(":", ""));
  const init: RequestInit = {
    method: request.method,
    headers,
    redirect: "manual",
  };
  if (request.method !== "GET" && request.method !== "HEAD") {
    init.body = request.body;
  }
  const upstream = await fetch(target, init);
  return decorate(upstream, false);
}

function decorate(upstream: Response, html: boolean): Response {
  const headers = new Headers();
  upstream.headers.forEach((value, key) => {
    if (key.toLowerCase() === "set-cookie") return;
    headers.set(key, value);
  });
  const cookies = typeof upstream.headers.getSetCookie === "function" ? upstream.headers.getSetCookie() : [];
  for (const cookie of cookies) headers.append("set-cookie", cookie);
  headers.set("x-content-type-options", "nosniff");
  headers.set("referrer-policy", "no-referrer");
  headers.set("x-frame-options", "DENY");
  headers.set("permissions-policy", "camera=(), microphone=(), geolocation=()");
  if (html) {
    headers.set(
      "content-security-policy",
      "default-src 'self'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'",
    );
    headers.set("cache-control", "no-store");
  }
  return new Response(upstream.body, { status: upstream.status, statusText: upstream.statusText, headers });
}
