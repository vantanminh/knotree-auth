import { useEffect, useState, type ReactNode } from "react";
import { Link } from "react-router";
import { useProfile } from "../components/shells";
import {
  AlertIcon,
  AppIcon,
  CheckCircleIcon,
  ChevronRightIcon,
  DevicesIcon,
  DownloadIcon,
  GitHubLogo,
  GoogleLogo,
  KeyIcon,
  LinkIcon,
  LockIcon,
  MailIcon,
  MonitorIcon,
  PhoneIcon,
  ShieldCheckIcon,
  ShieldIcon,
  SmartphoneCodeIcon,
  TrashIcon,
  UserIcon,
} from "../components/icons";
import {
  Alert,
  Avatar,
  Badge,
  Button,
  Card,
  CardBody,
  CardFooter,
  CardHeader,
  Checkbox,
  CodeField,
  ConfirmDialog,
  CopyButton,
  EmptyState,
  PageSkeleton,
  PageTitle,
  PasswordStrength,
  Row,
  Skeleton,
  Switch,
  TextField,
  buttonClass,
} from "../components/ui";
import { ApiError, api } from "../lib/api";
import { eventLabel, formatDate, formatRelative, formatWhen, isMobileDevice, providerLabel } from "../lib/format";
import type { MfaSummary, SecurityEvent, SessionItem } from "../lib/types";

type Status = { tone: "success" | "error"; text: string } | null;

function errorText(err: unknown, fallback: string) {
  return err instanceof ApiError ? err.message : fallback;
}

function StatusLine({ status }: { status: Status }) {
  if (!status) return null;
  return (
    <Alert tone={status.tone} className="mb-4">
      {status.text}
    </Alert>
  );
}

/* ------------------------------------------------------------------ */
/* Overview                                                            */
/* ------------------------------------------------------------------ */

