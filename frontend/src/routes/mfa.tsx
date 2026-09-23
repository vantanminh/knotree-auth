import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router";
import { AuthShell } from "../components/shells";
import { Alert, Button } from "../components/ui";
import { ApiError, api, continueAfterAuth } from "../lib/api";

type Challenge = {
  mfa_token: string;
  methods: string[];
  masked_email: string | null;
  return_to: string | null;
};

function loadChallenge(): Challenge | null {
  const raw = sessionStorage.getItem("knotree.mfa");
  if (!raw) return null;
  try {
    return JSON.parse(raw) as Challenge;
  } catch {
    return null;
  }
}

export function MfaPage({ method }: { method: "totp" | "email" | "recovery" }) {
  const navigate = useNavigate();
  const challenge = loadChallenge();
  const [code, setCode] = useState("");
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  const [cooldown, setCooldown] = useState(0);

  useEffect(() => {
    if (!challenge) navigate("/sign-in", { replace: true });
  }, [challenge, navigate]);

  useEffect(() => {
    if (cooldown <= 0) return;
    const timer = window.setTimeout(() => setCooldown((value) => value - 1), 1000);
    return () => window.clearTimeout(timer);
  }, [cooldown]);

  if (!challenge) return null;

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setPending(true);
    setError("");
    try {
      await api("/api/v1/auth/mfa/verify", {
        method: "POST",
        body: JSON.stringify({ mfa_token: challenge!.mfa_token, method, code }),
      });
      sessionStorage.removeItem("knotree.mfa");
      continueAfterAuth(challenge!.return_to);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "That code is not valid.");
    } finally {
      setPending(false);
    }
  }

  async function resend() {
    setPending(true);
    setError("");
    try {
      await api("/api/v1/auth/mfa/email/send", {
        method: "POST",
        body: JSON.stringify({ mfa_token: challenge!.mfa_token }),
      });
      setCooldown(30);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not send a new code.");
    } finally {
      setPending(false);
    }
  }

  const title = method === "recovery" ? "Use a recovery code" : "Verify it's you";
  const detail =
    method === "totp"
      ? "Enter the 6-digit code from your authenticator app."
      : method === "email"
        ? `We sent a verification code to ${challenge.masked_email ?? "your email"}. It expires in a few minutes. Do not share it.`
        : "Enter one unused recovery code.";

  return (
    <AuthShell>
      <h1 className="text-[22px] font-medium tracking-tight">{title}</h1>
      <p className="mt-1 mb-6 text-sm text-muted">{detail}</p>
      <form className="grid gap-4" onSubmit={submit}>
        {error ? <Alert>{error}</Alert> : null}
        <label className="grid gap-1.5 text-sm">
          {method === "recovery" ? "Recovery code" : "Verification code"}
          <input
            className="h-10 rounded-[6px] border border-line bg-surface px-3 tracking-[0.2em]"
            inputMode={method === "recovery" ? "text" : "numeric"}
            autoComplete="one-time-code"
            autoFocus
            value={code}
            onChange={(event) => setCode(event.target.value)}
            required
          />
        </label>
        <Button type="submit" pending={pending} className="w-full">
          {pending ? "Verifying…" : "Verify"}
        </Button>
      </form>
      <div className="mt-5 grid gap-2 text-sm">
        {method === "email" ? (
          <button type="button" className="text-left underline" disabled={cooldown > 0 || pending} onClick={() => void resend()}>
            {cooldown > 0 ? `Resend in ${cooldown}s` : "Resend code"}
          </button>
        ) : null}
        {challenge.methods.includes("totp") && method !== "totp" ? <Link to="/mfa/totp">Use authenticator instead</Link> : null}
        {challenge.methods.includes("email") && method !== "email" ? <Link to="/mfa/email">Use email code instead</Link> : null}
        {method !== "recovery" ? <Link to="/mfa/recovery">Use a recovery code</Link> : null}
      </div>
    </AuthShell>
  );
}
