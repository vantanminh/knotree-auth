import { t } from "../lib/i18n";
import { useEffect, useState, type ReactNode } from "react";
import { Link } from "react-router";
import { LanguageSwitcher, useProfile } from "../components/shells";
import {
  AlertIcon,
  AppIcon,
  CheckCircleIcon,
  ChevronRightIcon,
  DevicesIcon,
  DownloadIcon,
  GlobeIcon,
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
  ClientLogo,
} from "../components/ui";
import { ApiError, api } from "../lib/api";
import { eventLabel, formatDate, formatRelative, formatWhen, isMobileDevice, providerLabel } from "../lib/format";
import type { AccountEmail, AuthorizationItem, MfaSummary, Profile, SecurityEvent, SessionItem } from "../lib/types";

type Status = { tone: "success" | "error"; text: string } | null;

function errorText(err: unknown, fallback: string) {
  return err instanceof ApiError ? err.message : t(fallback);
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
  if (!profile) return <PageSkeleton label={t("Loading account…")} />;

  const twoFactor = profile.mfa.totp_enabled || profile.mfa.email_enabled;
  const recoveryReady = !profile.mfa.totp_enabled || profile.mfa.recovery_codes_remaining > 0;
  const checks = [profile.email_verified, twoFactor, recoveryReady];
  const done = checks.filter(Boolean).length;

  return (
    <div className="grid gap-6">
      <PageTitle title={profile.display_name || t("Account")} detail={t("Manage your Knotree identity, security, and signed-in devices.")} />

      <Card>
        <div className="flex flex-col gap-4 px-5 py-5 sm:flex-row sm:items-center sm:px-6">
          <Avatar name={profile.display_name || profile.email} size={52} />
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <p className="truncate text-[15px] font-medium text-ink">{profile.email}</p>
              {profile.email_verified ? (
                <Badge tone="success" dot>
                  {t("Verified")}
                </Badge>
              ) : (
                <Badge tone="warning" dot>
                  {t("Unverified")}
                </Badge>
              )}
              {profile.is_admin ? <Badge tone="outline">{t("Administrator")}</Badge> : null}
            </div>
            <p className="mt-0.5 text-[13px] text-muted">{t("Member since")}{' '}{formatDate(profile.created_at)}</p>
          </div>
          <Link to="/account/profile" className={buttonClass("secondary", "md", "self-start sm:self-auto")}>
            {t("Edit profile")}
          </Link>
        </div>
      </Card>

      <Card>
        <CardHeader
          icon={<ShieldCheckIcon />}
          title={t("Security checkup")}
          description={
            done === checks.length
              ? t("Your account follows every recommendation.")
              : t("{done} of {total} recommendations complete.", { done, total: checks.length })
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
            title={t("Email address")}
            detail={profile.email_verified ? t("Verified and ready for account recovery.") : t("Verify your email to recover your account.")}
            to="/account/profile"
          />
          <CheckItem
            done={twoFactor}
            title={t("Two-factor authentication")}
            detail={
              profile.mfa.totp_enabled
                ? t("Authenticator app is on.")
                : profile.mfa.email_enabled
                  ? t("Email codes are on. An authenticator app is stronger.")
                  : t("Add a second step when you sign in.")
            }
            to="/account/security"
          />
          <CheckItem
            done={recoveryReady}
            title={t("Recovery codes")}
            detail={
              profile.mfa.totp_enabled
                ? t("{count} unused codes available.", { count: profile.mfa.recovery_codes_remaining })
                : t("Generated when you turn on an authenticator app.")
            }
            to="/account/security"
          />
        </div>
      </Card>

      <div className="grid gap-3 sm:grid-cols-2">
        <Shortcut to="/account/profile" icon={<UserIcon />} title={t("Profile")} detail={t("Name and email address.")} />
        <Shortcut to="/account/security" icon={<LockIcon />} title={t("Security")} detail={t("Password, two-factor, and activity.")} />
        <Shortcut to="/account/sessions" icon={<DevicesIcon />} title={t("Sessions")} detail={t("Devices signed in to your account.")} />
        <Shortcut
          to="/account/connected-accounts"
          icon={<LinkIcon />}
          title={t("Connected accounts")}
          detail={t("Sign in with Google or GitHub.")}
        />
        <Shortcut
          to="/account/authorized-apps"
          icon={<AppIcon />}
          title={t("Authorized apps")}
          detail={t("Services that can use your Knotree account.")}
        />
      </div>

      {profile.is_admin ? (
        <Link
          to="/admin"
          className="group flex items-center gap-3 rounded-[var(--radius-card)] border border-dashed border-line-strong px-4 py-3.5 text-sm transition-colors hover:border-pine-line hover:bg-pine-soft/40"
        >
          <AppIcon className="text-pine" />
          <span className="flex-1">
            <span className="font-medium text-ink">{t("Open administration")}</span>
            <span className="text-muted">{' '}{t("· Users, security events, and email logs")}</span>
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
  const [nameStatus, setNameStatus] = useState<Status>(null);
  const [pending, setPending] = useState<"name" | null>(null);

  useEffect(() => {
    if (profile) setName(profile.display_name ?? "");
  }, [profile]);

  if (error) return <Alert>{error}</Alert>;
  if (!profile) return <PageSkeleton label={t("Loading profile…")} />;

  async function saveName(event: React.FormEvent) {
    event.preventDefault();
    setPending("name");
    setNameStatus(null);
    try {
      await api("/api/v1/me/profile", { method: "PATCH", body: JSON.stringify({ display_name: name }) });
      await reload();
      setNameStatus({ tone: "success", text: t("Name updated.") });
    } catch (err) {
      setNameStatus({ tone: "error", text: errorText(err, "Could not update your name.") });
    } finally {
      setPending(null);
    }
  }

  const nameUnchanged = name.trim() === (profile.display_name ?? "");

  return (
    <div className="grid gap-6">
      <PageTitle title={t("Profile")} detail={t("Name, username and emails for your Knotree account.")} />

      <Card>
        <form onSubmit={saveName}>
          <CardHeader title={t("Display name")} description={t("Shown to Knotree services you sign in to.")} />
          <CardBody>
            <StatusLine status={nameStatus} />
            <div className="flex items-end gap-4">
              <Avatar name={name || profile.email} size={40} />
              <TextField
                className="flex-1"
                label={t("Name")}
                name="name"
                autoComplete="name"
                placeholder={t("Your name")}
                value={name}
                onChange={setName}
                required
              />
            </div>
          </CardBody>
          <CardFooter note={t("Use the name people know you by.")}>
            <Button type="submit" pending={pending === "name"} disabled={nameUnchanged || !name.trim()}>
              {pending === "name" ? t("Saving…") : t("Save name")}
            </Button>
          </CardFooter>
        </form>
      </Card>

      <UsernameCard profile={profile} reload={reload} />

      <EmailsCard emails={profile.emails} reload={reload} />

      <LanguageCard />

      <DeleteAccount />
    </div>
  );
}

const MAX_EMAILS = 3;

function UsernameCard({ profile, reload }: { profile: Profile; reload: () => Promise<unknown> }) {
  const [username, setUsername] = useState(profile.username);
  const [status, setStatus] = useState<Status>(null);
  const [pending, setPending] = useState(false);
  const unchanged = username.trim().toLowerCase() === profile.username;

  async function save(event: React.FormEvent) {
    event.preventDefault();
    setPending(true);
    setStatus(null);
    try {
      await api("/api/v1/me/username", { method: "PATCH", body: JSON.stringify({ username }) });
      await reload();
      setStatus({ tone: "success", text: t("Username updated.") });
    } catch (err) {
      setStatus({ tone: "error", text: errorText(err, "Could not update your username.") });
    } finally {
      setPending(false);
    }
  }

  return (
    <Card>
      <form onSubmit={save}>
        <CardHeader title={t("Username")} description={t("Sign in with your username or any verified email.")} />
        <CardBody>
          <StatusLine status={status} />
          <TextField
            label={t("Username")}
            name="username"
            autoComplete="username"
            value={username}
            onChange={setUsername}
            required
          />
        </CardBody>
        <CardFooter note={t("Requires a recent sign-in. You can change it once every 30 days.")}>
          <Button type="submit" pending={pending} disabled={unchanged || !username.trim()}>
            {pending ? t("Saving…") : t("Save username")}
          </Button>
        </CardFooter>
      </form>
    </Card>
  );
}

function EmailsCard({ emails, reload }: { emails: AccountEmail[]; reload: () => Promise<unknown> }) {
  const [email, setEmail] = useState("");
  const [status, setStatus] = useState<Status>(null);
  const [pending, setPending] = useState<string | null>(null);
  const full = emails.length >= MAX_EMAILS;

  async function run(key: string, action: () => Promise<void>, fallback: string) {
    setPending(key);
    setStatus(null);
    try {
      await action();
    } catch (err) {
      setStatus({ tone: "error", text: errorText(err, fallback) });
    } finally {
      setPending(null);
    }
  }

  function add(event: React.FormEvent) {
    event.preventDefault();
    void run(
      "add",
      async () => {
        await api("/api/v1/me/emails", { method: "POST", body: JSON.stringify({ email }) });
        setEmail("");
        await reload();
        setStatus({ tone: "success", text: t("Check the new address for a confirmation link.") });
      },
      "Could not add the email.",
    );
  }

  function makePrimary(item: AccountEmail) {
    void run(
      `primary-${item.id}`,
      async () => {
        await api(`/api/v1/me/emails/${item.id}/primary`, { method: "POST", body: "{}" });
        await reload();
        setStatus({ tone: "success", text: t("Primary email updated.") });
      },
      "Could not change the primary email.",
    );
  }

  function resend(item: AccountEmail) {
    void run(
      `resend-${item.id}`,
      async () => {
        await api(`/api/v1/me/emails/${item.id}/resend`, { method: "POST", body: "{}" });
        setStatus({ tone: "success", text: t("Check the new address for a confirmation link.") });
      },
      "Could not resend the email.",
    );
  }

  function remove(item: AccountEmail) {
    void run(
      `remove-${item.id}`,
      async () => {
        await api(`/api/v1/me/emails/${item.id}`, { method: "DELETE" });
        await reload();
        setStatus({ tone: "success", text: t("Email removed.") });
      },
      "Could not remove the email.",
    );
  }

  return (
    <Card>
      <CardHeader
        title={t("Email addresses")}
        description={t("Up to three. Any verified email can sign in; the primary one receives security alerts.")}
      />
      <CardBody className="grid gap-3">
        <StatusLine status={status} />
        <ul className="grid gap-2" aria-label={t("Email addresses")}>
          {emails.map((item) => (
            <li
              key={item.id}
              className="flex flex-wrap items-center gap-3 rounded-[8px] border border-line bg-paper/70 px-3.5 py-3"
            >
              <MailIcon className="text-faint" />
              <span className="min-w-0 flex-1 truncate text-sm text-ink">{item.email}</span>
              {item.primary ? <Badge>{t("Primary")}</Badge> : null}
              {item.verified ? (
                <Badge tone="success" dot>
                  {t("Verified")}
                </Badge>
              ) : (
                <Badge tone="warning" dot>
                  {t("Unverified")}
                </Badge>
              )}
              {!item.primary && item.verified ? (
                <Button
                  size="sm"
                  variant="secondary"
                  pending={pending === `primary-${item.id}`}
                  onClick={() => makePrimary(item)}
                >
                  {t("Make primary")}
                </Button>
              ) : null}
              {!item.primary && !item.verified ? (
                <Button size="sm" variant="ghost" pending={pending === `resend-${item.id}`} onClick={() => resend(item)}>
                  {t("Resend")}
                </Button>
              ) : null}
              {!item.primary ? (
                <Button
                  size="sm"
                  variant="ghost"
                  aria-label={t("Remove {email}", { email: item.email })}
                  pending={pending === `remove-${item.id}`}
                  onClick={() => remove(item)}
                >
                  <TrashIcon />
                </Button>
              ) : null}
            </li>
          ))}
        </ul>
        {full ? null : (
          <form className="grid gap-3" onSubmit={add}>
            <TextField
              label={t("Add email")}
              name="email"
              type="email"
              autoComplete="email"
              placeholder={t("new@example.com")}
              value={email}
              onChange={setEmail}
            />
            <div>
              <Button type="submit" variant="secondary" pending={pending === "add"} disabled={!email}>
                {pending === "add" ? t("Sending…") : t("Add email")}
              </Button>
            </div>
          </form>
        )}
      </CardBody>
      <CardFooter note={t("Changes need a recent sign-in. Your primary email is alerted about every change.")} />
    </Card>
  );
}

function LanguageCard() {
  return (
    <Card>
      <CardHeader
        icon={<GlobeIcon />}
        title={t("Language")}
        description={t("Used for this site, emails, and security notifications.")}
        action={<LanguageSwitcher signedIn />}
      />
      <div className="h-5" />
    </Card>
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
        title={t("Delete account")}
        description={t("Permanently remove your Knotree account. You will be signed out of every Knotree service.")}
      />
      {error ? (
        <CardBody className="pt-4">
          <Alert>{error}</Alert>
        </CardBody>
      ) : (
        <div className="h-5" />
      )}
      <CardFooter note={t("This cannot be undone from the product.")}>
        <Button type="button" variant="danger" icon={<TrashIcon size={15} />} onClick={() => setOpen(true)}>
          {t("Delete account")}
        </Button>
      </CardFooter>
      <ConfirmDialog
        open={open}
        onOpenChange={setOpen}
        danger
        title={t("Delete this Knotree account?")}
        description={t("Sessions will end and applications using Knotree sign-in will lose access. This cannot be undone from the product.")}
        confirmLabel={t("Delete account")}
        pendingLabel={t("Deleting…")}
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
    [`${t("Knotree recovery codes")}\n${t("Generated {date}", { date: new Date().toISOString() })}\n\n${t("Each code works once.")}\n\n${codes.join("\n")}\n`],
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
      <p className="text-sm font-medium text-ink">{t("Save your recovery codes")}</p>
      <p className="mt-0.5 text-[13px] text-muted">{t("Each code works once. This is the only time they are shown.")}</p>
      <ul className="mt-4 grid grid-cols-2 gap-x-4 gap-y-1.5 rounded-[6px] border border-line bg-white px-4 py-3 font-mono text-[14px] text-ink sm:grid-cols-2">
        {codes.map((item) => (
          <li key={item} className="tabular">
            {item}
          </li>
        ))}
      </ul>
      <div className="mt-3 flex flex-wrap gap-2">
        <CopyButton value={codes.join("\n")} label={t("Copy all")} />
        <Button type="button" variant="secondary" size="sm" icon={<DownloadIcon size={14} />} onClick={() => downloadCodes(codes)}>
          {t("Download")}
        </Button>
      </div>
      <div className="mt-4 flex flex-col gap-3 border-t border-amber-line/70 pt-4 sm:flex-row sm:items-center sm:justify-between">
        <Checkbox checked={saved} onCheckedChange={setSaved}>
          {t("I have saved these codes")}
        </Checkbox>
        <Button type="button" disabled={!saved} onClick={onDone}>
          {t("Continue")}
        </Button>
      </div>
    </div>
  );
}

export function ActivityList({ events, empty }: { events: SecurityEvent[]; empty: string }) {
  if (events.length === 0) return <EmptyState icon={<ShieldIcon />} title={t("No activity yet")}>{empty}</EmptyState>;
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
        setStepStatus({ tone: "success", text: t("Confirmed. You can continue with the sensitive change.") });
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
        setPasswordStatus({ tone: "success", text: t("Password changed. Other sessions were signed out.") });
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
        setMfaStatus({ tone: "success", text: t("Authenticator enabled.") });
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
        setMfaStatus({ tone: "success", text: t("Authenticator disabled.") });
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

  if (!summary && !loadError) return <PageSkeleton label={t("Loading security…")} />;

  const remaining = summary?.recovery_codes_remaining ?? 0;

  return (
    <div className="grid gap-6">
      <PageTitle title={t("Security")} detail={t("Password, two-factor authentication, and recent activity.")} />
      {loadError ? <Alert>{loadError}</Alert> : null}

      <Card>
        <form onSubmit={elevate}>
          <CardHeader
            icon={<ShieldCheckIcon />}
            title={t("Confirm it’s you")}
            description={t("Sensitive changes ask for your password again, and an authenticator code when one is enabled.")}
          />
          <CardBody>
            <StatusLine status={stepStatus} />
            <div className="grid gap-4 sm:grid-cols-2">
              <TextField
                label={t("Password")}
                name="step-password"
                type="password"
                autoComplete="current-password"
                value={stepPassword}
                onChange={setStepPassword}
              />
              <TextField
                label={t("Authenticator code")}
                name="step-code"
                autoComplete="one-time-code"
                inputMode="numeric"
                placeholder={summary?.totp_enabled ? "000000" : t("Not required")}
                disabled={!summary?.totp_enabled}
                mono={summary?.totp_enabled}
                value={stepCode}
                onChange={setStepCode}
              />
            </div>
          </CardBody>
          <CardFooter note={t("Confirmation lasts a few minutes.")}>
            <Button type="submit" variant="secondary" pending={pending === "step"} disabled={!stepPassword}>
              {pending === "step" ? t("Confirming…") : t("Confirm")}
            </Button>
          </CardFooter>
        </form>
      </Card>

      <Card>
        <form onSubmit={changePassword}>
          <CardHeader
            icon={<KeyIcon />}
            title={t("Password")}
            description={<>{t("Last changed")}{' '}{formatWhen(summary?.password_changed_at)}</>}
          />
          <CardBody>
            <StatusLine status={passwordStatus} />
            <div className="grid gap-4 sm:grid-cols-2">
              <TextField
                label={t("Current password")}
                name="current"
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={setPassword}
              />
              <div className="grid content-start gap-2">
                <TextField
                  label={t("New password")}
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
          <CardFooter note={t("Other sessions are signed out after a change.")}>
            <Button type="submit" pending={pending === "password"} disabled={!password || !nextPassword}>
              {pending === "password" ? t("Changing…") : t("Change password")}
            </Button>
          </CardFooter>
        </form>
      </Card>

      <Card>
        <CardHeader
          icon={<LockIcon />}
          title={t("Two-factor authentication")}
          description={t("Add a second step when you sign in, so a password alone is not enough.")}
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
                  {t("Authenticator app")}
                  {summary?.totp_enabled ? (
                    <Badge tone="success" dot>
                      {t("Enabled")}
                    </Badge>
                  ) : (
                    <Badge tone="outline">{t("Disabled")}</Badge>
                  )}
                </>
              }
              description={t("Codes from an app like 1Password, Google Authenticator, or Authy.")}
              aside={
                summary?.totp_enabled ? (
                  disabling ? null : (
                    <Button type="button" variant="danger" size="sm" onClick={() => setDisabling(true)}>
                      {t("Disable")}
                    </Button>
                  )
                ) : setup ? null : (
                  <Button type="button" variant="secondary" size="sm" pending={pending === "totp-begin"} onClick={beginTotp}>
                    {t("Enable")}
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
                            {t(step)}
                          </li>
                        ))}
                      </ol>
                      <div>
                        <p className="text-[13px] text-muted">{t("Can’t scan? Enter this setup key manually:")}</p>
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
                    <CodeField label={t("6-digit code")} name="totp" value={setupCode} onChange={setSetupCode} />
                    <div className="flex gap-2">
                      <Button type="button" variant="ghost" size="lg" onClick={() => setSetup(null)}>
                        {t("Cancel")}
                      </Button>
                      <Button type="submit" size="lg" pending={pending === "totp-confirm"} disabled={setupCode.length < 6}>
                        {t("Enable authenticator")}
                      </Button>
                    </div>
                  </div>
                </form>
              ) : null}
              {summary?.totp_enabled && disabling ? (
                <form className="mt-4 grid animate-fade-up gap-3 rounded-[8px] border border-danger-line bg-danger-soft/40 p-4 sm:grid-cols-[1fr_auto] sm:items-end" onSubmit={disableTotp}>
                  <CodeField label={t("Current authenticator code")} name="disable" autoFocus value={disableCode} onChange={setDisableCode} />
                  <div className="flex gap-2">
                    <Button type="button" variant="ghost" size="lg" onClick={() => setDisabling(false)}>
                      {t("Cancel")}
                    </Button>
                    <Button type="submit" variant="danger-solid" size="lg" pending={pending === "totp-disable"}>
                      {t("Disable authenticator")}
                    </Button>
                  </div>
                </form>
              ) : null}
            </Row>
            <Row
              icon={<MailIcon />}
              title={t("Email verification code")}
              description={summary?.email_enabled ? t("A code is emailed to you at sign-in.") : t("Available. Receive a code by email at sign-in.")}
              aside={
                <Switch
                  label={t("Email verification code")}
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
                  {t("Recovery codes")}
                  {summary?.totp_enabled ? (
                    <Badge tone={remaining === 0 ? "danger" : remaining < 3 ? "warning" : "neutral"}>{t("{count} remaining", { count: remaining })}</Badge>
                  ) : null}
                </>
              }
              description={t("One-time codes for when you can’t use your authenticator.")}
              aside={
                <Button type="button" variant="secondary" size="sm" onClick={() => setRegenOpen(true)}>
                  {t("Regenerate")}
                </Button>
              }
            />
          </div>
          {recovery ? <RecoveryCodes codes={recovery} onDone={() => setRecovery(null)} /> : null}
        </div>
      </Card>

      <Card>
        <CardHeader title={t("Recent security activity")} description={t("Sign-ins and changes to your account.")} />
        <div className="mt-4 border-t border-line">
          <ActivityList events={events} empty={t("Sign-ins and security changes will appear here.")} />
        </div>
      </Card>

      <ConfirmDialog
        open={regenOpen}
        onOpenChange={setRegenOpen}
        title={t("Regenerate recovery codes?")}
        description={t("Your current recovery codes will stop working. Save the new codes somewhere safe.")}
        confirmLabel={t("Regenerate codes")}
        pendingLabel={t("Generating…")}
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
        title={t("Sessions")}
        detail={t("Devices currently signed in to your Knotree account.")}
        actions={
          <Button type="button" variant="secondary" disabled={!items || others === 0} onClick={() => setOthersOpen(true)}>
            {t("Sign out other sessions")}
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
          <EmptyState icon={<DevicesIcon />} title={t("No active sessions")}>
            {t("Devices you sign in on will appear here.")}
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
                        {t("This device")}
                      </Badge>
                    ) : null}
                    {item.mfa ? <Badge tone="outline">{t("2FA")}</Badge> : null}
                  </p>
                  <p className="mt-0.5 text-[13px] text-muted">
                    <span className="font-mono text-[12.5px]">{item.ip ?? t("IP unavailable")}</span>
                    <span className="mx-1.5 text-faint">·</span>
                    <span title={formatWhen(item.last_active_at)}>{t("Active")}{' '}{formatRelative(item.last_active_at)}</span>
                    <span className="mx-1.5 hidden text-faint sm:inline">·</span>
                    <span className="hidden sm:inline">{t("Signed in")}{' '}{formatDate(item.created_at)}</span>
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
                  {item.current ? t("Sign out") : t("Revoke")}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Card>
      <ConfirmDialog
        open={othersOpen}
        onOpenChange={setOthersOpen}
        title={t("Sign out other sessions?")}
        description={t(others === 1 ? "{count} other device will be signed out. This device stays signed in." : "{count} other devices will be signed out. This device stays signed in.", { count: others })}
        confirmLabel={t("Sign out others")}
        pendingLabel={t("Signing out…")}
        onConfirm={revokeOthers}
      />
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Authorized apps                                                     */
/* ------------------------------------------------------------------ */

export function AuthorizedAppsPage() {
  const [items, setItems] = useState<AuthorizationItem[] | null>(null);
  const [error, setError] = useState("");
  const [confirm, setConfirm] = useState<AuthorizationItem | null>(null);

  async function load() {
    const body = await api<{ items: AuthorizationItem[] }>("/api/v1/me/authorizations");
    setItems(body.items);
  }

  useEffect(() => {
    void load().catch((err: unknown) => setError(errorText(err, "Could not load authorized apps.")));
  }, []);

  async function revoke(item: AuthorizationItem) {
    setError("");
    try {
      await api(`/api/v1/me/authorizations/${encodeURIComponent(item.client_id)}`, { method: "DELETE" });
      await load();
    } catch (err) {
      setError(errorText(err, "Could not revoke access."));
    } finally {
      setConfirm(null);
    }
  }

  return (
    <div className="grid gap-6">
      <PageTitle
        title={t("Authorized apps")}
        detail={t("Services you have signed in to with your Knotree account.")}
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
          <EmptyState icon={<AppIcon />} title={t("No authorized apps")}>
            {t("Services you sign in to with Knotree will appear here.")}
          </EmptyState>
        ) : (
          <ul className="divide-y divide-line">
            {items.map((item) => (
              <li key={item.client_id} className="flex flex-col gap-3 px-5 py-4 sm:flex-row sm:items-center sm:px-6">
                <ClientLogo name={item.name} src={item.logo_url} size={36} />
                <div className="min-w-0 flex-1">
                  <p className="flex flex-wrap items-center gap-2 text-sm font-medium text-ink">
                    {item.name}
                    {item.first_party ? <Badge tone="outline">{t("Knotree service")}</Badge> : null}
                    {item.status === "disabled" ? <Badge tone="outline">{t("Disabled")}</Badge> : null}
                  </p>
                  <p className="mt-0.5 text-[13px] text-muted">
                    <span title={formatWhen(item.granted_at)}>{t("Authorized")}{' '}{formatDate(item.granted_at)}</span>
                    {item.last_used_at ? (
                      <>
                        <span className="mx-1.5 text-faint">·</span>
                        <span title={formatWhen(item.last_used_at)}>{t("Last used")}{' '}{formatRelative(item.last_used_at)}</span>
                      </>
                    ) : null}
                  </p>
                  <p className="mt-1.5 flex flex-wrap gap-1.5">
                    {item.scopes.map((scope) => (
                      <span key={scope} className="rounded-[6px] border border-line bg-paper px-1.5 py-0.5 font-mono text-[11.5px] text-ink-soft">
                        {scope}
                      </span>
                    ))}
                  </p>
                </div>
                <Button
                  type="button"
                  variant="danger"
                  size="sm"
                  className="self-start sm:self-auto"
                  onClick={() => setConfirm(item)}
                >
                  {t("Revoke access")}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Card>
      <ConfirmDialog
        open={confirm !== null}
        onOpenChange={(open) => {
          if (!open) setConfirm(null);
        }}
        title={t("Revoke access for {name}?", { name: confirm?.name ?? "" })}
        description={t("You will be signed out of this service. It can ask for access again the next time you sign in.")}
        confirmLabel={t("Revoke access")}
        pendingLabel={t("Revoking…")}
        onConfirm={() => (confirm ? revoke(confirm) : undefined)}
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
  if (!profile) return <PageSkeleton label={t("Loading connected accounts…")} />;

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
        title={t("Connected accounts")}
        detail={t("Google and GitHub can sign in to the same Knotree account. Matching emails are not linked automatically.")}
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
                      {t("Connected")}
                    </Badge>
                  </>
                }
                description={profile.identities.find((i) => i.provider === "password")?.email ?? profile.email}
                aside={
                  <Link to="/account/security" className={buttonClass("ghost", "sm")}>
                    {t("Manage")}
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
                          {t("Connected")}
                        </Badge>
                      ) : null}
                    </>
                  }
                  description={identity ? identity.email ?? t("Connected") : t("Sign in with your {provider} account.", { provider: providerLabel(provider.id) })}
                  aside={
                    identity ? (
                      <Button
                        type="button"
                        variant="secondary"
                        size="sm"
                        pending={pending === provider.id}
                        onClick={() => setConfirm(provider.id)}
                      >
                        {t("Disconnect")}
                      </Button>
                    ) : (
                      <a className={buttonClass("secondary", "sm")} href={href}>
                        {t("Connect")}{' '}{providerLabel(provider.id)}
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
                description={identity.email ?? t("Connected")}
                aside={
                  <Button type="button" variant="secondary" size="sm" onClick={() => setConfirm(identity.provider)}>
                    {t("Disconnect")}
                  </Button>
                }
              />
            </li>
          ))}
        </ul>
      </Card>
      <p className="text-[13px] text-muted">
        {t("Keep at least one way to sign in. Disconnecting does not delete the account on the other service.")}
      </p>
      <ConfirmDialog
        open={confirm !== null}
        onOpenChange={(open) => {
          if (!open) setConfirm(null);
        }}
        title={t("Disconnect {provider}?", { provider: providerLabel(confirm ?? "") })}
        description={t("You will no longer be able to sign in to Knotree with this account. You can connect it again later.")}
        confirmLabel={t("Disconnect")}
        pendingLabel={t("Disconnecting…")}
        danger
        onConfirm={() => unlink(confirm ?? "")}
      />
    </div>
  );
}
