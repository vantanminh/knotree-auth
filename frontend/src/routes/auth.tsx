import { t } from "../lib/i18n";
import { useEffect, useState } from "react";
import { Navigate, useNavigate, useSearchParams } from "react-router";
import { AuthHeading, AuthShell, TextLink } from "../components/shells";
import { AlertIcon, CheckCircleIcon, GitHubLogo, GoogleLogo, KeyIcon, MailIcon } from "../components/icons";
import { Alert, Button, PasswordStrength, Spinner, TextField, buttonClass, useCountdown } from "../components/ui";
import { ApiError, api, continueAfterAuth, destinationAfterAuth, safeReturnTo } from "../lib/api";
import type { LoginResponse } from "../lib/types";

function useReturnTo() {
  const [params] = useSearchParams();
  return safeReturnTo(params.get("return_to"));
}

function withReturnTo(path: string, returnTo: string | null) {
  return returnTo ? `${path}?return_to=${encodeURIComponent(returnTo)}` : path;
}

/** Names the service that sent the user here, for "to continue to …" headings. */
function useClientName(returnTo: string | null) {
  const [clientName, setClientName] = useState<string | null>(null);
  useEffect(() => {
    if (!returnTo?.startsWith("/oauth/authorize")) return;
    void api<{ client_name: string | null }>(`/api/v1/oauth/context?return_to=${encodeURIComponent(returnTo)}`)
      .then((body) => setClientName(body.client_name))
      .catch(() => setClientName(null));
  }, [returnTo]);
  return clientName;
}

function CheckingSession() {
  return (
    <div className="flex min-h-dvh items-center justify-center">
      <p role="status" className="flex items-center gap-2.5 text-sm text-muted">
        <Spinner size={16} className="text-pine" />
        {t("Checking your session…")}
      </p>
    </div>
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

function Divider({ children }: { children: string }) {
  return (
    <div className="my-6 flex items-center gap-3 text-[12px] uppercase tracking-[0.08em] text-faint">
      <span className="h-px flex-1 bg-line" />
      {children}
      <span className="h-px flex-1 bg-line" />
    </div>
  );
}

function SocialButtons({ returnTo }: { returnTo: string | null }) {
  const social = (provider: string) =>
    `/api/v1/auth/social/${provider}/start?return_to=${encodeURIComponent(returnTo ?? "/account")}`;
  return (
    <div className="grid grid-cols-2 gap-2.5">
      <a className={buttonClass("secondary", "md", "w-full")} href={social("google")}>
        <GoogleLogo size={16} />
        {t("Google")}
      </a>
      <a className={buttonClass("secondary", "md", "w-full")} href={social("github")}>
        <GitHubLogo size={16} />
        {t("GitHub")}
      </a>
    </div>
  );
}

export function HomePage() {
  const status = useResumeSession(null);
  if (status === "checking") return <CheckingSession />;
  return <Navigate to="/sign-in" replace />;
}

export function SignInPage() {
  const returnTo = useReturnTo();
  const session = useResumeSession(returnTo);
  const [identifier, setIdentifier] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  const clientName = useClientName(returnTo);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setPending(true);
    setError("");
    try {
      const result = await api<LoginResponse>("/api/v1/auth/login", {
        method: "POST",
        body: JSON.stringify({ identifier, password }),
      });
      if (result.status === "mfa_required") {
        sessionStorage.setItem("knotree.mfa", JSON.stringify({ ...result, return_to: returnTo }));
        const method = result.methods.includes("totp") ? "totp" : result.methods[0];
        window.location.assign(`/mfa/${method === "email" ? "email" : method === "totp" ? "totp" : "recovery"}`);
        return;
      }
      continueAfterAuth(returnTo);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : t("Could not sign in."));
    } finally {
      setPending(false);
    }
  }

  if (session === "checking") return <CheckingSession />;

  return (
    <AuthShell
      footer={
        <>
          {t("New to Knotree?")}{' '}<TextLink to={withReturnTo("/sign-up", returnTo)}>{t("Create account")}</TextLink>
        </>
      }
    >
      <AuthHeading title={t("Sign in")}>
        {clientName ? (
          <>
            {t("to continue to")}{' '}<span className="font-medium text-ink">{clientName}</span>
          </>
        ) : (
          t("Use your Knotree account.")
        )}
      </AuthHeading>
      <form className="grid gap-4" onSubmit={submit}>
        {error ? <Alert>{error}</Alert> : null}
        <TextField
          label={t("Email or username")}
          name="identifier"
          type="text"
          autoComplete="username"
          required
          autoFocus
          value={identifier}
          onChange={setIdentifier}
        />
        <TextField
          label={t("Password")}
          labelAside={
            <TextLink to="/forgot-password" className="font-normal">
              {t("Forgot password?")}
            </TextLink>
          }
          name="password"
          type="password"
          autoComplete="current-password"
          required
          value={password}
          onChange={setPassword}
        />
        <Button type="submit" size="lg" pending={pending} className="mt-1 w-full">
          {pending ? t("Signing in…") : t("Continue")}
        </Button>
      </form>
      <Divider>{t("or")}</Divider>
      <SocialButtons returnTo={returnTo} />
    </AuthShell>
  );
}

