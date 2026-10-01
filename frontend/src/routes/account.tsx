import { useEffect, useState } from "react";
import { Link } from "react-router";
import { Alert, Button, PageTitle, TextField } from "../components/ui";
import { ApiError, api, signInLocation } from "../lib/api";
import { eventLabel, formatWhen } from "../lib/format";
import type { MfaSummary, Profile, SecurityEvent, SessionItem } from "../lib/types";

function useProfile() {
  const [profile, setProfile] = useState<Profile | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    void api<Profile>("/api/v1/me")
      .then(setProfile)
      .catch((err: unknown) => {
        if (err instanceof ApiError && err.status === 401) window.location.assign(signInLocation());
        else setError(err instanceof ApiError ? err.message : "Could not load your account.");
      });
  }, []);
  return { profile, error, reload: () => api<Profile>("/api/v1/me").then(setProfile) };
}

export function AccountHome() {
  const { profile, error } = useProfile();
  if (error) return <Alert>{error}</Alert>;
  if (!profile) return <p className="text-sm text-muted">Loading account…</p>;
  return (
    <section>
      <PageTitle title={profile.display_name || "Account"} detail={profile.email} />
      <dl className="grid max-w-lg gap-3 text-sm">
        <div className="flex justify-between gap-4 border-b border-line py-2">
          <dt className="text-muted">Email</dt>
          <dd className="text-right">
            {profile.email}
            <span className="block text-muted">{profile.email_verified ? "Verified" : "Unverified"}</span>
          </dd>
        </div>
        <div className="flex justify-between gap-4 border-b border-line py-2">
          <dt className="text-muted">Authenticator</dt>
          <dd>{profile.mfa.totp_enabled ? "Enabled" : "Off"}</dd>
        </div>
        <div className="flex justify-between gap-4 border-b border-line py-2">
          <dt className="text-muted">Email codes</dt>
          <dd>{profile.mfa.email_enabled ? "Enabled" : "Off"}</dd>
        </div>
      </dl>
      {profile.is_admin ? (
        <Link className="mt-6 inline-block text-sm underline" to="/admin">
          Open administration
        </Link>
      ) : null}
    </section>
  );
}

export function ProfilePage() {
  const { profile, error, reload } = useProfile();
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [message, setMessage] = useState("");
  const [formError, setFormError] = useState("");
  const [pending, setPending] = useState(false);

  useEffect(() => {
    if (profile) setName(profile.display_name ?? "");
  }, [profile]);

  if (error) return <Alert>{error}</Alert>;
  if (!profile) return <p className="text-sm text-muted">Loading profile…</p>;

  async function saveName(event: React.FormEvent) {
    event.preventDefault();
    setPending(true);
    setFormError("");
    try {
      await api("/api/v1/me/profile", { method: "PATCH", body: JSON.stringify({ display_name: name }) });
      setMessage("Name updated.");
      await reload();
    } catch (err) {
      setFormError(err instanceof ApiError ? err.message : "Could not update your name.");
    } finally {
      setPending(false);
    }
  }

  async function changeEmail(event: React.FormEvent) {
    event.preventDefault();
    setPending(true);
    setFormError("");
    try {
      await api("/api/v1/me/email", { method: "POST", body: JSON.stringify({ email }) });
      setMessage("Check the new address for a verification link.");
    } catch (err) {
      setFormError(err instanceof ApiError ? err.message : "Could not change the email.");
    } finally {
      setPending(false);
    }
  }

  return (
    <section className="max-w-lg">
      <PageTitle title="Profile" detail="Name and email for your Knotree account." />
      {formError ? <Alert>{formError}</Alert> : null}
      {message ? <p className="mb-4 text-sm">{message}</p> : null}
      <form className="grid gap-4" onSubmit={saveName}>
        <TextField label="Name" name="name" value={name} onChange={setName} required />
        <Button type="submit" pending={pending} className="w-fit">
          {pending ? "Saving…" : "Save name"}
        </Button>
      </form>
      <form className="mt-8 grid gap-4" onSubmit={changeEmail}>
        <TextField label="New email" name="email" type="email" value={email} onChange={setEmail} hint="Requires a recent sign-in." />
        <Button type="submit" variant="quiet" pending={pending} className="w-fit">
          Send verification
        </Button>
      </form>
      <DeleteAccount />
    </section>
  );
}

