import { useState } from "react";
import { useSearchParams } from "react-router";
import { AuthHeading, AuthShell, TextLink } from "../components/shells";
import { AlertIcon, AppIcon, LockIcon, LogoMark } from "../components/icons";
import { Alert, Button } from "../components/ui";
import { ApiError, api } from "../lib/api";

export function ConsentPage() {
  const [params] = useSearchParams();
  const requestId = params.get("request") ?? "";
  const [error, setError] = useState("");
  const [pending, setPending] = useState<"approve" | "deny" | null>(null);

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
      setError(err instanceof ApiError ? err.message : "Could not complete authorization.");
      setPending(null);
    }
  }

  return (
    <AuthShell>
      <div className="mb-6 flex items-center gap-3" aria-hidden="true">
        <span className="flex h-11 w-11 items-center justify-center rounded-[10px] border border-line bg-white text-ink-soft shadow-card">
          <AppIcon size={20} />
        </span>
        <span className="flex gap-1">
          <span className="h-1 w-1 rounded-full bg-line-strong" />
          <span className="h-1 w-1 rounded-full bg-line-strong" />
          <span className="h-1 w-1 rounded-full bg-line-strong" />
        </span>
        <LogoMark size={44} />
      </div>
      <AuthHeading title="Authorize application">
        This application is asking to use your Knotree account. Continue only if you recognize it.
      </AuthHeading>
      <p className="mb-6 flex items-start gap-2.5 rounded-[8px] border border-line bg-paper/70 px-4 py-3 text-[13.5px] text-ink-soft">
        <LockIcon size={15} className="mt-[3px] shrink-0 text-pine" />
        Your password is never shared with the application.
      </p>
      {error ? <Alert className="mb-4">{error}</Alert> : null}
      <div className="grid gap-2.5">
        <Button type="button" size="lg" pending={pending === "approve"} disabled={pending !== null} onClick={() => void decide(true)}>
          {pending === "approve" ? "Continuing…" : "Continue"}
        </Button>
        <Button
          type="button"
          variant="secondary"
          size="lg"
          pending={pending === "deny"}
          disabled={pending !== null}
          onClick={() => void decide(false)}
        >
          Cancel
        </Button>
      </div>
      <p className="mt-5 text-[12.5px] text-muted">
        You can review signed-in devices and connected services from your account at any time.
      </p>
    </AuthShell>
  );
}

export function OAuthErrorPage() {
  const [params] = useSearchParams();
  const description = params.get("error_description") || params.get("error") || "The sign-in request could not be completed.";
  const safe = description.slice(0, 240);
  return (
    <AuthShell
      footer={
        <>
          Go to <TextLink to="/account">your account</TextLink>
        </>
      }
    >
      <AuthHeading icon={<AlertIcon size={20} />} title="Could not sign in">
        The application’s sign-in request was not completed. Return to the application and try again.
      </AuthHeading>
      <Alert tone="info">
        <span className="break-words font-mono text-[13px]">{safe}</span>
      </Alert>
    </AuthShell>
  );
}