export function SignUpPage() {
  const returnTo = useReturnTo();
  const session = useResumeSession(returnTo);
  const clientName = useClientName(returnTo);
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  const navigate = useNavigate();
  const mismatch = confirm.length > 0 && password !== confirm;

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (password !== confirm) {
      setError(t("Passwords do not match."));
      return;
    }
    setPending(true);
    setError("");
    try {
      await api("/api/v1/auth/register", {
        method: "POST",
        body: JSON.stringify({ username, email, password, password_confirm: confirm, return_to: returnTo }),
      });
      sessionStorage.setItem("knotree.pending-email", email);
      navigate("/verify-email");
    } catch (err) {
      setError(err instanceof ApiError ? err.message : t("Could not create the account."));
    } finally {
      setPending(false);
    }
  }

  if (session === "checking") return <CheckingSession />;

  return (
    <AuthShell
      footer={
        <>
          {t("Already have an account?")}{' '}<TextLink to={withReturnTo("/sign-in", returnTo)}>{t("Sign in")}</TextLink>
        </>
      }
    >
      <AuthHeading title={t("Create account")}>
        {clientName ? (
          <>
            {t("to continue to")}{' '}<span className="font-medium text-ink">{clientName}</span>
          </>
        ) : (
          t("One Knotree account works across Knotree services.")
        )}
      </AuthHeading>
      <form className="grid gap-4" onSubmit={submit}>
        {error ? <Alert>{error}</Alert> : null}
        <TextField
          label={t("Username")}
          name="username"
          autoComplete="username"
          placeholder={t("ada-lovelace")}
          hint={t("3–39 letters, numbers or hyphens. You can sign in with it.")}
          required
          autoFocus
          value={username}
          onChange={setUsername}
        />
        <TextField
          label={t("Email")}
          name="email"
          type="email"
          autoComplete="email"
          placeholder={t("you@example.com")}
          required
          value={email}
          onChange={setEmail}
        />
        <div className="grid gap-2">
          <TextField
            label={t("Password")}
            name="password"
            type="password"
            autoComplete="new-password"
            required
            value={password}
            onChange={setPassword}
          />
          <PasswordStrength value={password} />
        </div>
        <TextField
          label={t("Confirm password")}
          name="confirm"
          type="password"
          autoComplete="new-password"
          required
          value={confirm}
          onChange={setConfirm}
          error={mismatch ? t("Passwords do not match.") : undefined}
        />
        <Button type="submit" size="lg" pending={pending} className="mt-1 w-full">
          {pending ? t("Creating account…") : t("Create account")}
        </Button>
      </form>
      <Divider>{t("or sign up with")}</Divider>
      <SocialButtons returnTo={returnTo} />
    </AuthShell>
  );
}

// Verification tokens are single-use. StrictMode (and remounts) run the effect
// twice, so share one in-flight request per token instead of sending two.
const verifyRequests = new Map<string, Promise<unknown>>();

function verifyEmailOnce(token: string) {
  let request = verifyRequests.get(token);
  if (!request) {
    request = api("/api/v1/auth/email/verify", { method: "POST", body: JSON.stringify({ token }) });
    verifyRequests.set(token, request);
  }
  return request;
}