export function SecurityPage() {
  const [summary, setSummary] = useState<MfaSummary | null>(null);
  const [events, setEvents] = useState<SecurityEvent[]>([]);
  const [error, setError] = useState("");
  const [setup, setSetup] = useState<{ secret: string; qr_svg: string } | null>(null);
  const [code, setCode] = useState("");
  const [recovery, setRecovery] = useState<string[] | null>(null);
  const [saved, setSaved] = useState(false);
  const [password, setPassword] = useState("");
  const [nextPassword, setNextPassword] = useState("");
  const [stepPassword, setStepPassword] = useState("");
  const [stepCode, setStepCode] = useState("");
  const [message, setMessage] = useState("");

  async function load() {
    const [security, activity] = await Promise.all([
      api<MfaSummary>("/api/v1/me/security"),
      api<{ items: SecurityEvent[] }>("/api/v1/me/security-events"),
    ]);
    setSummary(security);
    setEvents(activity.items);
  }

  useEffect(() => {
    void load().catch((err: unknown) => setError(err instanceof ApiError ? err.message : "Could not load security settings."));
  }, []);

  async function elevate(event: React.FormEvent) {
    event.preventDefault();
    setError("");
    try {
      await api("/api/v1/auth/step-up", {
        method: "POST",
        body: JSON.stringify({ password: stepPassword, code: stepCode || undefined, method: "totp" }),
      });
      setMessage("Confirmed. You can continue with the sensitive change.");
      setStepPassword("");
      setStepCode("");
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not confirm it’s you.");
    }
  }

  async function beginTotp() {
    setError("");
    try {
      const body = await api<{ secret: string; qr_svg: string }>("/api/v1/me/mfa/totp/setup", { method: "POST" });
      setSetup(body);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not start authenticator setup.");
    }
  }

  async function confirmTotp(event: React.FormEvent) {
    event.preventDefault();
    try {
      const body = await api<{ recovery_codes: string[] }>("/api/v1/me/mfa/totp/confirm", {
        method: "POST",
        body: JSON.stringify({ code }),
      });
      setRecovery(body.recovery_codes);
      setSetup(null);
      await load();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "That code is not valid.");
    }
  }

  async function toggleEmail(enabled: boolean) {
    try {
      await api("/api/v1/me/mfa/email", { method: "POST", body: JSON.stringify({ enabled }) });
      await load();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not update email codes.");
    }
  }

  async function regenerate() {
    try {
      const body = await api<{ recovery_codes: string[] }>("/api/v1/me/mfa/recovery-codes", { method: "POST" });
      setRecovery(body.recovery_codes);
      setSaved(false);
      await load();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not regenerate recovery codes.");
    }
  }

  async function changePassword(event: React.FormEvent) {
    event.preventDefault();
    try {
      await api("/api/v1/auth/password/change", {
        method: "POST",
        body: JSON.stringify({ current_password: password, new_password: nextPassword }),
      });
      setMessage("Password changed. Other sessions were signed out.");
      setPassword("");
      setNextPassword("");
      await load();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not change the password.");
    }
  }

  async function disableTotp(event: React.FormEvent) {
    event.preventDefault();
    try {
      await api("/api/v1/me/mfa/totp/disable", {
        method: "POST",
        body: JSON.stringify({ code, method: "totp" }),
      });
      setCode("");
      setMessage("Authenticator disabled.");
      await load();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not disable the authenticator.");
    }
  }

  if (!summary && !error) return <p className="text-sm text-muted">Loading security…</p>;

  return (
    <section className="max-w-xl">
      <PageTitle title="Security" detail="Password, two-factor authentication, and recent activity." />
      {error ? <Alert>{error}</Alert> : null}
      {message ? <p className="mb-4 text-sm">{message}</p> : null}
      <form className="mb-8 grid gap-3 border-b border-line pb-8" onSubmit={elevate}>
        <h2 className="text-base font-medium">Confirm it’s you</h2>
        <p className="text-sm text-muted">Sensitive changes ask for your password again, and an authenticator code when one is enabled.</p>
        <TextField label="Password" name="step-password" type="password" autoComplete="current-password" value={stepPassword} onChange={setStepPassword} />
        <TextField label="Authenticator code" name="step-code" autoComplete="one-time-code" value={stepCode} onChange={setStepCode} />
        <Button type="submit" variant="quiet" className="w-fit">
          Confirm
        </Button>
      </form>
      <form className="mb-8 grid gap-3 border-b border-line pb-8" onSubmit={changePassword}>
        <h2 className="text-base font-medium">Password</h2>
        <p className="text-sm text-muted">Last changed {formatWhen(summary?.password_changed_at)}</p>
        <TextField label="Current password" name="current" type="password" autoComplete="current-password" value={password} onChange={setPassword} />
        <TextField label="New password" name="next" type="password" autoComplete="new-password" value={nextPassword} onChange={setNextPassword} />
        <Button type="submit" className="w-fit">
          Change password
        </Button>
      </form>
      <div className="mb-8 border-b border-line pb-8">
        <h2 className="text-base font-medium">Two-factor authentication</h2>
        <div className="mt-4 flex items-center justify-between gap-4 text-sm">
          <div>
            <p>Authenticator app</p>
            <p className="text-muted">{summary?.totp_enabled ? "Enabled" : "Disabled"}</p>
          </div>
          {summary?.totp_enabled ? null : (
            <Button type="button" variant="quiet" onClick={() => void beginTotp()}>
              Enable
            </Button>
          )}
        </div>
        {setup ? (
          <form className="mt-4 grid gap-3" onSubmit={confirmTotp}>
            <ol className="list-decimal space-y-1 pl-5 text-sm text-muted">
              <li>Open your authenticator app.</li>
              <li>Scan this QR code.</li>
              <li>Enter the 6-digit code.</li>
            </ol>
            <div className="w-fit bg-white p-2" dangerouslySetInnerHTML={{ __html: setup.qr_svg }} />
            <p className="text-sm">
              Can’t scan? Enter this setup key manually: <span className="font-medium tracking-wide">{setup.secret}</span>
            </p>
            <TextField label="6-digit code" name="totp" autoComplete="one-time-code" value={code} onChange={setCode} />
            <Button type="submit" className="w-fit">
              Enable authenticator
            </Button>
          </form>
        ) : null}
        {summary?.totp_enabled ? (
          <form className="mt-4 grid gap-3" onSubmit={disableTotp}>
            <TextField label="Current authenticator code" name="disable" value={code} onChange={setCode} />
            <Button type="submit" variant="danger" className="w-fit">
              Disable authenticator
            </Button>
          </form>
        ) : null}
        <div className="mt-5 flex items-center justify-between gap-4 text-sm">
          <div>
            <p>Email verification code</p>
            <p className="text-muted">{summary?.email_enabled ? "Enabled" : "Available"}</p>
          </div>
          <Button type="button" variant="quiet" onClick={() => void toggleEmail(!summary?.email_enabled)}>
            {summary?.email_enabled ? "Disable" : "Enable"}
          </Button>
        </div>
        <div className="mt-5 flex items-center justify-between gap-4 text-sm">
          <div>
            <p>Recovery codes</p>
            <p className="text-muted">{summary?.recovery_codes_remaining ?? 0} remaining</p>
          </div>
          <Button type="button" variant="quiet" onClick={() => void regenerate()}>
            Regenerate
          </Button>
        </div>
        {recovery ? (
          <div className="mt-4">
            <p className="text-sm">Save your recovery codes. Each code works once. This is the only time they are shown.</p>
            <ul className="mt-2 grid gap-1 font-mono text-sm">
              {recovery.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
            <label className="mt-3 flex items-center gap-2 text-sm">
              <input type="checkbox" checked={saved} onChange={(event) => setSaved(event.target.checked)} />
              I have saved these codes
            </label>
            <Button type="button" className="mt-3" disabled={!saved} onClick={() => setRecovery(null)}>
              Continue
            </Button>
          </div>
        ) : null}
      </div>
      <h2 className="text-base font-medium">Recent security activity</h2>
      <ul className="mt-3 divide-y divide-line text-sm">
        {events.length === 0 ? <li className="py-2 text-muted">No activity yet.</li> : null}
        {events.map((event) => (
          <li key={event.id} className="flex justify-between gap-4 py-2">
            <span>{eventLabel(event.event_type)}</span>
            <time className="text-muted" dateTime={event.occurred_at}>
              {formatWhen(event.occurred_at)}
            </time>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function SessionsPage() {
  const [items, setItems] = useState<SessionItem[]>([]);
  const [error, setError] = useState("");

  async function load() {
    const body = await api<{ items: SessionItem[] }>("/api/v1/me/sessions");
    setItems(body.items);
  }

  useEffect(() => {
    void load().catch((err: unknown) => setError(err instanceof ApiError ? err.message : "Could not load sessions."));
  }, []);

  async function revoke(id: string) {
    await api(`/api/v1/me/sessions/${id}`, { method: "DELETE" });
    if (items.find((item) => item.id === id)?.current) {
      window.location.assign("/sign-in");
      return;
    }
    await load();
  }

  async function revokeOthers() {
    try {
      await api("/api/v1/me/sessions/revoke-others", { method: "POST" });
      await load();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not revoke sessions.");
    }
  }

  return (
    <section>
      <PageTitle title="Sessions" detail="Devices currently signed in to your Knotree account." />
      {error ? <Alert>{error}</Alert> : null}
      <Button type="button" variant="quiet" className="mb-4" onClick={() => void revokeOthers()}>
        Sign out other sessions
      </Button>
      <ul className="divide-y divide-line">
        {items.map((item) => (
          <li key={item.id} className="flex flex-col gap-2 py-3 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <p className="text-sm">
                {item.device}
                {item.current ? " · This device" : ""}
              </p>
              <p className="text-sm text-muted">
                {item.ip ?? "IP unavailable"} · Active {formatWhen(item.last_active_at)}
              </p>
            </div>
            <Button type="button" variant="quiet" onClick={() => void revoke(item.id)}>
              {item.current ? "Sign out" : "Revoke"}
            </Button>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function ConnectedAccountsPage() {
  const { profile, error, reload } = useProfile();
  const [formError, setFormError] = useState("");
  if (error) return <Alert>{error}</Alert>;
  if (!profile) return <p className="text-sm text-muted">Loading connected accounts…</p>;

  async function unlink(provider: string) {
    try {
      await api(`/api/v1/me/identities/${provider}`, { method: "DELETE" });
      await reload();
    } catch (err) {
      setFormError(err instanceof ApiError ? err.message : "Could not disconnect that account.");
    }
  }

  return (
    <section className="max-w-lg">
      <PageTitle title="Connected accounts" detail="Google and GitHub can sign in to the same Knotree account. Matching emails are not linked automatically." />
      {formError ? <Alert>{formError}</Alert> : null}
      <ul className="divide-y divide-line text-sm">
        {profile.identities.map((identity) => (
          <li key={identity.provider} className="flex items-center justify-between gap-4 py-3">
            <span>
              {identity.provider}
              {identity.email ? ` · ${identity.email}` : ""}
            </span>
            {identity.provider === "password" ? null : (
              <Button type="button" variant="quiet" onClick={() => void unlink(identity.provider)}>
                Disconnect
              </Button>
            )}
          </li>
        ))}
      </ul>
      <div className="mt-6 grid gap-2">
        <a className="text-sm underline" href="/api/v1/auth/social/google/start?mode=link&return_to=%2Faccount%2Fconnected-accounts">
          Connect Google
        </a>
        <a className="text-sm underline" href="/api/v1/auth/social/github/start?mode=link&return_to=%2Faccount%2Fconnected-accounts">
          Connect GitHub
        </a>
      </div>
    </section>
  );
}

export function DeleteAccount() {
  const [error, setError] = useState("");
  async function remove() {
    if (!window.confirm("Delete this Knotree account? Sessions will end. This cannot be undone from the product.")) return;
    try {
      await api("/api/v1/me/deletion", { method: "POST" });
      window.location.assign("/sign-in");
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not delete the account.");
    }
  }
  return (
    <div className="mt-10 border-t border-line pt-6">
      {error ? <Alert>{error}</Alert> : null}
      <Button type="button" variant="danger" onClick={() => void remove()}>
        Delete account
      </Button>
    </div>
  );
}
