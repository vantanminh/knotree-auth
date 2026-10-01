import { useEffect, useState } from "react";
import { Link, Navigate, useNavigate, useSearchParams } from "react-router";
import { AuthShell } from "../components/shells";
import { Alert, Button, TextField } from "../components/ui";
import { ApiError, api, continueAfterAuth, destinationAfterAuth, safeReturnTo } from "../lib/api";
import type { LoginResponse } from "../lib/types";

function useReturnTo() {
  const [params] = useSearchParams();
  return safeReturnTo(params.get("return_to"));
}

function CheckingSession() {
  return (
    <AuthShell>
      <p className="text-sm text-muted">Checking your session…</p>
    </AuthShell>
  );
}

function useResumeSession(returnTo: string | null) {
  const navigate = useNavigate();
  const [status, setStatus] = useState<"checking" | "anonymous">("checking");

  useEffect(() => {
    let cancelled = false;
    void api("/api/v1/me")
      .then(() => {
        if (cancelled) return;
        const destination = destinationAfterAuth(returnTo);
        if (destination.startsWith("/oauth/")) {
          window.location.assign(destination);
          return;
        }
        navigate(destination, { replace: true });
      })
      .catch(() => {
        if (!cancelled) setStatus("anonymous");
      });
    return () => {
      cancelled = true;
    };
  }, [navigate, returnTo]);

  return status;
}

export function HomePage() {
  const status = useResumeSession(null);
  if (status === "checking") return <CheckingSession />;
  return <Navigate to="/sign-in" replace />;
}

export function SignInPage() {
  const returnTo = useReturnTo();
  const session = useResumeSession(returnTo);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  const [clientName, setClientName] = useState<string | null>(null);

  useEffect(() => {
    if (!returnTo?.startsWith("/oauth/authorize")) return;
    void api<{ client_name: string | null }>(`/api/v1/oauth/context?return_to=${encodeURIComponent(returnTo)}`)
      .then((body) => setClientName(body.client_name))
      .catch(() => setClientName(null));
  }, [returnTo]);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setPending(true);
    setError("");
    try {
      const result = await api<LoginResponse>("/api/v1/auth/login", {
        method: "POST",
        body: JSON.stringify({ email, password }),
      });
      if (result.status === "mfa_required") {
        sessionStorage.setItem(
          "knotree.mfa",
          JSON.stringify({ ...result, return_to: returnTo }),
        );
        const method = result.methods.includes("totp") ? "totp" : result.methods[0];
        window.location.assign(`/mfa/${method === "email" ? "email" : method === "totp" ? "totp" : "recovery"}`);
        return;
      }
      continueAfterAuth(returnTo);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not sign in.");
    } finally {
      setPending(false);
    }
  }

  const social = (provider: string) =>
    `/api/v1/auth/social/${provider}/start?return_to=${encodeURIComponent(returnTo ?? "/account")}`;

  if (session === "checking") return <CheckingSession />;

  return (
    <AuthShell>
      <h1 className="text-[22px] font-medium tracking-tight">Sign in</h1>
      <p className="mt-1 mb-6 text-sm text-muted">
        {clientName ? `Sign in to continue to ${clientName}` : "Use your Knotree account."}
      </p>
      <form className="grid gap-4" onSubmit={submit}>
        {error ? <Alert>{error}</Alert> : null}
        <TextField label="Email" name="email" type="email" autoComplete="username" required value={email} onChange={setEmail} />
        <TextField
          label="Password"
          name="password"
          type="password"
          autoComplete="current-password"
          required
          value={password}
          onChange={setPassword}
        />
        <Button type="submit" pending={pending} className="w-full">
          {pending ? "Signing in…" : "Continue"}
        </Button>
      </form>
      <p className="mt-3 text-sm">
        <Link className="text-ink underline decoration-line underline-offset-2" to="/forgot-password">
          Forgot password?
        </Link>
      </p>
      <div className="my-5 flex items-center gap-3 text-xs text-muted">
        <span className="h-px flex-1 bg-line" />
        or
        <span className="h-px flex-1 bg-line" />
      </div>
      <div className="grid gap-2">
        <a className="inline-flex h-10 items-center justify-center rounded-[6px] border border-line bg-surface text-sm" href={social("google")}>
          Continue with Google
        </a>
        <a className="inline-flex h-10 items-center justify-center rounded-[6px] border border-line bg-surface text-sm" href={social("github")}>
          Continue with GitHub
        </a>
      </div>
      <p className="mt-6 text-sm text-muted">
        New to Knotree?{" "}
        <Link className="text-ink underline decoration-line underline-offset-2" to="/sign-up">
          Create account
        </Link>
      </p>
    </AuthShell>
  );
}