export function VerifyEmailPage() {
  const [params] = useSearchParams();
  const token = params.get("token") ?? "";
  const [state, setState] = useState<"waiting" | "verifying" | "verified" | "failed">(token ? "verifying" : "waiting");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [email, setEmail] = useState(sessionStorage.getItem("knotree.pending-email") ?? "");
  const [pending, setPending] = useState(false);
  const [cooldown, setCooldown] = useCountdown();
  const [continueTo, setContinueTo] = useState<string | null>(null);

  useEffect(() => {
    if (!token) return;
    let active = true;
    void verifyEmailOnce(token)
      .then((body) => {
        if (!active) return;
        const result = body as { return_to?: string | null } | null;
        setContinueTo(safeReturnTo(result?.return_to ?? null));
        setState("verified");
      })
      .catch((err: unknown) => {
        if (!active) return;
        setState("failed");
        setError(err instanceof ApiError ? err.message : t("This link is not valid."));
      });
    return () => {
      active = false;
    };
  }, [token]);

  async function resend(event: React.FormEvent) {
    event.preventDefault();
    setPending(true);
    setError("");
    try {
      await api("/api/v1/auth/email/resend", { method: "POST", body: JSON.stringify({ email }) });
      setNotice(t("If an unverified account exists for this email, we sent a new link."));
      setCooldown(30);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : t("Could not resend the email."));
    } finally {
      setPending(false);
    }
  }

  if (state === "verifying") {
    return (
      <AuthShell>
        <AuthHeading icon={<Spinner size={18} />} title={t("Verify your email")}>
          {t("Verifying your email…")}
        </AuthHeading>
      </AuthShell>
    );
  }

  if (state === "verified") {
    return (
      <AuthShell>
        <AuthHeading icon={<CheckCircleIcon size={20} />} title={t("Verify your email")}>
          {t("Email verified. You can sign in.")}
        </AuthHeading>
        <a href={withReturnTo("/sign-in", continueTo)} className={buttonClass("primary", "lg", "w-full")}>
          {t("Continue to sign in")}
        </a>
      </AuthShell>
    );
  }

  if (state === "failed") {
    return (
      <AuthShell
        footer={
          <>
            {t("Need a new link?")}{' '}<TextLink to="/verify-email">{t("Resend verification")}</TextLink>
          </>
        }
      >
        <AuthHeading icon={<AlertIcon size={20} />} title={t("Verify your email")}>
          {t("We couldn’t verify this link.")}
        </AuthHeading>
        <Alert>{error}</Alert>
        <a href="/sign-in" className={buttonClass("secondary", "lg", "mt-5 w-full")}>
          {t("Back to sign in")}
        </a>
      </AuthShell>
    );
  }

  return (
    <AuthShell
      footer={
        <>
          {t("Already verified?")}{' '}<TextLink to="/sign-in">{t("Sign in")}</TextLink>
        </>
      }
    >
      <AuthHeading icon={<MailIcon size={20} />} title={t("Verify your email")}>
        {email ? (
          <>
            {t("We sent a verification link to")}{' '}<span className="font-medium text-ink">{email}</span>{t(". Open it to activate your account.")}
          </>
        ) : (
          t("Check your inbox for a verification link.")
        )}
      </AuthHeading>
      <form className="grid gap-4" onSubmit={resend}>
        {error ? <Alert>{error}</Alert> : null}
        {notice ? <Alert tone="success">{notice}</Alert> : null}
        <TextField label={t("Email")} name="email" type="email" required value={email} onChange={setEmail} />
        <Button type="submit" variant="secondary" size="lg" pending={pending} disabled={cooldown > 0} className="w-full">
          {cooldown > 0 ? t("Resend in {seconds}s", { seconds: cooldown }) : pending ? t("Sending…") : t("Resend verification email")}
        </Button>
      </form>
      <p className="mt-5 text-[13px] text-muted">{t("Can’t find it? Check spam or promotions. Links expire after a while.")}</p>
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
        body: JSON.stringify({ identifier: email }),
      });
      setMessage(t(body.message));
    } catch (err) {
      setError(err instanceof ApiError ? err.message : t("Could not send the email."));
    } finally {
      setPending(false);
    }
  }

  return (
    <AuthShell
      footer={
        <>
          {t("Remembered it?")}{' '}<TextLink to="/sign-in">{t("Back to sign in")}</TextLink>
        </>
      }
    >
      {message ? (
        <>
          <AuthHeading icon={<MailIcon size={20} />} title={t("Check your email")}>
            {message}
          </AuthHeading>
          <Button type="button" variant="secondary" size="lg" className="w-full" onClick={() => setMessage("")}>
            {t("Use a different email")}
          </Button>
        </>
      ) : (
        <>
          <AuthHeading icon={<KeyIcon size={20} />} title={t("Reset password")}>
            {t("We’ll email a link to your verified address if the account exists.")}
          </AuthHeading>
          <form className="grid gap-4" onSubmit={submit}>
            {error ? <Alert>{error}</Alert> : null}
            <TextField
              label={t("Email or username")}
              name="identifier"
              autoComplete="username"
              required
              autoFocus
              value={email}
              onChange={setEmail}
            />
            <Button type="submit" size="lg" pending={pending} className="w-full">
              {pending ? t("Sending…") : t("Send reset link")}
            </Button>
          </form>
        </>
      )}
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
      setError(err instanceof ApiError ? err.message : t("This reset link is not valid."));
    } finally {
      setPending(false);
    }
  }

  if (done) {
    return (
      <AuthShell>
        <AuthHeading icon={<CheckCircleIcon size={20} />} title={t("Password updated")}>
          {t("Your new password is set. Existing sessions were signed out.")}
        </AuthHeading>
        <a href="/sign-in" className={buttonClass("primary", "lg", "w-full")}>
          {t("Sign in")}
        </a>
      </AuthShell>
    );
  }

  return (
    <AuthShell
      footer={
        <>
          <TextLink to="/sign-in">{t("Back to sign in")}</TextLink>
        </>
      }
    >
      <AuthHeading icon={<KeyIcon size={20} />} title={t("Choose a new password")}>
        {t("Pick something you don’t use anywhere else.")}
      </AuthHeading>
      <form className="grid gap-4" onSubmit={submit}>
        {error ? <Alert>{error}</Alert> : null}
        <div className="grid gap-2">
          <TextField
            label={t("New password")}
            name="password"
            type="password"
            autoComplete="new-password"
            required
            autoFocus
            value={password}
            onChange={setPassword}
          />
          <PasswordStrength value={password} />
        </div>
        <Button type="submit" size="lg" pending={pending} className="w-full">
          {pending ? t("Updating…") : t("Update password")}
        </Button>
      </form>
    </AuthShell>
  );
}
