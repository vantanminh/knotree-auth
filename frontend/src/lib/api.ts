export class ApiError extends Error {
  status: number;
  code: string;
  requestId?: string;

  constructor(status: number, code: string, message: string, requestId?: string) {
    super(message);
    this.status = status;
    this.code = code;
    this.requestId = requestId;
  }
}

type ErrorBody = {
  error?: { code?: string; message?: string; request_id?: string };
};

let csrfToken: string | null = null;

export function clearCsrf() {
  csrfToken = null;
}

async function ensureCsrf(): Promise<string> {
  if (csrfToken) return csrfToken;
  const response = await fetch("/api/v1/auth/csrf", { credentials: "include" });
  const body = (await response.json()) as { csrf_token?: string };
  if (!response.ok || !body.csrf_token) {
    throw new ApiError(response.status, "CSRF", "Refresh the page and try again.");
  }
  csrfToken = body.csrf_token;
  return csrfToken;
}

export async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const method = (init.method ?? "GET").toUpperCase();
  const headers = new Headers(init.headers);
  if (init.body && !headers.has("content-type")) {
    headers.set("content-type", "application/json");
  }
  if (method !== "GET" && method !== "HEAD") {
    headers.set("x-csrf-token", await ensureCsrf());
  }
  const response = await fetch(path, { ...init, method, headers, credentials: "include" });
  const text = await response.text();
  const parsed = text ? (JSON.parse(text) as unknown) : null;
  if (!response.ok) {
    const error = (parsed as ErrorBody | null)?.error;
    if (response.status === 403) clearCsrf();
    throw new ApiError(
      response.status,
      error?.code ?? "REQUEST_FAILED",
      error?.message ?? "Something went wrong. Try again.",
      error?.request_id,
    );
  }
  return parsed as T;
}

export function safeReturnTo(value: string | null): string | null {
  if (!value) return null;
  if (!value.startsWith("/") || value.startsWith("//") || value.includes("\\") || value.includes("\n")) {
    return null;
  }
  const path = value.split("?")[0] ?? value;
  if (path.includes("..") || path.includes("@") || path.includes("#")) return null;
  return value;
}

export function continueAfterAuth(returnTo: string | null) {
  const safe = safeReturnTo(returnTo);
  if (safe?.startsWith("/oauth/")) {
    window.location.assign(safe);
    return;
  }
  window.location.assign(safe && safe !== "/sign-in" ? safe : "/account");
}