export function SignUpPage() {
  const session = useResumeSession(null);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  const navigate = useNavigate();

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (password !== confirm) {
      setError("Passwords do not match.");
      return;
    }
    setPending(true);
    setError("");
    try {
      await api("/api/v1/auth/register", {
        method: "POST",
        body: JSON.stringify({ email, password, password_confirm: confirm }),
      });
      sessionStorage.setItem("knotree.pending-email", email);
      navigate("/verify-email");
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not create the account.");
    } finally {
      setPending(false);
    }
  }

  if (session === "checking") return <CheckingSession />;

  return (
    <AuthShell>
      <h1 className="text-[22px] font-medium tracking-tight">Create account</h1>
      <p className="mt-1 mb-6 text-sm text-muted">One Knotree account works across Knotree services.</p>
      <form className="grid gap-4" onSubmit={submit}>
        {error ? <Alert>{error}</Alert> : null}
        <TextField label="Email" name="email" type="email" autoComplete="email" required value={email} onChange={setEmail} />
        <TextField
          label="Password"
          name="password"
          type="password"
          autoComplete="new-password"
          required
          value={password}
          onChange={setPassword}
          hint="At least 10 characters."
        />
        <TextField
          label="Confirm password"
          name="confirm"
          type="password"
          autoComplete="new-password"
          required
          value={confirm}
          onChange={setConfirm}
        />
        <Button type="submit" pending={pending} className="w-full">
          {pending ? "Creating account…" : "Create account"}
        </Button>
      </form>
      <p className="mt-6 text-sm text-muted">
        Already have an account?{" "}
        <Link className="text-ink underline decoration-line underline-offset-2" to="/sign-in">
          Sign in
        </Link>
      </p>
    </AuthShell>
  );
}

export function VerifyEmailPage() {
  const [params] = useSearchParams();
  const token = params.get("token") ?? "";
  const [message, setMessage] = useState(token ? "Verifying your email…" : "Check your inbox for a verification link.");
  const [error, setError] = useState("");
  const [email, setEmail] = useState(sessionStorage.getItem("knotree.pending-email") ?? "");
  const [pending, setPending] = useState(false);
  const [cooldown, setCooldown] = useState(0);

  useEffect(() => {
    if (!token) return;
    void api("/api/v1/auth/email/verify", { method: "POST", body: JSON.stringify({ token }) })
      .then(() => setMessage("Email verified. You can sign in."))
      .catch((err: unknown) => setError(err instanceof ApiError ? err.message : "This link is not valid."));
  }, [token]);

  useEffect(() => {
    if (cooldown <= 0) return;
    const timer = window.setTimeout(() => setCooldown((value) => value - 1), 1000);
    return () => window.clearTimeout(timer);
  }, [cooldown]);

  async function resend(event: React.FormEvent) {
    event.preventDefault();
    setPending(true);
    setError("");
    try {
      await api("/api/v1/auth/email/resend", { method: "POST", body: JSON.stringify({ email }) });
      setMessage("If an unverified account exists for this email, we sent a new link.");
      setCooldown(30);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not resend the email.");
    } finally {
      setPending(false);
    }
  }

  return (
    <AuthShell>
      <h1 className="text-[22px] font-medium tracking-tight">Verify your email</h1>
      <p className="mt-2 mb-6 text-sm text-muted">{message}</p>
      {error ? <Alert>{error}</Alert> : null}
      {token ? (
        <Link className="mt-4 inline-block text-sm underline" to="/sign-in">
          Continue to sign in
        </Link>
      ) : (
        <form className="mt-4 grid gap-4" onSubmit={resend}>
          <TextField label="Email" name="email" type="email" required value={email} onChange={setEmail} />
          <Button type="submit" pending={pending || cooldown > 0} className="w-full">
            {cooldown > 0 ? `Resend in ${cooldown}s` : pending ? "Sending…" : "Resend verification email"}
          </Button>
        </form>
      )}
    </AuthShell>
  );
}

export function ForgotPasswordPage() {
  const [email, setEmail] = useState("");
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setPending(true);
    setError("");
    try {
      const body = await api<{ message: string }>("/api/v1/auth/password/forgot", {
        method: "POST",
        body: JSON.stringify({ email }),
      });
      setMessage(body.message);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not send the email.");
    } finally {
      setPending(false);
    }
  }

  return (
    <AuthShell>
      <h1 className="text-[22px] font-medium tracking-tight">Reset password</h1>
      <p className="mt-1 mb-6 text-sm text-muted">We’ll email a link if an account exists for this address.</p>
      <form className="grid gap-4" onSubmit={submit}>
        {error ? <Alert>{error}</Alert> : null}
        {message ? <p className="text-sm">{message}</p> : null}
        <TextField label="Email" name="email" type="email" autoComplete="email" required value={email} onChange={setEmail} />
        <Button type="submit" pending={pending} className="w-full">
          {pending ? "Sending…" : "Send reset link"}
        </Button>
      </form>
      <p className="mt-6 text-sm">
        <Link className="underline" to="/sign-in">
          Back to sign in
        </Link>
      </p>
    </AuthShell>
  );
}

export function ResetPasswordPage() {
  const [params] = useSearchParams();
  const token = params.get("token") ?? "";
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [done, setDone] = useState(false);
  const [pending, setPending] = useState(false);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setPending(true);
    setError("");
    try {
      await api("/api/v1/auth/password/reset", {
        method: "POST",
        body: JSON.stringify({ token, password }),
      });
      setDone(true);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "This reset link is not valid.");
    } finally {
      setPending(false);
    }
  }

  return (
    <AuthShell>
      <h1 className="text-[22px] font-medium tracking-tight">Choose a new password</h1>
      {done ? (
        <p className="mt-4 text-sm">
          Password updated. Existing sessions were signed out.{" "}
          <Link className="underline" to="/sign-in">
            Sign in
          </Link>
        </p>
      ) : (
        <form className="mt-6 grid gap-4" onSubmit={submit}>
          {error ? <Alert>{error}</Alert> : null}
          <TextField
            label="New password"
            name="password"
            type="password"
            autoComplete="new-password"
            required
            value={password}
            onChange={setPassword}
          />
          <Button type="submit" pending={pending} className="w-full">
            {pending ? "Updating…" : "Update password"}
          </Button>
        </form>
      )}
    </AuthShell>
  );
}
