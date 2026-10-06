import { t } from "../lib/i18n";
import { useEffect, useState } from "react";
import { useSearchParams } from "react-router";
import { AuthHeading, AuthShell, TextLink } from "../components/shells";
import { AlertIcon, CheckIcon, LockIcon, LogoMark } from "../components/icons";
import { Alert, Button, ClientLogo, Skeleton } from "../components/ui";
import { ApiError, api } from "../lib/api";

type ConsentContext = {
  client_id: string;
  client_name: string;
  description: string | null;
  homepage_url: string | null;
  logo_url: string | null;
  scopes: string[];
};

const scopeLabels: Record<string, string> = {
  openid: "Confirm your Knotree identity",
  profile: "See your name and username",
  email: "See your email address",
  offline_access: "Stay signed in when you are not using it",
};

export function ConsentPage() {
  const [params] = useSearchParams();
  const requestId = params.get("request") ?? "";
  const [error, setError] = useState("");
  const [pending, setPending] = useState<"approve" | "deny" | null>(null);
  const [context, setContext] = useState<ConsentContext | null>(null);

  useEffect(() => {
    if (!requestId) return;
    api<ConsentContext>(`/api/v1/oauth/consent/${encodeURIComponent(requestId)}`)
      .then(setContext)
      .catch((err: unknown) => setError(err instanceof ApiError ? err.message : t("Could not complete authorization.")));
  }, [requestId]);

  async function decide(approve: boolean) {
    setPending(approve ? "approve" : "deny");
    setError("");
    try {
      const body = await api<{ redirect_to: string }>("/api/v1/oauth/consent", {
        method: "POST",
        body: JSON.stringify({ request_id: requestId, approve }),
      });
      if (approve) window.location.assign(body.redirect_to);
      else window.location.assign("/oauth/error?error=access_denied");
    } catch (err) {
      setError(err instanceof ApiError ? err.message : t("Could not complete authorization."));
      setPending(null);
    }
  }

  return (
    <AuthShell>
      <div className="mb-6 flex items-center gap-3" aria-hidden="true">
        {context ? (
          <ClientLogo name={context.client_name} src={context.logo_url} size={44} />
        ) : (
          <Skeleton className="h-11 w-11 rounded-[10px]" />
        )}
        <span className="flex gap-1">
          <span className="h-1 w-1 rounded-full bg-line-strong" />
          <span className="h-1 w-1 rounded-full bg-line-strong" />
          <span className="h-1 w-1 rounded-full bg-line-strong" />
        </span>
        <LogoMark size={44} />
      </div>
      <AuthHeading title={context ? t("{name} wants to access your Knotree account", { name: context.client_name }) : t("Authorize application")}>
        {t("This application is asking to use your Knotree account. Continue only if you recognize it.")}
      </AuthHeading>
      {context ? (
        <div className="mb-5 grid gap-3 animate-fade-in">
          {context.description ? <p className="text-[14px] leading-relaxed text-ink-soft">{context.description}</p> : null}
          {context.homepage_url ? (
            <a className="w-fit text-[13px] font-medium text-pine hover:underline" href={context.homepage_url} target="_blank" rel="noopener noreferrer">
              {new URL(context.homepage_url).host}
            </a>
          ) : null}
          <div className="rounded-[8px] border border-line bg-white px-4 py-3">
            <p className="mb-2 text-[13px] font-medium text-ink">{t("This will allow {name} to:", { name: context.client_name })}</p>
            <ul className="grid gap-1.5">
              {context.scopes.map((scope) => (
                <li key={scope} className="flex items-start gap-2 text-[13.5px] text-ink-soft">
                  <CheckIcon size={14} className="mt-[3px] shrink-0 text-pine" />
                  {scopeLabels[scope] ? t(scopeLabels[scope]) : <code className="font-mono text-[12.5px]">{scope}</code>}
                </li>
              ))}
            </ul>
          </div>
        </div>
      ) : !error ? (
        <div className="mb-5 grid gap-2" role="status" aria-label={t("Loading…")}>
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-2/3" />
          <Skeleton className="mt-2 h-24 w-full" />
        </div>
      ) : null}
      <p className="mb-6 flex items-start gap-2.5 rounded-[8px] border border-line bg-paper/70 px-4 py-3 text-[13.5px] text-ink-soft">
        <LockIcon size={15} className="mt-[3px] shrink-0 text-pine" />
        {t("Your password is never shared with the application.")}
      </p>
      {error ? <Alert className="mb-4">{error}</Alert> : null}
      <div className="grid gap-2.5">
        <Button type="button" size="lg" pending={pending === "approve"} disabled={pending !== null || !context} onClick={() => void decide(true)}>
          {pending === "approve" ? t("Continuing…") : t("Continue")}
        </Button>
        <Button
          type="button"
          variant="secondary"
          size="lg"
          pending={pending === "deny"}
          disabled={pending !== null}
          onClick={() => void decide(false)}
        >
          {t("Cancel")}
        </Button>
      </div>
      <p className="mt-5 text-[12.5px] text-muted">
        {t("You can review signed-in devices and connected services from your account at any time.")}
      </p>
    </AuthShell>
  );
}

export function OAuthErrorPage() {
  const [params] = useSearchParams();
  const description = params.get("error_description") || params.get("error") || t("The sign-in request could not be completed.");
  const safe = description.slice(0, 240);
  return (
    <AuthShell
      footer={
        <>
          {t("Go to")}{' '}<TextLink to="/account">{t("your account")}</TextLink>
        </>
      }
    >
      <AuthHeading icon={<AlertIcon size={20} />} title={t("Could not sign in")}>
        {t("The application’s sign-in request was not completed. Return to the application and try again.")}
      </AuthHeading>
      <Alert tone="info">
        <span className="break-words font-mono text-[13px]">{safe}</span>
      </Alert>
    </AuthShell>
  );
}
