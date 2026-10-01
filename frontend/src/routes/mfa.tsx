import { t } from "../lib/i18n";
import { useEffect, useState, type ReactNode } from "react";
import { Link, useNavigate } from "react-router";
import { AuthHeading, AuthShell, TextLink } from "../components/shells";
import { ChevronRightIcon, KeyIcon, MailIcon, SmartphoneCodeIcon } from "../components/icons";
import { Alert, Button, CodeField, useCountdown } from "../components/ui";
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

function Alternative({ to, icon, children }: { to: string; icon: ReactNode; children: ReactNode }) {
  return (
    <Link
      to={to}
      className="group flex items-center gap-3 rounded-[8px] border border-line bg-white px-3 py-2.5 text-sm text-ink-soft transition-colors hover:border-line-strong hover:text-ink"
    >
      <span className="text-faint group-hover:text-ink-soft">{icon}</span>
      <span className="flex-1">{children}</span>
      <ChevronRightIcon className="text-faint transition-transform group-hover:translate-x-0.5" />
    </Link>
  );
}

export function MfaPage({ method }: { method: "totp" | "email" | "recovery" }) {
  const navigate = useNavigate();
  const challenge = loadChallenge();
  const [code, setCode] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [pending, setPending] = useState(false);
  const [sending, setSending] = useState(false);
  const [cooldown, setCooldown] = useCountdown();

  useEffect(() => {
    if (!challenge) navigate("/sign-in", { replace: true });
  }, [challenge, navigate]);

  useEffect(() => {
    setCode("");
    setError("");
    setNotice("");
  }, [method]);

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
      setError(err instanceof ApiError ? err.message : t("That code is not valid."));
    } finally {
      setPending(false);
    }
  }

  async function resend() {
    setSending(true);
    setError("");
    try {
      await api("/api/v1/auth/mfa/email/send", {
        method: "POST",
        body: JSON.stringify({ mfa_token: challenge!.mfa_token }),
      });
      setNotice(t("A new code is on its way."));
      setCooldown(30);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : t("Could not send a new code."));
    } finally {
      setSending(false);
    }
  }

  const title = method === "recovery" ? t("Use a recovery code") : t("Verify it’s you");
  const icon =
    method === "totp" ? <SmartphoneCodeIcon size={20} /> : method === "email" ? <MailIcon size={20} /> : <KeyIcon size={20} />;
  const detail =
    method === "totp" ? (
      t("Enter the 6-digit code from your authenticator app.")
    ) : method === "email" ? (
      <>
        {t("We sent a verification code to")}{' '}<span className="font-medium text-ink">{challenge.masked_email ?? t("your email")}</span>{t(". It expires in a few minutes. Do not share it.")}
      </>
    ) : (
      t("Enter one unused recovery code. Each code works once.")
    );

  const alternatives = [
    challenge.methods.includes("totp") && method !== "totp" ? (
      <Alternative key="totp" to="/mfa/totp" icon={<SmartphoneCodeIcon />}>
        {t("Use authenticator instead")}
      </Alternative>
    ) : null,
    challenge.methods.includes("email") && method !== "email" ? (
      <Alternative key="email" to="/mfa/email" icon={<MailIcon />}>
        {t("Use email code instead")}
      </Alternative>
    ) : null,
    method !== "recovery" ? (
      <Alternative key="recovery" to="/mfa/recovery" icon={<KeyIcon />}>
        {t("Use a recovery code")}
      </Alternative>
    ) : null,
  ].filter(Boolean);

  return (
    <AuthShell
      footer={
        <>
          {t("Not you?")}{' '}<TextLink to="/sign-in">{t("Sign in with another account")}</TextLink>
        </>
      }
    >
      <AuthHeading icon={icon} title={title}>
        {detail}
      </AuthHeading>
      <form className="grid gap-4" onSubmit={submit}>
        {error ? <Alert>{error}</Alert> : null}
        {notice && !error ? <Alert tone="success">{notice}</Alert> : null}
        <CodeField
          label={method === "recovery" ? t("Recovery code") : t("Verification code")}
          recovery={method === "recovery"}
          autoFocus
          value={code}
          onChange={setCode}
        />
        <Button type="submit" size="lg" pending={pending} className="w-full">
          {pending ? t("Verifying…") : t("Verify")}
        </Button>
        {method === "email" ? (
          <p className="text-center text-[13px] text-muted">
            {t("Didn’t get it?")}{" "}
            <button
              type="button"
              className="font-medium text-pine underline decoration-pine/25 underline-offset-[3px] hover:decoration-pine disabled:cursor-not-allowed disabled:text-muted disabled:no-underline"
              disabled={cooldown > 0 || sending}
              onClick={() => void resend()}
            >
              {cooldown > 0 ? t("Resend in {seconds}s", { seconds: cooldown }) : sending ? t("Sending…") : t("Resend code")}
            </button>
          </p>
        ) : null}
      </form>
      {alternatives.length ? (
        <div className="mt-6 border-t border-line pt-5">
          <p className="mb-2.5 text-[12px] font-medium uppercase tracking-[0.08em] text-faint">{t("Other options")}</p>
          <div className="grid gap-2">{alternatives}</div>
        </div>
      ) : null}
    </AuthShell>
  );
}
