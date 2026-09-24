import { useEffect, useState } from "react";
import { useSearchParams } from "react-router";
import { AuthShell } from "../components/shells";
import { Alert, Button } from "../components/ui";
import { ApiError, api } from "../lib/api";

export function ConsentPage() {
  const [params] = useSearchParams();
  const requestId = params.get("request") ?? "";
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);

  async function decide(approve: boolean) {
    setPending(true);
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
      setPending(false);
    }
  }

  return (
    <AuthShell>
      <h1 className="text-[22px] font-medium">Authorize application</h1>
      <p className="mt-2 mb-6 text-sm text-muted">
        This application is asking to use your Knotree account. Continue only if you recognize it.
      </p>
      {error ? <Alert>{error}</Alert> : null}
      <div className="grid gap-2">
        <Button type="button" pending={pending} onClick={() => void decide(true)}>
          {pending ? "Continuing…" : "Continue"}
        </Button>
        <Button type="button" variant="quiet" onClick={() => void decide(false)}>
          Cancel
        </Button>
      </div>
    </AuthShell>
  );
}

export function OAuthErrorPage() {
  const [params] = useSearchParams();
  const description = params.get("error_description") || params.get("error") || "The sign-in request could not be completed.";
  const [safe, setSafe] = useState(description);
  useEffect(() => {
    setSafe(description.slice(0, 240));
  }, [description]);
  return (
    <AuthShell>
      <h1 className="text-[22px] font-medium">Could not sign in</h1>
      <p className="mt-2 text-sm text-muted">{safe}</p>
    </AuthShell>
  );
}