function CheckItem({ done, title, detail, to }: { done: boolean; title: string; detail: string; to: string }) {
  return (
    <Link
      to={to}
      className="group flex items-center gap-3 px-5 py-3.5 transition-colors hover:bg-paper/80 sm:px-6"
    >
      <span
        className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-full ${
          done ? "bg-pine-soft text-pine" : "bg-amber-soft text-amber"
        }`}
      >
        {done ? <CheckCircleIcon size={15} /> : <AlertIcon size={15} />}
      </span>
      <span className="min-w-0 flex-1">
        <span className="block text-sm font-medium text-ink">{title}</span>
        <span className="block text-[13px] text-muted">{detail}</span>
      </span>
      <ChevronRightIcon className="shrink-0 text-faint transition-transform group-hover:translate-x-0.5 group-hover:text-ink-soft" />
    </Link>
  );
}

function Shortcut({ to, icon, title, detail }: { to: string; icon: ReactNode; title: string; detail: string }) {
  return (
    <Link
      to={to}
      className="group flex items-start gap-3 rounded-[var(--radius-card)] border border-line bg-surface p-4 shadow-card transition-[border-color,box-shadow] duration-150 hover:border-line-strong hover:shadow-raised"
    >
      <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[8px] border border-line bg-paper text-ink-soft transition-colors group-hover:text-pine">
        {icon}
      </span>
      <span className="min-w-0 flex-1">
        <span className="flex items-center justify-between gap-2 text-sm font-medium text-ink">
          {title}
          <ChevronRightIcon className="text-faint transition-transform group-hover:translate-x-0.5" />
        </span>
        <span className="mt-0.5 block text-[13px] text-muted">{detail}</span>
      </span>
    </Link>
  );
}

export function AccountHome() {
  const { profile, error } = useProfile();
  if (error) return <Alert>{error}</Alert>;
  if (!profile) return <PageSkeleton label="Loading account…" />;

  const twoFactor = profile.mfa.totp_enabled || profile.mfa.email_enabled;
  const recoveryReady = !profile.mfa.totp_enabled || profile.mfa.recovery_codes_remaining > 0;
  const checks = [profile.email_verified, twoFactor, recoveryReady];
  const done = checks.filter(Boolean).length;

  return (
    <div className="grid gap-6">
      <PageTitle title={profile.display_name || "Account"} detail="Manage your Knotree identity, security, and signed-in devices." />

      <Card>
        <div className="flex flex-col gap-4 px-5 py-5 sm:flex-row sm:items-center sm:px-6">
          <Avatar name={profile.display_name || profile.email} size={52} />
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <p className="truncate text-[15px] font-medium text-ink">{profile.email}</p>
              {profile.email_verified ? (
                <Badge tone="success" dot>
                  Verified
                </Badge>
              ) : (
                <Badge tone="warning" dot>
                  Unverified
                </Badge>
              )}
              {profile.is_admin ? <Badge tone="outline">Administrator</Badge> : null}
            </div>
            <p className="mt-0.5 text-[13px] text-muted">Member since {formatDate(profile.created_at)}</p>
          </div>
          <Link to="/account/profile" className={buttonClass("secondary", "md", "self-start sm:self-auto")}>
            Edit profile
          </Link>
        </div>
      </Card>

      <Card>
        <CardHeader
          icon={<ShieldCheckIcon />}
          title="Security checkup"
          description={
            done === checks.length
              ? "Your account follows every recommendation."
              : `${done} of ${checks.length} recommendations complete.`
          }
          action={
            <span className="tabular hidden text-[13px] font-medium text-muted sm:inline">
              {Math.round((done / checks.length) * 100)}%
            </span>
          }
        />
        <div className="px-5 pb-1 pt-4 sm:px-6">
          <div className="h-1.5 overflow-hidden rounded-full bg-sunken">
            <div
              className={`h-full rounded-full transition-[width] duration-500 ${done === checks.length ? "bg-pine" : "bg-amber"}`}
              style={{ width: `${(done / checks.length) * 100}%` }}
            />
          </div>
        </div>
        <div className="mt-3 divide-y divide-line border-t border-line">
          <CheckItem
            done={profile.email_verified}
            title="Email address"
            detail={profile.email_verified ? "Verified and ready for account recovery." : "Verify your email to recover your account."}
            to="/account/profile"
          />
          <CheckItem
            done={twoFactor}
            title="Two-factor authentication"
            detail={
              profile.mfa.totp_enabled
                ? "Authenticator app is on."
                : profile.mfa.email_enabled
                  ? "Email codes are on. An authenticator app is stronger."
                  : "Add a second step when you sign in."
            }
            to="/account/security"
          />
          <CheckItem
            done={recoveryReady}
            title="Recovery codes"
            detail={
              profile.mfa.totp_enabled
                ? `${profile.mfa.recovery_codes_remaining} unused codes available.`
                : "Generated when you turn on an authenticator app."
            }
            to="/account/security"
          />
        </div>
      </Card>

      <div className="grid gap-3 sm:grid-cols-2">
        <Shortcut to="/account/profile" icon={<UserIcon />} title="Profile" detail="Name and email address." />
        <Shortcut to="/account/security" icon={<LockIcon />} title="Security" detail="Password, two-factor, and activity." />
        <Shortcut to="/account/sessions" icon={<DevicesIcon />} title="Sessions" detail="Devices signed in to your account." />
        <Shortcut
          to="/account/connected-accounts"
          icon={<LinkIcon />}
          title="Connected accounts"
          detail="Sign in with Google or GitHub."
        />
      </div>

      {profile.is_admin ? (
        <Link
          to="/admin"
          className="group flex items-center gap-3 rounded-[var(--radius-card)] border border-dashed border-line-strong px-4 py-3.5 text-sm transition-colors hover:border-pine-line hover:bg-pine-soft/40"
        >
          <AppIcon className="text-pine" />
          <span className="flex-1">
            <span className="font-medium text-ink">Open administration</span>
            <span className="text-muted"> · Users, security events, and email logs</span>
          </span>
          <ChevronRightIcon className="text-faint transition-transform group-hover:translate-x-0.5" />
        </Link>
      ) : null}
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Profile                                                             */
/* ------------------------------------------------------------------ */

export function ProfilePage() {
  const { profile, error, reload } = useProfile();
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [nameStatus, setNameStatus] = useState<Status>(null);
  const [emailStatus, setEmailStatus] = useState<Status>(null);
  const [pending, setPending] = useState<"name" | "email" | null>(null);

  useEffect(() => {
    if (profile) setName(profile.display_name ?? "");
  }, [profile]);

  if (error) return <Alert>{error}</Alert>;
  if (!profile) return <PageSkeleton label="Loading profile…" />;

  async function saveName(event: React.FormEvent) {
    event.preventDefault();
    setPending("name");
    setNameStatus(null);
    try {
      await api("/api/v1/me/profile", { method: "PATCH", body: JSON.stringify({ display_name: name }) });
      await reload();
      setNameStatus({ tone: "success", text: "Name updated." });
    } catch (err) {
      setNameStatus({ tone: "error", text: errorText(err, "Could not update your name.") });
    } finally {
      setPending(null);
    }
  }

  async function changeEmail(event: React.FormEvent) {
    event.preventDefault();
    setPending("email");
    setEmailStatus(null);
    try {
      await api("/api/v1/me/email", { method: "POST", body: JSON.stringify({ email }) });
      setEmailStatus({ tone: "success", text: "Check the new address for a verification link." });
      setEmail("");
    } catch (err) {
      setEmailStatus({ tone: "error", text: errorText(err, "Could not change the email.") });
    } finally {
      setPending(null);
    }
  }

  const nameUnchanged = name.trim() === (profile.display_name ?? "");

  return (
    <div className="grid gap-6">
      <PageTitle title="Profile" detail="Name and email for your Knotree account." />

      <Card>
        <form onSubmit={saveName}>
          <CardHeader title="Display name" description="Shown to Knotree services you sign in to." />
          <CardBody>
            <StatusLine status={nameStatus} />
            <div className="flex items-end gap-4">
              <Avatar name={name || profile.email} size={40} />
              <TextField
                className="flex-1"
                label="Name"
                name="name"
                autoComplete="name"
                placeholder="Your name"
                value={name}
                onChange={setName}
                required
              />
            </div>
          </CardBody>
          <CardFooter note="Use the name people know you by.">
            <Button type="submit" pending={pending === "name"} disabled={nameUnchanged || !name.trim()}>
              {pending === "name" ? "Saving…" : "Save name"}
            </Button>
          </CardFooter>
        </form>
      </Card>

      <Card>
        <form onSubmit={changeEmail}>
          <CardHeader title="Email address" description="Used to sign in and to recover your account." />
          <CardBody className="grid gap-5">
            <div className="flex flex-wrap items-center gap-3 rounded-[8px] border border-line bg-paper/70 px-3.5 py-3">
              <MailIcon className="text-faint" />
              <span className="min-w-0 flex-1 truncate text-sm text-ink">{profile.email}</span>
              {profile.email_verified ? (
                <Badge tone="success" dot>
                  Verified
                </Badge>
              ) : (
                <Badge tone="warning" dot>
                  Unverified
                </Badge>
              )}
            </div>
            <div>
              <StatusLine status={emailStatus} />
              <TextField
                label="New email"
                name="email"
                type="email"
                autoComplete="email"
                placeholder="new@example.com"
                value={email}
                onChange={setEmail}
              />
            </div>
          </CardBody>
          <CardFooter note="Requires a recent sign-in. We’ll send a link to the new address.">
            <Button type="submit" variant="secondary" pending={pending === "email"} disabled={!email}>
              {pending === "email" ? "Sending…" : "Send verification"}
            </Button>
          </CardFooter>
        </form>
      </Card>

      <DeleteAccount />
    </div>
  );
}

export function DeleteAccount() {
  const [error, setError] = useState("");
  const [open, setOpen] = useState(false);
  async function remove() {
    setError("");
    try {
      await api("/api/v1/me/deletion", { method: "POST" });
      window.location.assign("/sign-in");
    } catch (err) {
      setOpen(false);
      setError(errorText(err, "Could not delete the account."));
    }
  }
  return (
    <Card tone="danger">
      <CardHeader
        title="Delete account"
        description="Permanently remove your Knotree account. You will be signed out of every Knotree service."
      />
      {error ? (
        <CardBody className="pt-4">
          <Alert>{error}</Alert>
        </CardBody>
      ) : (
        <div className="h-5" />
      )}
      <CardFooter note="This cannot be undone from the product.">
        <Button type="button" variant="danger" icon={<TrashIcon size={15} />} onClick={() => setOpen(true)}>
          Delete account
        </Button>
      </CardFooter>
      <ConfirmDialog
        open={open}
        onOpenChange={setOpen}
        danger
        title="Delete this Knotree account?"
        description="Sessions will end and applications using Knotree sign-in will lose access. This cannot be undone from the product."
        confirmLabel="Delete account"
        pendingLabel="Deleting…"
        onConfirm={remove}
      />
    </Card>
  );
}

/* ------------------------------------------------------------------ */
/* Security                                                            */
/* ------------------------------------------------------------------ */

function downloadCodes(codes: string[]) {
  const blob = new Blob(
    [`Knotree recovery codes\nGenerated ${new Date().toISOString()}\n\nEach code works once.\n\n${codes.join("\n")}\n`],
    { type: "text/plain" },
  );
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = "knotree-recovery-codes.txt";
  link.click();
  URL.revokeObjectURL(url);
}

function RecoveryCodes({ codes, onDone }: { codes: string[]; onDone: () => void }) {
  const [saved, setSaved] = useState(false);
  return (
    <div className="mx-5 mb-5 animate-fade-up rounded-[8px] border border-amber-line bg-amber-soft/50 p-4 sm:mx-6">
      <p className="text-sm font-medium text-ink">Save your recovery codes</p>
      <p className="mt-0.5 text-[13px] text-muted">Each code works once. This is the only time they are shown.</p>
      <ul className="mt-4 grid grid-cols-2 gap-x-4 gap-y-1.5 rounded-[6px] border border-line bg-white px-4 py-3 font-mono text-[14px] text-ink sm:grid-cols-2">
        {codes.map((item) => (
          <li key={item} className="tabular">
            {item}
          </li>
        ))}
      </ul>
      <div className="mt-3 flex flex-wrap gap-2">
        <CopyButton value={codes.join("\n")} label="Copy all" />
        <Button type="button" variant="secondary" size="sm" icon={<DownloadIcon size={14} />} onClick={() => downloadCodes(codes)}>
          Download
        </Button>
      </div>
      <div className="mt-4 flex flex-col gap-3 border-t border-amber-line/70 pt-4 sm:flex-row sm:items-center sm:justify-between">
        <Checkbox checked={saved} onCheckedChange={setSaved}>
          I have saved these codes
        </Checkbox>
        <Button type="button" disabled={!saved} onClick={onDone}>
          Continue
        </Button>
      </div>
    </div>
  );
}

export function ActivityList({ events, empty }: { events: SecurityEvent[]; empty: string }) {
  if (events.length === 0) return <EmptyState icon={<ShieldIcon />} title="No activity yet">{empty}</EmptyState>;
  return (
    <ol className="divide-y divide-line">
      {events.map((event) => (
        <li key={event.id} className="flex items-start gap-3 px-5 py-3 sm:px-6">
          <span
            className={`mt-[7px] h-2 w-2 shrink-0 rounded-full ${
              event.result === "failure" ? "bg-danger" : event.result === "success" ? "bg-pine" : "bg-faint"
            }`}
          />
          <div className="min-w-0 flex-1">
            <p className="text-sm text-ink">{eventLabel(event.event_type)}</p>
            {event.ip || event.user_agent ? (
              <p className="truncate text-[12.5px] text-muted">{[event.ip, event.user_agent].filter(Boolean).join(" · ")}</p>
            ) : null}
          </div>
          <time
            className="tabular shrink-0 text-[12.5px] text-muted"
            dateTime={event.occurred_at}
            title={formatWhen(event.occurred_at)}
          >
            {formatRelative(event.occurred_at)}
          </time>
        </li>
      ))}
    </ol>
  );
}

export function SecurityPage() {
  const [summary, setSummary] = useState<MfaSummary | null>(null);
  const [events, setEvents] = useState<SecurityEvent[]>([]);
  const [loadError, setLoadError] = useState("");

  const [stepPassword, setStepPassword] = useState("");
  const [stepCode, setStepCode] = useState("");
  const [stepStatus, setStepStatus] = useState<Status>(null);

  const [password, setPassword] = useState("");
  const [nextPassword, setNextPassword] = useState("");
  const [passwordStatus, setPasswordStatus] = useState<Status>(null);

  const [mfaStatus, setMfaStatus] = useState<Status>(null);
  const [setup, setSetup] = useState<{ secret: string; qr_svg: string } | null>(null);
  const [setupCode, setSetupCode] = useState("");
  const [disabling, setDisabling] = useState(false);
  const [disableCode, setDisableCode] = useState("");
  const [recovery, setRecovery] = useState<string[] | null>(null);
  const [regenOpen, setRegenOpen] = useState(false);

  const [pending, setPending] = useState<string | null>(null);

  async function load() {
    const [security, activity] = await Promise.all([
      api<MfaSummary>("/api/v1/me/security"),
      api<{ items: SecurityEvent[] }>("/api/v1/me/security-events"),
    ]);
    setSummary(security);
    setEvents(activity.items);
  }

  useEffect(() => {
    void load().catch((err: unknown) => setLoadError(errorText(err, "Could not load security settings.")));
  }, []);

  async function run(key: string, action: () => Promise<void>) {
    setPending(key);
    try {
      await action();
    } finally {
      setPending(null);
    }
  }

  function elevate(event: React.FormEvent) {
    event.preventDefault();
    setStepStatus(null);
    void run("step", async () => {
      try {
        await api("/api/v1/auth/step-up", {
          method: "POST",
          body: JSON.stringify({ password: stepPassword, code: stepCode || undefined, method: "totp" }),
        });
        setStepStatus({ tone: "success", text: "Confirmed. You can continue with the sensitive change." });
        setStepPassword("");
        setStepCode("");
      } catch (err) {
        setStepStatus({ tone: "error", text: errorText(err, "Could not confirm it’s you.") });
      }
    });
  }

  function changePassword(event: React.FormEvent) {
    event.preventDefault();
    setPasswordStatus(null);
    void run("password", async () => {
      try {
        await api("/api/v1/auth/password/change", {
          method: "POST",
          body: JSON.stringify({ current_password: password, new_password: nextPassword }),
        });
        setPasswordStatus({ tone: "success", text: "Password changed. Other sessions were signed out." });
        setPassword("");
        setNextPassword("");
        await load();
      } catch (err) {
        setPasswordStatus({ tone: "error", text: errorText(err, "Could not change the password.") });
      }
    });
  }

  function beginTotp() {
    setMfaStatus(null);
    void run("totp-begin", async () => {
      try {
        setSetup(await api<{ secret: string; qr_svg: string }>("/api/v1/me/mfa/totp/setup", { method: "POST" }));
        setSetupCode("");
      } catch (err) {
        setMfaStatus({ tone: "error", text: errorText(err, "Could not start authenticator setup.") });
      }
    });
  }

  function confirmTotp(event: React.FormEvent) {
    event.preventDefault();
    setMfaStatus(null);
    void run("totp-confirm", async () => {
      try {
        const body = await api<{ recovery_codes: string[] }>("/api/v1/me/mfa/totp/confirm", {
          method: "POST",
          body: JSON.stringify({ code: setupCode }),
        });
        setRecovery(body.recovery_codes);
        setSetup(null);
        setMfaStatus({ tone: "success", text: "Authenticator enabled." });
        await load();
      } catch (err) {
        setMfaStatus({ tone: "error", text: errorText(err, "That code is not valid.") });
      }
    });
  }

  function disableTotp(event: React.FormEvent) {
    event.preventDefault();
    setMfaStatus(null);
    void run("totp-disable", async () => {
      try {
        await api("/api/v1/me/mfa/totp/disable", {
          method: "POST",
          body: JSON.stringify({ code: disableCode, method: "totp" }),
        });
        setDisableCode("");
        setDisabling(false);
        setMfaStatus({ tone: "success", text: "Authenticator disabled." });
        await load();
      } catch (err) {
        setMfaStatus({ tone: "error", text: errorText(err, "Could not disable the authenticator.") });
      }
    });
  }

  function toggleEmail(enabled: boolean) {
    setMfaStatus(null);
    void run("email", async () => {
      try {
        await api("/api/v1/me/mfa/email", { method: "POST", body: JSON.stringify({ enabled }) });
        await load();
      } catch (err) {
        setMfaStatus({ tone: "error", text: errorText(err, "Could not update email codes.") });
      }
    });
  }

  async function regenerate() {
    setMfaStatus(null);
    try {
      const body = await api<{ recovery_codes: string[] }>("/api/v1/me/mfa/recovery-codes", { method: "POST" });
      setRecovery(body.recovery_codes);
      await load();
    } catch (err) {
      setMfaStatus({ tone: "error", text: errorText(err, "Could not regenerate recovery codes.") });
    } finally {
      setRegenOpen(false);
    }
  }

  if (!summary && !loadError) return <PageSkeleton label="Loading security…" />;

  const remaining = summary?.recovery_codes_remaining ?? 0;

  return (
    <div className="grid gap-6">
      <PageTitle title="Security" detail="Password, two-factor authentication, and recent activity." />
      {loadError ? <Alert>{loadError}</Alert> : null}

      <Card>
        <form onSubmit={elevate}>
          <CardHeader
            icon={<ShieldCheckIcon />}
            title="Confirm it’s you"
            description="Sensitive changes ask for your password again, and an authenticator code when one is enabled."
          />
          <CardBody>
            <StatusLine status={stepStatus} />
            <div className="grid gap-4 sm:grid-cols-2">
              <TextField
                label="Password"
                name="step-password"
                type="password"
                autoComplete="current-password"
                value={stepPassword}
                onChange={setStepPassword}
              />
              <TextField
                label="Authenticator code"
                name="step-code"
                autoComplete="one-time-code"
                inputMode="numeric"
                placeholder={summary?.totp_enabled ? "000000" : "Not required"}
                disabled={!summary?.totp_enabled}
                mono={summary?.totp_enabled}
                value={stepCode}
                onChange={setStepCode}
              />
            </div>
          </CardBody>
          <CardFooter note="Confirmation lasts a few minutes.">
            <Button type="submit" variant="secondary" pending={pending === "step"} disabled={!stepPassword}>
              {pending === "step" ? "Confirming…" : "Confirm"}
            </Button>
          </CardFooter>
        </form>
      </Card>

      <Card>
        <form onSubmit={changePassword}>
          <CardHeader
            icon={<KeyIcon />}
            title="Password"
            description={<>Last changed {formatWhen(summary?.password_changed_at)}</>}
          />
          <CardBody>
            <StatusLine status={passwordStatus} />
            <div className="grid gap-4 sm:grid-cols-2">
              <TextField
                label="Current password"
                name="current"
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={setPassword}
              />
              <div className="grid content-start gap-2">
                <TextField
                  label="New password"
                  name="next"
                  type="password"
                  autoComplete="new-password"
                  value={nextPassword}
                  onChange={setNextPassword}
                />
                {nextPassword ? <PasswordStrength value={nextPassword} /> : null}
              </div>
            </div>
          </CardBody>
          <CardFooter note="Other sessions are signed out after a change.">
            <Button type="submit" pending={pending === "password"} disabled={!password || !nextPassword}>
              {pending === "password" ? "Changing…" : "Change password"}
            </Button>
          </CardFooter>
        </form>
      </Card>

      <Card>
        <CardHeader
          icon={<LockIcon />}
          title="Two-factor authentication"
          description="Add a second step when you sign in, so a password alone is not enough."
        />
        <div className="mt-5 border-t border-line">
          {mfaStatus ? (
            <div className="px-5 pt-4 sm:px-6">
              <StatusLine status={mfaStatus} />
            </div>
          ) : null}
          <div className="divide-y divide-line">
            <Row
              icon={<SmartphoneCodeIcon />}
              title={
                <>
                  Authenticator app
                  {summary?.totp_enabled ? (
                    <Badge tone="success" dot>
                      Enabled
                    </Badge>
                  ) : (
                    <Badge tone="outline">Disabled</Badge>
                  )}
                </>
              }
              description="Codes from an app like 1Password, Google Authenticator, or Authy."
              aside={
                summary?.totp_enabled ? (
                  disabling ? null : (
                    <Button type="button" variant="danger" size="sm" onClick={() => setDisabling(true)}>
                      Disable
                    </Button>
                  )
                ) : setup ? null : (
                  <Button type="button" variant="secondary" size="sm" pending={pending === "totp-begin"} onClick={beginTotp}>
                    Enable
                  </Button>
                )
              }
            >
              {setup ? (
                <form className="mt-4 animate-fade-up rounded-[8px] border border-line bg-paper/70 p-4" onSubmit={confirmTotp}>
                  <div className="grid gap-5 sm:grid-cols-[auto_1fr]">
                    <div className="qr-frame w-fit rounded-[8px] border border-line bg-white p-2.5" dangerouslySetInnerHTML={{ __html: setup.qr_svg }} />
                    <div className="grid content-start gap-3 text-sm">
                      <ol className="grid gap-1.5 text-ink-soft">
                        {["Open your authenticator app.", "Scan this QR code.", "Enter the 6-digit code."].map((step, index) => (
                          <li key={step} className="flex gap-2.5">
                            <span className="tabular flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-line-strong bg-white text-[11px] font-medium text-muted">
                              {index + 1}
                            </span>
                            {step}
                          </li>
                        ))}
                      </ol>
                      <div>
                        <p className="text-[13px] text-muted">Can’t scan? Enter this setup key manually:</p>
                        <div className="mt-1.5 flex flex-wrap items-center gap-2">
                          <code className="break-all rounded-[6px] border border-line bg-white px-2 py-1 font-mono text-[13px] tracking-wide text-ink">
                            {setup.secret}
                          </code>
                          <CopyButton value={setup.secret} />
                        </div>
                      </div>
                    </div>
                  </div>
                  <div className="mt-5 grid gap-3 sm:grid-cols-[1fr_auto] sm:items-end">
                    <CodeField label="6-digit code" name="totp" value={setupCode} onChange={setSetupCode} />
                    <div className="flex gap-2">
                      <Button type="button" variant="ghost" size="lg" onClick={() => setSetup(null)}>
                        Cancel
                      </Button>
                      <Button type="submit" size="lg" pending={pending === "totp-confirm"} disabled={setupCode.length < 6}>
                        Enable authenticator
                      </Button>
                    </div>
                  </div>
                </form>
              ) : null}
              {summary?.totp_enabled && disabling ? (
                <form className="mt-4 grid animate-fade-up gap-3 rounded-[8px] border border-danger-line bg-danger-soft/40 p-4 sm:grid-cols-[1fr_auto] sm:items-end" onSubmit={disableTotp}>
                  <CodeField label="Current authenticator code" name="disable" autoFocus value={disableCode} onChange={setDisableCode} />
                  <div className="flex gap-2">
                    <Button type="button" variant="ghost" size="lg" onClick={() => setDisabling(false)}>
                      Cancel
                    </Button>
                    <Button type="submit" variant="danger-solid" size="lg" pending={pending === "totp-disable"}>
                      Disable authenticator
                    </Button>
                  </div>
                </form>
              ) : null}
            </Row>
            <Row
              icon={<MailIcon />}
              title="Email verification code"
              description={summary?.email_enabled ? "A code is emailed to you at sign-in." : "Available. Receive a code by email at sign-in."}
              aside={
                <Switch
                  label="Email verification code"
                  checked={Boolean(summary?.email_enabled)}
                  disabled={pending === "email"}
                  onCheckedChange={toggleEmail}
                />
              }
            />
            <Row
              icon={<KeyIcon />}
              title={
                <>
                  Recovery codes
                  {summary?.totp_enabled ? (
                    <Badge tone={remaining === 0 ? "danger" : remaining < 3 ? "warning" : "neutral"}>{remaining} remaining</Badge>
                  ) : null}
                </>
              }
              description="One-time codes for when you can’t use your authenticator."
              aside={
                <Button type="button" variant="secondary" size="sm" onClick={() => setRegenOpen(true)}>
                  Regenerate
                </Button>
              }
            />
          </div>
          {recovery ? <RecoveryCodes codes={recovery} onDone={() => setRecovery(null)} /> : null}
        </div>
      </Card>

      <Card>
        <CardHeader title="Recent security activity" description="Sign-ins and changes to your account." />
        <div className="mt-4 border-t border-line">
          <ActivityList events={events} empty="Sign-ins and security changes will appear here." />
        </div>
      </Card>

      <ConfirmDialog
        open={regenOpen}
        onOpenChange={setRegenOpen}
        title="Regenerate recovery codes?"
        description="Your current recovery codes will stop working. Save the new codes somewhere safe."
        confirmLabel="Regenerate codes"
        pendingLabel="Generating…"
        onConfirm={regenerate}
      />
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Sessions                                                            */
/* ------------------------------------------------------------------ */

export function SessionsPage() {
  const [items, setItems] = useState<SessionItem[] | null>(null);
  const [error, setError] = useState("");
  const [revoking, setRevoking] = useState<string | null>(null);
  const [othersOpen, setOthersOpen] = useState(false);

  async function load() {
    const body = await api<{ items: SessionItem[] }>("/api/v1/me/sessions");
    setItems([...body.items].sort((a, b) => Number(b.current) - Number(a.current)));
  }

  useEffect(() => {
    void load().catch((err: unknown) => setError(errorText(err, "Could not load sessions.")));
  }, []);

  async function revoke(id: string) {
    setRevoking(id);
    setError("");
    try {
      await api(`/api/v1/me/sessions/${id}`, { method: "DELETE" });
      if (items?.find((item) => item.id === id)?.current) {
        window.location.assign("/sign-in");
        return;
      }
      await load();
    } catch (err) {
      setError(errorText(err, "Could not revoke the session."));
    } finally {
      setRevoking(null);
    }
  }

  async function revokeOthers() {
    try {
      await api("/api/v1/me/sessions/revoke-others", { method: "POST" });
      await load();
    } catch (err) {
      setError(errorText(err, "Could not revoke sessions."));
    } finally {
      setOthersOpen(false);
    }
  }

  const others = items?.filter((item) => !item.current).length ?? 0;

  return (
    <div className="grid gap-6">
      <PageTitle
        title="Sessions"
        detail="Devices currently signed in to your Knotree account."
        actions={
          <Button type="button" variant="secondary" disabled={!items || others === 0} onClick={() => setOthersOpen(true)}>
            Sign out other sessions
          </Button>
        }
      />
      {error ? <Alert>{error}</Alert> : null}
      <Card>
        {!items ? (
          <div className="divide-y divide-line">
            {[0, 1, 2].map((key) => (
              <div key={key} className="flex items-center gap-3 px-5 py-4 sm:px-6">
                <Skeleton className="h-9 w-9" />
                <div className="grid flex-1 gap-1.5">
                  <Skeleton className="h-3.5 w-40" />
                  <Skeleton className="h-3 w-56" />
                </div>
              </div>
            ))}
          </div>
        ) : items.length === 0 ? (
          <EmptyState icon={<DevicesIcon />} title="No active sessions">
            Devices you sign in on will appear here.
          </EmptyState>
        ) : (
          <ul className="divide-y divide-line">
            {items.map((item) => (
              <li key={item.id} className="flex flex-col gap-3 px-5 py-4 sm:flex-row sm:items-center sm:px-6">
                <span
                  className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-[8px] border ${
                    item.current ? "border-pine-line bg-pine-soft text-pine" : "border-line bg-paper text-ink-soft"
                  }`}
                >
                  {isMobileDevice(item.device) ? <PhoneIcon /> : <MonitorIcon />}
                </span>
                <div className="min-w-0 flex-1">
                  <p className="flex flex-wrap items-center gap-2 text-sm font-medium text-ink">
                    {item.device}
                    {item.current ? (
                      <Badge tone="success" dot>
                        This device
                      </Badge>
                    ) : null}
                    {item.mfa ? <Badge tone="outline">2FA</Badge> : null}
                  </p>
                  <p className="mt-0.5 text-[13px] text-muted">
                    <span className="font-mono text-[12.5px]">{item.ip ?? "IP unavailable"}</span>
                    <span className="mx-1.5 text-faint">·</span>
                    <span title={formatWhen(item.last_active_at)}>Active {formatRelative(item.last_active_at)}</span>
                    <span className="mx-1.5 hidden text-faint sm:inline">·</span>
                    <span className="hidden sm:inline">Signed in {formatDate(item.created_at)}</span>
                  </p>
                </div>
                <Button
                  type="button"
                  variant={item.current ? "secondary" : "danger"}
                  size="sm"
                  className="self-start sm:self-auto"
                  pending={revoking === item.id}
                  onClick={() => void revoke(item.id)}
                >
                  {item.current ? "Sign out" : "Revoke"}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Card>
      <ConfirmDialog
        open={othersOpen}
        onOpenChange={setOthersOpen}
        title="Sign out other sessions?"
        description={`${others} other ${others === 1 ? "device" : "devices"} will be signed out. This device stays signed in.`}
        confirmLabel="Sign out others"
        pendingLabel="Signing out…"
        onConfirm={revokeOthers}
      />
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Connected accounts                                                  */
/* ------------------------------------------------------------------ */

const providers = [
  { id: "google", logo: <GoogleLogo size={18} /> },
  { id: "github", logo: <GitHubLogo size={18} /> },
];

export function ConnectedAccountsPage() {
  const { profile, error, reload } = useProfile();
  const [formError, setFormError] = useState("");
  const [pending, setPending] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<string | null>(null);
  if (error) return <Alert>{error}</Alert>;
  if (!profile) return <PageSkeleton label="Loading connected accounts…" />;

  async function unlink(provider: string) {
    setPending(provider);
    setFormError("");
    try {
      await api(`/api/v1/me/identities/${provider}`, { method: "DELETE" });
      await reload();
    } catch (err) {
      setFormError(errorText(err, "Could not disconnect that account."));
    } finally {
      setPending(null);
      setConfirm(null);
    }
  }

  const hasPassword = profile.identities.some((identity) => identity.provider === "password");
  const extra = profile.identities.filter(
    (identity) => identity.provider !== "password" && !providers.some((p) => p.id === identity.provider),
  );

  return (
    <div className="grid gap-6">
      <PageTitle
        title="Connected accounts"
        detail="Google and GitHub can sign in to the same Knotree account. Matching emails are not linked automatically."
      />
      {formError ? <Alert>{formError}</Alert> : null}
      <Card>
        <ul className="divide-y divide-line">
          {hasPassword ? (
            <li>
              <Row
                icon={<KeyIcon />}
                title={
                  <>
                    {providerLabel("password")}
                    <Badge tone="success" dot>
                      Connected
                    </Badge>
                  </>
                }
                description={profile.identities.find((i) => i.provider === "password")?.email ?? profile.email}
                aside={
                  <Link to="/account/security" className={buttonClass("ghost", "sm")}>
                    Manage
                  </Link>
                }
              />
            </li>
          ) : null}
          {providers.map((provider) => {
            const identity = profile.identities.find((item) => item.provider === provider.id);
            const href = `/api/v1/auth/social/${provider.id}/start?mode=link&return_to=%2Faccount%2Fconnected-accounts`;
            return (
              <li key={provider.id}>
                <Row
                  icon={provider.logo}
                  title={
                    <>
                      {providerLabel(provider.id)}
                      {identity ? (
                        <Badge tone="success" dot>
                          Connected
                        </Badge>
                      ) : null}
                    </>
                  }
                  description={identity ? identity.email ?? "Connected" : `Sign in with your ${providerLabel(provider.id)} account.`}
                  aside={
                    identity ? (
                      <Button
                        type="button"
                        variant="secondary"
                        size="sm"
                        pending={pending === provider.id}
                        onClick={() => setConfirm(provider.id)}
                      >
                        Disconnect
                      </Button>
                    ) : (
                      <a className={buttonClass("secondary", "sm")} href={href}>
                        Connect {providerLabel(provider.id)}
                      </a>
                    )
                  }
                />
              </li>
            );
          })}
          {extra.map((identity) => (
            <li key={identity.provider}>
              <Row
                icon={<LinkIcon />}
                title={providerLabel(identity.provider)}
                description={identity.email ?? "Connected"}
                aside={
                  <Button type="button" variant="secondary" size="sm" onClick={() => setConfirm(identity.provider)}>
                    Disconnect
                  </Button>
                }
              />
            </li>
          ))}
        </ul>
      </Card>
      <p className="text-[13px] text-muted">
        Keep at least one way to sign in. Disconnecting does not delete the account on the other service.
      </p>
      <ConfirmDialog
        open={confirm !== null}
        onOpenChange={(open) => {
          if (!open) setConfirm(null);
        }}
        title={`Disconnect ${providerLabel(confirm ?? "")}?`}
        description="You will no longer be able to sign in to Knotree with this account. You can connect it again later."
        confirmLabel="Disconnect"
        pendingLabel="Disconnecting…"
        danger
        onConfirm={() => unlink(confirm ?? "")}
      />
    </div>
  );
}
