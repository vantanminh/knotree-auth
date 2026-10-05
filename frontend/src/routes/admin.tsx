import { intlLocale, t } from "../lib/i18n";
import { useEffect, useState, type ReactNode } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { ActivityList } from "./account";
import {
  AppIcon,
  ArrowLeftIcon,
  BanIcon,
  ChartIcon,
  CheckCircleIcon,
  ChevronRightIcon,
  DevicesIcon,
  KeyIcon,
  MailIcon,
  RefreshIcon,
  SearchIcon,
  ShieldIcon,
  UsersIcon,
  WarningIcon,
} from "../components/icons";
import {
  Alert,
  Avatar,
  Badge,
  Button,
  Card,
  CardHeader,
  ClientLogo,
  ConfirmDialog,
  CopyButton,
  EmptyState,
  PageSkeleton,
  PageTitle,
  Skeleton,
  buttonClass,
} from "../components/ui";
import { ApiError, api } from "../lib/api";
import { ServiceManagement } from "./admin-services";
import { eventLabel, eventLabels, formatDate, formatRelative, formatWhen, statusLabel } from "../lib/format";
import type { MfaSummary, SecurityEvent } from "../lib/types";

type Stats = {
  users: {
    total: number;
    verified: number;
    unverified: number;
    created_today: number;
    created_this_week: number;
    created_this_month: number;
  };
  active_sessions: number;
  mfa_enabled_users: number;
  logins_today: { success: number; failure: number };
  security_alerts_today: number;
};

type UserRow = {
  id: string;
  username?: string;
  display_name: string | null;
  email: string;
  email_verified: boolean;
  created_at: string;
  last_login_at: string | null;
  status: string;
  mfa_enabled: boolean;
};

type UserDetail = {
  id: string;
  username?: string;
  emails?: { id: string; email: string; primary: boolean; verified: boolean }[];
  display_name: string | null;
  email: string;
  created_at: string;
  last_login_at: string | null;
  status: string;
  email_verified: boolean;
  must_reset_password?: boolean;
  identities?: { provider: string; email: string | null; email_verified?: boolean }[];
  active_sessions?: number;
  mfa?: MfaSummary;
  security_events?: SecurityEvent[];
};

type ClientRow = {
  id: string;
  name: string;
  client_type: "public" | "confidential" | "service";
  status: string;
  first_party: boolean;
  require_pkce: boolean;
  allowed_scopes: string[];
  redirect_uris: string[];
  has_secret: boolean;
  created_at: string;
  authorized_users: number;
  active_tokens: number;
  last_authorized_at: string | null;
  description: string | null;
  homepage_url: string | null;
  logo_url: string | null;
};

type ClientDetail = ClientRow & {
  recent_consents: { user_id: string; email: string | null; scopes: string[]; granted_at: string }[];
};

const numberFormat = { format: (value: number) => new Intl.NumberFormat(intlLocale()).format(value) };

function percent(part: number, whole: number) {
  if (!whole) return 0;
  return Math.round((part / whole) * 100);
}

function StatusBadge({ status }: { status: string }) {
  const tone = status === "active" ? "success" : status === "disabled" ? "danger" : "warning";
  return (
    <Badge tone={tone} dot>
      {statusLabel(status)}
    </Badge>
  );
}

/* ------------------------------------------------------------------ */
/* Overview                                                            */
/* ------------------------------------------------------------------ */

function Metric({
  label,
  value,
  detail,
  icon,
  meter,
}: {
  label: string;
  value: number;
  detail?: ReactNode;
  icon?: ReactNode;
  meter?: { value: number; tone?: "pine" | "danger" | "amber" };
}) {
  const tones = { pine: "bg-pine", danger: "bg-danger", amber: "bg-amber" };
  return (
    <div className="flex flex-col rounded-[var(--radius-card)] border border-line bg-surface p-5 shadow-card">
      <div className="flex items-center justify-between gap-2">
        <p className="text-[13px] font-medium text-muted">{label}</p>
        {icon ? <span className="text-faint">{icon}</span> : null}
      </div>
      <p className="tabular mt-3 text-[28px] font-semibold leading-none tracking-[-0.02em] text-ink">
        {numberFormat.format(value)}
      </p>
      {meter ? (
        <div className="mt-4 h-1 overflow-hidden rounded-full bg-sunken">
          <div className={`h-full rounded-full ${tones[meter.tone ?? "pine"]}`} style={{ width: `${Math.min(meter.value, 100)}%` }} />
        </div>
      ) : null}
      {detail ? <p className="mt-2.5 text-[12.5px] text-muted">{detail}</p> : null}
    </div>
  );
}

export function AdminOverview() {
  const [stats, setStats] = useState<Stats | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    void api<Stats>("/api/v1/admin/stats")
      .then(setStats)
      .catch((err: unknown) => setError(err instanceof ApiError ? err.message : t("Could not load overview.")));
  }, []);
  if (error) return <Alert>{error}</Alert>;
  if (!stats) return <PageSkeleton label={t("Loading overview…")} />;

  const attempts = stats.logins_today.success + stats.logins_today.failure;
  const successRate = percent(stats.logins_today.success, attempts);

  return (
    <div className="grid gap-6">
      <PageTitle
        title={t("Overview")}
        detail={t("Identity operations for Knotree.")}
        actions={
          <div className="flex flex-wrap gap-2">
            <Link to="/admin/analytics" className={buttonClass("secondary", "md")}>
              <ChartIcon size={15} />
              {t("View analytics")}
            </Link>
            <Link to="/admin/users" className={buttonClass("secondary", "md")}>
              <UsersIcon size={15} />
              {t("Browse users")}
            </Link>
          </div>
        }
      />

      {stats.security_alerts_today > 0 ? (
        <Alert tone="warning" title={t("{count} security alerts today", { count: stats.security_alerts_today })}>
          {t("Failed sign-ins, failed verifications, and disabled accounts.")}{" "}
          <Link to="/admin/security" className="font-medium underline underline-offset-2">
            {t("Review events")}
          </Link>
        </Alert>
      ) : null}

      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        <Metric
          label={t("Users")}
          value={stats.users.total}
          icon={<UsersIcon />}
          detail={
            <>
              {t("+{today} today · +{week} this week", { today: numberFormat.format(stats.users.created_today), week: numberFormat.format(stats.users.created_this_week) })}
            </>
          }
        />
        <Metric label={t("New this month")} value={stats.users.created_this_month} icon={<CheckCircleIcon />} detail={t("Accounts created since the 1st.")} />
        <Metric
          label={t("MFA enabled")}
          value={stats.mfa_enabled_users}
          icon={<ShieldIcon />}
          meter={{ value: percent(stats.mfa_enabled_users, stats.users.total) }}
          detail={`${percent(stats.mfa_enabled_users, stats.users.total)}% of users`}
        />
        <Metric label={t("Active sessions")} value={stats.active_sessions} icon={<DevicesIcon />} detail={t("Signed-in browsers right now.")} />
      </div>

      <div className="grid gap-3 lg:grid-cols-2">
        <Card>
          <CardHeader title={t("Email verification")} description={t("Share of accounts with a verified address.")} />
          <div className="px-5 pb-5 pt-5 sm:px-6">
            <div className="flex h-2 overflow-hidden rounded-full bg-sunken">
              <div className="h-full bg-pine" style={{ width: `${percent(stats.users.verified, stats.users.total)}%` }} />
              <div className="h-full bg-amber/70" style={{ width: `${percent(stats.users.unverified, stats.users.total)}%` }} />
            </div>
            <dl className="mt-4 grid grid-cols-2 gap-4 text-sm">
              <div>
                <dt className="flex items-center gap-2 text-muted">
                  <span className="h-2 w-2 rounded-full bg-pine" />
                  {t("Verified")}
                </dt>
                <dd className="tabular mt-1 text-[20px] font-semibold tracking-tight text-ink">{numberFormat.format(stats.users.verified)}</dd>
              </div>
              <div>
                <dt className="flex items-center gap-2 text-muted">
                  <span className="h-2 w-2 rounded-full bg-amber/70" />
                  {t("Unverified")}
                </dt>
                <dd className="tabular mt-1 text-[20px] font-semibold tracking-tight text-ink">{numberFormat.format(stats.users.unverified)}</dd>
              </div>
            </dl>
          </div>
        </Card>
        <Card>
          <CardHeader
            title={t("Sign-ins today")}
            description={attempts ? t("{rate}% succeeded across {count} attempts.", { rate: successRate, count: numberFormat.format(attempts) }) : t("No sign-in attempts yet today.")}
          />
          <div className="px-5 pb-5 pt-5 sm:px-6">
            <div className="flex h-2 overflow-hidden rounded-full bg-sunken">
              <div className="h-full bg-pine" style={{ width: `${percent(stats.logins_today.success, attempts)}%` }} />
              <div className="h-full bg-danger/80" style={{ width: `${percent(stats.logins_today.failure, attempts)}%` }} />
            </div>
            <dl className="mt-4 grid grid-cols-2 gap-4 text-sm">
              <div>
                <dt className="flex items-center gap-2 text-muted">
                  <span className="h-2 w-2 rounded-full bg-pine" />
                  {t("Sign-ins today")}
                </dt>
                <dd className="tabular mt-1 text-[20px] font-semibold tracking-tight text-ink">
                  {numberFormat.format(stats.logins_today.success)}
                </dd>
              </div>
              <div>
                <dt className="flex items-center gap-2 text-muted">
                  <span className="h-2 w-2 rounded-full bg-danger/80" />
                  {t("Failed sign-ins today")}
                </dt>
                <dd className="tabular mt-1 text-[20px] font-semibold tracking-tight text-ink">
                  {numberFormat.format(stats.logins_today.failure)}
                </dd>
              </div>
            </dl>
          </div>
        </Card>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Users                                                               */
/* ------------------------------------------------------------------ */

function SearchBar({
  value,
  onChange,
  placeholder,
  label,
  children,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  label: string;
  children?: ReactNode;
}) {
  return (
    <div className="relative flex-1">
      <SearchIcon className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-faint" />
      <input
        type="search"
        aria-label={label}
        placeholder={placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className="h-10 w-full rounded-[6px] border border-line-strong bg-white pl-9 pr-3 text-[14.5px] text-ink outline-none transition-[border-color,box-shadow] placeholder:text-faint hover:border-[#c4beb3] focus:border-pine focus:shadow-[0_0_0_3px_rgb(31_61_50/0.12)]"
      />
      {children}
    </div>
  );
}

function TableSkeleton({ rows = 5 }: { rows?: number }) {
  return (
    <div className="divide-y divide-line">
      {Array.from({ length: rows }, (_, index) => (
        <div key={index} className="flex items-center gap-3 px-5 py-3.5">
          <Skeleton className="h-8 w-8 rounded-full" />
          <div className="grid flex-1 gap-1.5">
            <Skeleton className="h-3.5 w-52" />
            <Skeleton className="h-3 w-32" />
          </div>
          <Skeleton className="hidden h-5 w-16 rounded-full sm:block" />
        </div>
      ))}
    </div>
  );
}

export function AdminUsers() {
  const navigate = useNavigate();
  const [q, setQ] = useState("");
  const [items, setItems] = useState<UserRow[] | null>(null);
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);

  async function search(event?: React.FormEvent) {
    event?.preventDefault();
    setPending(true);
    setError("");
    try {
      const body = await api<{ items: UserRow[] }>(`/api/v1/admin/users?q=${encodeURIComponent(q)}`);
      setItems(body.items);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : t("Could not search users."));
      setItems((current) => current ?? []);
    } finally {
      setPending(false);
    }
  }

  useEffect(() => {
    void search();
  }, []);

  return (
    <div className="grid gap-6">
      <PageTitle title={t("Users")} detail={t("Search by user ID, email, or name.")} />
      {error ? <Alert>{error}</Alert> : null}
      <Card>
        <form className="flex gap-2 border-b border-line p-3 sm:p-4" onSubmit={search}>
          <SearchBar label={t("Search")} value={q} onChange={setQ} placeholder={t("User ID, email, or name")} />
          <Button type="submit" pending={pending}>
            {t("Search")}
          </Button>
        </form>
        {!items ? (
          <TableSkeleton />
        ) : items.length === 0 ? (
          <EmptyState icon={<UsersIcon />} title={t("No users match")}>
            {t("Try a different email, name, or user ID.")}
          </EmptyState>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[680px] text-left text-sm">
              <thead>
                <tr className="border-b border-line bg-paper/70 text-[12px] uppercase tracking-[0.06em] text-muted">
                  <th className="px-5 py-2.5 font-medium">{t("User")}</th>
                  <th className="px-3 py-2.5 font-medium">{t("Status")}</th>
                  <th className="px-3 py-2.5 font-medium">{t("MFA")}</th>
                  <th className="px-3 py-2.5 font-medium">{t("Last sign-in")}</th>
                  <th className="px-3 py-2.5 font-medium">{t("Created")}</th>
                  <th className="w-10" />
                </tr>
              </thead>
              <tbody className="divide-y divide-line">
                {items.map((user) => (
                  <tr
                    key={user.id}
                    className="group cursor-pointer transition-colors hover:bg-paper/80"
                    onClick={() => navigate(`/admin/users/${user.id}`)}
                  >
                    <td className="px-5 py-3">
                      <div className="flex items-center gap-3">
                        <Avatar name={user.display_name || user.email} />
                        <div className="min-w-0">
                          <Link
                            className="block truncate font-medium text-ink hover:underline"
                            to={`/admin/users/${user.id}`}
                            onClick={(event) => event.stopPropagation()}
                          >
                            {user.email}
                          </Link>
                          <span className="flex items-center gap-1.5 text-[12.5px] text-muted">
                            {user.username ? `@${user.username} · ` : null}
                            {user.display_name || t("No name")}
                            {!user.email_verified ? <span className="text-amber">{t("· Unverified")}</span> : null}
                          </span>
                        </div>
                      </div>
                    </td>
                    <td className="px-3 py-3">
                      <StatusBadge status={user.status} />
                    </td>
                    <td className="px-3 py-3">
                      {user.mfa_enabled ? <Badge tone="success">{t("On")}</Badge> : <Badge tone="outline">{t("Off")}</Badge>}
                    </td>
                    <td className="tabular px-3 py-3 text-muted" title={formatWhen(user.last_login_at)}>
                      {user.last_login_at ? formatRelative(user.last_login_at) : t("Never")}
                    </td>
                    <td className="tabular px-3 py-3 text-muted">{formatDate(user.created_at)}</td>
                    <td className="pr-4 text-faint">
                      <ChevronRightIcon className="transition-transform group-hover:translate-x-0.5" />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* User detail                                                         */
/* ------------------------------------------------------------------ */

type Action = {
  key: string;
  path: string;
  label: string;
  title: string;
  description: string;
  icon: ReactNode;
  danger?: boolean;
};

function Detail({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid gap-1 px-5 py-3 sm:grid-cols-[180px_1fr] sm:gap-4 sm:px-6">
      <dt className="text-[13px] text-muted">{label}</dt>
      <dd className="min-w-0 text-sm text-ink">{children}</dd>
    </div>
  );
}

export function AdminUser() {
  const { id = "" } = useParams();
  const [user, setUser] = useState<UserDetail | null>(null);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [confirm, setConfirm] = useState<Action | null>(null);

  async function load() {
    setUser(await api<UserDetail>(`/api/v1/admin/users/${id}`));
  }
  useEffect(() => {
    void load().catch((err: unknown) => setError(err instanceof ApiError ? err.message : t("Could not load this user.")));
  }, [id]);

  async function act(action: Action) {
    setError("");
    setMessage("");
    try {
      await api(action.path, { method: "POST" });
      setMessage(t("{action}: action recorded.", { action: action.label }));
      await load();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : t("The action was not applied."));
    } finally {
      setConfirm(null);
    }
  }

  if (error && !user) return <Alert>{error}</Alert>;
  if (!user) return <PageSkeleton label={t("Loading user…")} />;

  const events = user.security_events ?? [];
  const base = `/api/v1/admin/users/${id}`;
  const disabled = user.status === "disabled";
  const actions: Action[] = [
    disabled
      ? {
          key: "enable",
          path: `${base}/enable`,
          label: t("Enable"),
          title: t("Enable this account?"),
          description: t("The user will be able to sign in again."),
          icon: <CheckCircleIcon size={15} />,
        }
      : {
          key: "disable",
          path: `${base}/disable`,
          label: t("Disable"),
          title: t("Disable this account?"),
          description: t("The user is signed out everywhere and can’t sign in until the account is enabled again."),
          icon: <BanIcon size={15} />,
          danger: true,
        },
    {
      key: "revoke",
      path: `${base}/revoke-sessions`,
      label: t("Revoke sessions"),
      title: t("Revoke all sessions?"),
      description: t("Every browser signed in to this account will be signed out."),
      icon: <DevicesIcon size={15} />,
    },
    {
      key: "reset",
      path: `${base}/force-password-reset`,
      label: t("Force password reset"),
      title: t("Force a password reset?"),
      description: t("The user must choose a new password at their next sign-in."),
      icon: <KeyIcon size={15} />,
      danger: true,
    },
  ];

  return (
    <div className="grid gap-6">
      <Link to="/admin/users" className="inline-flex w-fit items-center gap-1.5 text-[13px] font-medium text-muted transition-colors hover:text-ink">
        <ArrowLeftIcon size={14} />
        {t("All users")}
      </Link>
      <header className="flex flex-col gap-4 sm:flex-row sm:items-center">
        <Avatar name={user.display_name || user.email} size={52} />
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-[24px] font-semibold leading-tight tracking-[-0.015em] text-ink">{user.email}</h1>
          <div className="mt-1.5 flex flex-wrap items-center gap-2">
            <StatusBadge status={user.status} />
            {user.email_verified ? <Badge tone="success">{t("Email verified")}</Badge> : <Badge tone="warning">{t("Email unverified")}</Badge>}
            {user.must_reset_password ? <Badge tone="warning">{t("Password reset pending")}</Badge> : null}
          </div>
        </div>
      </header>

      {error ? <Alert>{error}</Alert> : null}
      {message ? <Alert tone="success">{message}</Alert> : null}

      <Card>
        <CardHeader title={t("Details")} />
        <dl className="mt-4 divide-y divide-line border-t border-line">
          <Detail label={t("User ID")}>
            <span className="flex flex-wrap items-center gap-2">
              <code className="break-all font-mono text-[13px]">{user.id}</code>
              <CopyButton value={user.id} />
            </span>
          </Detail>
          <Detail label={t("Name")}>{user.display_name || <span className="text-muted">{t("Not set")}</span>}</Detail>
          <Detail label={t("Username")}>{user.username ?? <span className="text-muted">{t("Not set")}</span>}</Detail>
          {user.emails && user.emails.length > 1 ? (
            <Detail label={t("Email addresses")}>
              {user.emails
                .map((item) => `${item.email}${item.primary ? ` (${t("Primary")})` : ""}${item.verified ? "" : ` (${t("Unverified")})`}`)
                .join(", ")}
            </Detail>
          ) : null}
          <Detail label={t("Created")}>{formatWhen(user.created_at)}</Detail>
          <Detail label={t("Last sign-in")}>{formatWhen(user.last_login_at)}</Detail>
          <Detail label={t("Active sessions")}>
            <span className="tabular">{user.active_sessions ?? 0}</span>
          </Detail>
          {user.mfa ? (
            <Detail label={t("Two-factor")}>
              <span className="flex flex-wrap gap-1.5">
                <Badge tone={user.mfa.totp_enabled ? "success" : "outline"}>{user.mfa.totp_enabled ? t("Authenticator on") : t("Authenticator off")}</Badge>
                <Badge tone={user.mfa.email_enabled ? "success" : "outline"}>{user.mfa.email_enabled ? t("Email codes on") : t("Email codes off")}</Badge>
                {user.mfa.totp_enabled ? <Badge tone="neutral">{t("{count} recovery codes", { count: user.mfa.recovery_codes_remaining })}</Badge> : null}
              </span>
            </Detail>
          ) : null}
          {user.identities?.length ? (
            <Detail label={t("Sign-in methods")}>
              <span className="flex flex-wrap gap-1.5">
                {user.identities.map((identity) => (
                  <Badge key={identity.provider} tone="neutral">
                    {identity.provider}
                    {identity.email ? ` · ${identity.email}` : ""}
                  </Badge>
                ))}
              </span>
            </Detail>
          ) : null}
        </dl>
      </Card>

      <Card>
        <CardHeader title={t("Actions")} description={t("Every action is recorded in the security log.")} />
        <div className="mt-4 divide-y divide-line border-t border-line">
          {actions.map((action) => (
            <div key={action.key} className="flex flex-col gap-3 px-5 py-3.5 sm:flex-row sm:items-center sm:justify-between sm:px-6">
              <div className="min-w-0">
                <p className="text-sm font-medium text-ink">{action.label}</p>
                <p className="text-[13px] text-muted">{action.description}</p>
              </div>
              <Button
                type="button"
                size="sm"
                variant={action.danger ? "danger" : "secondary"}
                icon={action.icon}
                className="self-start sm:self-auto"
                onClick={() => setConfirm(action)}
              >
                {action.label}
              </Button>
            </div>
          ))}
        </div>
      </Card>

      <Card>
        <CardHeader title={t("Security activity")} />
        <div className="mt-4 border-t border-line">
          <ActivityList events={events} empty={t("This user has no recorded security events.")} />
        </div>
      </Card>

      <ConfirmDialog
        open={confirm !== null}
        onOpenChange={(open) => {
          if (!open) setConfirm(null);
        }}
        title={confirm?.title ?? ""}
        description={
          <>
            {confirm?.description} <span className="text-ink">{user.email}</span>
          </>
        }
        confirmLabel={confirm?.label ?? t("Confirm")}
        pendingLabel={t("Applying…")}
        danger={confirm?.danger}
        onConfirm={() => (confirm ? act(confirm) : undefined)}
      />
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Services (OAuth clients)                                            */
/* ------------------------------------------------------------------ */

function clientTypeLabel(type: string) {
  const labels: Record<string, string> = {
    public: "Public",
    confidential: "Confidential",
    service: "Service",
  };
  const label = labels[type];
  return label ? t(label) : type;
}

export function AdminClients() {
  const navigate = useNavigate();
  const [q, setQ] = useState("");
  const [items, setItems] = useState<ClientRow[] | null>(null);
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);

  async function search(event?: React.FormEvent) {
    event?.preventDefault();
    setPending(true);
    setError("");
    try {
      const body = await api<{ items: ClientRow[] }>(`/api/v1/admin/clients?q=${encodeURIComponent(q)}`);
      setItems(body.items);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : t("Could not load services."));
      setItems((current) => current ?? []);
    } finally {
      setPending(false);
    }
  }

  useEffect(() => {
    void search();
  }, []);

  return (
    <div className="grid gap-6">
      <PageTitle
        title={t("Services")}
        detail={t("Every service registered to sign users in with this account system.")}
        actions={
          <Link to="/admin/clients/new" className={buttonClass("primary")}>
            {t("New service")}
          </Link>
        }
      />
      {error ? <Alert>{error}</Alert> : null}
      <Card>
        <form className="flex gap-2 border-b border-line p-3 sm:p-4" onSubmit={search}>
          <SearchBar label={t("Search")} value={q} onChange={setQ} placeholder={t("Client ID or name")} />
          <Button type="submit" pending={pending}>
            {t("Search")}
          </Button>
        </form>
        {!items ? (
          <TableSkeleton />
        ) : items.length === 0 ? (
          <EmptyState icon={<AppIcon />} title={t("No services match")}>
            {t("Try a different client ID or name.")}
          </EmptyState>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[680px] text-left text-sm">
              <thead>
                <tr className="border-b border-line bg-paper/70 text-[12px] uppercase tracking-[0.06em] text-muted">
                  <th className="px-5 py-2.5 font-medium">{t("Service")}</th>
                  <th className="px-3 py-2.5 font-medium">{t("Status")}</th>
                  <th className="px-3 py-2.5 font-medium">{t("Type")}</th>
                  <th className="px-3 py-2.5 text-right font-medium">{t("Authorized users")}</th>
                  <th className="px-3 py-2.5 font-medium">{t("Last authorized")}</th>
                  <th className="w-10" />
                </tr>
              </thead>
              <tbody className="divide-y divide-line">
                {items.map((client) => (
                  <tr
                    key={client.id}
                    className="group cursor-pointer transition-colors hover:bg-paper/80"
                    onClick={() => navigate(`/admin/clients/${encodeURIComponent(client.id)}`)}
                  >
                    <td className="px-5 py-3">
                      <div className="flex min-w-0 items-center gap-3">
                      <ClientLogo name={client.name} src={client.logo_url} size={36} />
                      <div className="min-w-0">
                        <Link
                          className="block truncate font-medium text-ink hover:underline"
                          to={`/admin/clients/${encodeURIComponent(client.id)}`}
                          onClick={(event) => event.stopPropagation()}
                        >
                          {client.name}
                        </Link>
                        <code className="font-mono text-[12.5px] text-muted">{client.id}</code>
                      </div>
                      </div>
                    </td>
                    <td className="px-3 py-3">
                      <StatusBadge status={client.status} />
                    </td>
                    <td className="px-3 py-3">
                      <span className="flex flex-wrap gap-1.5">
                        <Badge tone="outline">{clientTypeLabel(client.client_type)}</Badge>
                        {client.first_party ? (
                          <Badge tone="neutral">{t("First-party")}</Badge>
                        ) : (
                          <Badge tone="outline">{t("Third-party")}</Badge>
                        )}
                      </span>
                    </td>
                    <td className="tabular px-3 py-3 text-right text-ink">{numberFormat.format(client.authorized_users)}</td>
                    <td className="tabular px-3 py-3 text-muted" title={formatWhen(client.last_authorized_at)}>
                      {client.last_authorized_at ? formatRelative(client.last_authorized_at) : t("Never")}
                    </td>
                    <td className="pr-4 text-faint">
                      <ChevronRightIcon className="transition-transform group-hover:translate-x-0.5" />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>
    </div>
  );
}

export function AdminClient() {
  const { id = "" } = useParams();
  const [client, setClient] = useState<ClientDetail | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    api<ClientDetail>(`/api/v1/admin/clients/${encodeURIComponent(id)}`)
      .then(setClient)
      .catch((err: unknown) => setError(err instanceof ApiError ? err.message : t("Could not load this service.")));
  }, [id]);

  if (error) return <Alert>{error}</Alert>;
  if (!client) return <PageSkeleton label={t("Loading service…")} />;

  return (
    <div className="grid gap-6">
      <Link to="/admin/clients" className="inline-flex w-fit items-center gap-1.5 text-[13px] font-medium text-muted transition-colors hover:text-ink">
        <ArrowLeftIcon size={14} />
        {t("All services")}
      </Link>
      <header className="flex min-w-0 items-center gap-4">
        <ClientLogo name={client.name} src={client.logo_url} size={56} />
        <div className="min-w-0">
        <h1 className="truncate text-[24px] font-semibold leading-tight tracking-[-0.015em] text-ink">{client.name}</h1>
        <div className="mt-1.5 flex flex-wrap items-center gap-2">
          <StatusBadge status={client.status} />
          <Badge tone="outline">{clientTypeLabel(client.client_type)}</Badge>
          {client.first_party ? <Badge tone="neutral">{t("First-party")}</Badge> : <Badge tone="outline">{t("Third-party")}</Badge>}
        </div>
        {client.description ? <p className="mt-2 max-w-prose text-[14px] text-ink-soft">{client.description}</p> : null}
        </div>
      </header>

      <ServiceManagement client={client} onChange={(next) => setClient({ ...client, ...next })} />

      <Card>
        <CardHeader title={t("Details")} />
        <dl className="mt-4 divide-y divide-line border-t border-line">
          <Detail label={t("Client ID")}>
            <span className="flex flex-wrap items-center gap-2">
              <code className="break-all font-mono text-[13px]">{client.id}</code>
              <CopyButton value={client.id} />
            </span>
          </Detail>
          <Detail label={t("Created")}>{formatWhen(client.created_at)}</Detail>
          <Detail label={t("PKCE required")}>{client.require_pkce ? t("Yes") : t("No")}</Detail>
          <Detail label={t("Client secret")}>{client.has_secret ? t("Configured") : t("None")}</Detail>
          <Detail label={t("Authorized users")}>
            <span className="tabular">{numberFormat.format(client.authorized_users)}</span>
          </Detail>
          <Detail label={t("Active access tokens")}>
            <span className="tabular">{numberFormat.format(client.active_tokens)}</span>
          </Detail>
          <Detail label={t("Last authorized")}>{formatWhen(client.last_authorized_at)}</Detail>
          <Detail label={t("Allowed scopes")}>
            <span className="flex flex-wrap gap-1.5">
              {client.allowed_scopes.map((scope) => (
                <Badge key={scope} tone="neutral">
                  {scope}
                </Badge>
              ))}
            </span>
          </Detail>
          <Detail label={t("Redirect URIs")}>
            {client.redirect_uris.length ? (
              <ul className="grid gap-1">
                {client.redirect_uris.map((uri) => (
                  <li key={uri}>
                    <code className="break-all font-mono text-[13px]">{uri}</code>
                  </li>
                ))}
              </ul>
            ) : (
              <span className="text-muted">{t("None")}</span>
            )}
          </Detail>
        </dl>
      </Card>

      <Card>
        <CardHeader title={t("Recent authorizations")} description={t("The latest users who granted this service access.")} />
        <div className="mt-4 border-t border-line">
          {client.recent_consents.length === 0 ? (
            <EmptyState icon={<UsersIcon />} title={t("No authorizations yet")}>
              {t("No user has authorized this service.")}
            </EmptyState>
          ) : (
            <ul className="divide-y divide-line">
              {client.recent_consents.map((consent) => (
                <li key={consent.user_id} className="flex flex-col gap-1 px-5 py-3 sm:flex-row sm:items-center sm:justify-between sm:px-6">
                  <div className="min-w-0">
                    <Link className="block truncate text-sm font-medium text-ink hover:underline" to={`/admin/users/${consent.user_id}`}>
                      {consent.email ?? consent.user_id}
                    </Link>
                    <span className="text-[12.5px] text-muted">{consent.scopes.join(" ")}</span>
                  </div>
                  <time className="tabular text-[12.5px] text-muted" dateTime={consent.granted_at} title={formatWhen(consent.granted_at)}>
                    {formatRelative(consent.granted_at)}
                  </time>
                </li>
              ))}
            </ul>
          )}
        </div>
      </Card>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Security events                                                     */
/* ------------------------------------------------------------------ */

export function AdminSecurity() {
  const [items, setItems] = useState<SecurityEvent[] | null>(null);
  const [eventType, setEventType] = useState("");
  const [error, setError] = useState("");

  async function load(type: string) {
    setError("");
    const query = type ? `?event_type=${encodeURIComponent(type)}` : "";
    try {
      const body = await api<{ items: SecurityEvent[] }>(`/api/v1/admin/security-events${query}`);
      setItems(body.items);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : t("Could not load events."));
      setItems((current) => current ?? []);
    }
  }
  useEffect(() => {
    void load(eventType);
  }, [eventType]);

  const options = Object.keys(eventLabels).sort((a, b) => eventLabel(a).localeCompare(eventLabel(b)));

  return (
    <div className="grid gap-6">
      <PageTitle
        title={t("Security")}
        detail={t("Authentication and administrative events. These records are not deleted from this screen.")}
      />
      {error ? <Alert>{error}</Alert> : null}
      <Card>
        <div className="flex flex-col gap-2 border-b border-line p-3 sm:flex-row sm:items-center sm:p-4">
          <label className="relative flex-1">
            <span className="sr-only">{t("Event type")}</span>
            <select
              value={eventType}
              onChange={(event) => setEventType(event.target.value)}
              className="h-10 w-full appearance-none rounded-[6px] border border-line-strong bg-white pl-3 pr-9 text-[14.5px] text-ink outline-none transition-[border-color,box-shadow] hover:border-[#c4beb3] focus:border-pine focus:shadow-[0_0_0_3px_rgb(31_61_50/0.12)]"
            >
              <option value="">{t("All event types")}</option>
              {options.map((type) => (
                <option key={type} value={type}>
                  {eventLabel(type)} ({type})
                </option>
              ))}
            </select>
            <ChevronRightIcon className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 rotate-90 text-faint" />
          </label>
          <Button type="button" variant="secondary" icon={<RefreshIcon size={15} />} onClick={() => void load(eventType)}>
            {t("Refresh")}
          </Button>
        </div>
        {!items ? (
          <TableSkeleton />
        ) : items.length === 0 ? (
          <EmptyState icon={<ShieldIcon />} title={t("No events")}>
            {t("No security events match this filter.")}
          </EmptyState>
        ) : (
          <ul className="divide-y divide-line">
            {items.map((item) => (
              <li key={item.id} className="grid gap-2 px-5 py-3.5 sm:grid-cols-[1fr_auto] sm:items-center sm:gap-4">
                <div className="flex min-w-0 items-start gap-3">
                  <span
                    className={`mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full ${
                      item.result === "failure" ? "bg-danger-soft text-danger" : "bg-pine-soft text-pine"
                    }`}
                  >
                    {item.result === "failure" ? <WarningIcon size={14} /> : <CheckCircleIcon size={14} />}
                  </span>
                  <div className="min-w-0">
                    <p className="flex flex-wrap items-center gap-2 text-sm font-medium text-ink">
                      {eventLabel(item.event_type)}
                      <code className="font-mono text-[11.5px] font-normal text-faint">{item.event_type}</code>
                    </p>
                    <p className="mt-0.5 flex flex-wrap gap-x-3 text-[12.5px] text-muted">
                      <span className={item.result === "failure" ? "text-danger" : ""}>{item.result}</span>
                      {item.ip ? <span className="font-mono">{item.ip}</span> : null}
                      {item.target_user_id ? (
                        <Link className="hover:text-ink hover:underline" to={`/admin/users/${item.target_user_id}`}>
                          {t("User")}{' '}{item.target_user_id.slice(0, 8)}
                        </Link>
                      ) : null}
                      {item.request_id ? <span className="font-mono">req{' '}{item.request_id}</span> : null}
                    </p>
                  </div>
                </div>
                <time className="tabular pl-10 text-[12.5px] text-muted sm:pl-0 sm:text-right" dateTime={item.occurred_at} title={formatWhen(item.occurred_at)}>
                  {formatWhen(item.occurred_at)}
                </time>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Email logs                                                          */
/* ------------------------------------------------------------------ */

type MailLog = {
  id: string;
  to: string;
  template: string;
  status: string;
  created_at: string;
  error: string | null;
};

export function AdminLogs() {
  const [items, setItems] = useState<MailLog[] | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    void api<{ items: MailLog[] }>("/api/v1/admin/logs")
      .then((body) => setItems(body.items))
      .catch((err: unknown) => {
        setError(err instanceof ApiError ? err.message : t("Could not load logs."));
        setItems([]);
      });
  }, []);
  return (
    <div className="grid gap-6">
      <PageTitle title={t("Logs")} detail={t("Email delivery attempts. Message bodies are not shown.")} />
      {error ? <Alert>{error}</Alert> : null}
      <Card>
        {!items ? (
          <TableSkeleton />
        ) : items.length === 0 ? (
          <EmptyState icon={<MailIcon />} title={t("No email sent yet")}>
            {t("Delivery attempts will appear here.")}
          </EmptyState>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[640px] text-left text-sm">
              <thead>
                <tr className="border-b border-line bg-paper/70 text-[12px] uppercase tracking-[0.06em] text-muted">
                  <th className="px-5 py-2.5 font-medium">{t("Template")}</th>
                  <th className="px-3 py-2.5 font-medium">{t("Recipient")}</th>
                  <th className="px-3 py-2.5 font-medium">{t("Status")}</th>
                  <th className="px-5 py-2.5 text-right font-medium">{t("Sent")}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-line">
                {items.map((item) => {
                  const failed = Boolean(item.error) || /fail|error|bounce/i.test(item.status);
                  return (
                    <tr key={item.id} className="align-top">
                      <td className="px-5 py-3">
                        <code className="font-mono text-[13px] text-ink">{item.template}</code>
                      </td>
                      <td className="px-3 py-3 text-ink-soft">{item.to}</td>
                      <td className="px-3 py-3">
                        <Badge tone={failed ? "danger" : "success"} dot>
                          {item.status}
                        </Badge>
                        {item.error ? <p className="mt-1 max-w-xs text-[12.5px] text-danger">{item.error}</p> : null}
                      </td>
                      <td className="tabular px-5 py-3 text-right text-muted" title={formatWhen(item.created_at)}>
                        {formatRelative(item.created_at)}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </Card>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Analytics                                                           */
/* ------------------------------------------------------------------ */

type AnalyticsDay = {
  day: string;
  signups: number;
  logins_success: number;
  logins_failed: number;
  active_users: number;
};

type Breakdown = { key: string; count: number }[];

type Analytics = {
  days: number;
  totals: { signups: number; logins_success: number; logins_failed: number; users: number; mfa_users: number };
  active_users: { daily: number; weekly: number; monthly: number };
  series: AnalyticsDay[];
  login_methods: Breakdown;
  identity_providers: Breakdown;
  event_types: Breakdown;
  oauth_clients: Breakdown;
};

const ranges = [7, 30, 90] as const;

const methodLabels: Record<string, string> = {
  password: "Password",
  google: "Google",
  github: "GitHub",
  totp: "Authenticator app",
  email: "Email code",
  recovery: "Recovery code",
};

function shortDay(value: string) {
  return new Intl.DateTimeFormat(intlLocale(), { month: "short", day: "numeric", timeZone: "UTC" }).format(new Date(value));
}

function niceMax(value: number) {
  if (value <= 4) return 4;
  const step = 10 ** Math.floor(Math.log10(value));
  return Math.ceil(value / step) * step;
}

const chartWidth = 480;
const chartHeight = 200;
const chartPad = { top: 10, right: 8, bottom: 26, left: 36 };

function ChartFrame({ max, series, children }: { max: number; series: AnalyticsDay[]; children: ReactNode }) {
  const innerH = chartHeight - chartPad.top - chartPad.bottom;
  const ticks = [0, max / 2, max];
  const labelEvery = Math.ceil(series.length / 4);
  const innerW = chartWidth - chartPad.left - chartPad.right;
  return (
    <svg viewBox={`0 0 ${chartWidth} ${chartHeight}`} className="h-auto w-full" role="img" aria-hidden="true">
      {ticks.map((tick) => {
        const y = chartPad.top + innerH - (tick / max) * innerH;
        return (
          <g key={tick}>
            <line x1={chartPad.left} x2={chartWidth - chartPad.right} y1={y} y2={y} className="stroke-line" strokeWidth={1} />
            <text x={chartPad.left - 6} y={y + 3.5} textAnchor="end" className="fill-muted text-[12px] tabular">
              {numberFormat.format(Math.round(tick))}
            </text>
          </g>
        );
      })}
      {series.map((point, index) =>
        index % labelEvery === 0 || index === series.length - 1 ? (
          <text
            key={point.day}
            x={chartPad.left + ((index + 0.5) / series.length) * innerW}
            y={chartHeight - 6}
            textAnchor="middle"
            className="fill-muted text-[12px]"
          >
            {shortDay(point.day)}
          </text>
        ) : null,
      )}
      {children}
    </svg>
  );
}

function LoginBars({ series }: { series: AnalyticsDay[] }) {
  const max = niceMax(Math.max(...series.map((d) => d.logins_success + d.logins_failed), 0));
  const innerW = chartWidth - chartPad.left - chartPad.right;
  const innerH = chartHeight - chartPad.top - chartPad.bottom;
  const slot = innerW / series.length;
  const bar = Math.max(Math.min(slot * 0.7, 18), 1.5);
  return (
    <ChartFrame max={max} series={series}>
      {series.map((point, index) => {
        const x = chartPad.left + index * slot + (slot - bar) / 2;
        const okH = (point.logins_success / max) * innerH;
        const failH = (point.logins_failed / max) * innerH;
        const base = chartPad.top + innerH;
        return (
          <g key={point.day}>
            <title>
              {`${shortDay(point.day)}: ${t("{count} successful", { count: point.logins_success })}, ${t("{count} failed", { count: point.logins_failed })}`}
            </title>
            <rect x={x} y={base - okH} width={bar} height={okH} className="fill-pine" />
            <rect x={x} y={base - okH - failH} width={bar} height={failH} className="fill-danger/80" />
          </g>
        );
      })}
    </ChartFrame>
  );
}

function TrendLines({ series }: { series: AnalyticsDay[] }) {
  const max = niceMax(Math.max(...series.map((d) => Math.max(d.signups, d.active_users)), 0));
  const innerW = chartWidth - chartPad.left - chartPad.right;
  const innerH = chartHeight - chartPad.top - chartPad.bottom;
  const x = (index: number) => chartPad.left + ((index + 0.5) / series.length) * innerW;
  const y = (value: number) => chartPad.top + innerH - (value / max) * innerH;
  const path = (pick: (d: AnalyticsDay) => number) =>
    series.map((d, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(1)},${y(pick(d)).toFixed(1)}`).join(" ");
  const area = `${path((d) => d.active_users)} L${x(series.length - 1).toFixed(1)},${y(0)} L${x(0).toFixed(1)},${y(0)} Z`;
  return (
    <ChartFrame max={max} series={series}>
      <path d={area} className="fill-pine/10" />
      <path d={path((d) => d.active_users)} className="stroke-pine" fill="none" strokeWidth={2} strokeLinejoin="round" />
      <path d={path((d) => d.signups)} className="stroke-amber" fill="none" strokeWidth={2} strokeLinejoin="round" strokeDasharray="4 3" />
      {series.map((point, index) => (
        <rect key={point.day} x={x(index) - innerW / series.length / 2} y={chartPad.top} width={innerW / series.length} height={innerH} fill="transparent">
          <title>
            {`${shortDay(point.day)}: ${t("{count} active users", { count: point.active_users })}, ${t("{count} sign-ups", { count: point.signups })}`}
          </title>
        </rect>
      ))}
    </ChartFrame>
  );
}

function Legend({ items }: { items: { label: string; className: string; dashed?: boolean }[] }) {
  return (
    <div className="flex flex-wrap gap-4 text-[12.5px] text-muted">
      {items.map((item) => (
        <span key={item.label} className="inline-flex items-center gap-1.5">
          <span className={`inline-block h-2 w-3 rounded-[2px] ${item.className} ${item.dashed ? "opacity-80" : ""}`} />
          {item.label}
        </span>
      ))}
    </div>
  );
}

function BarList({ rows, label }: { rows: Breakdown; label: (key: string) => string }) {
  if (rows.length === 0) return <p className="px-5 pb-5 text-sm text-muted sm:px-6">{t("No data for this period.")}</p>;
  const max = Math.max(...rows.map((row) => row.count));
  const total = rows.reduce((sum, row) => sum + row.count, 0);
  return (
    <ul className="grid gap-2.5 px-5 pb-5 sm:px-6">
      {rows.map((row) => (
        <li key={row.key}>
          <div className="flex items-baseline justify-between gap-3 text-[13px]">
            <span className="truncate text-ink-soft">{label(row.key)}</span>
            <span className="tabular shrink-0 text-muted">
              {numberFormat.format(row.count)} · {percent(row.count, total)}%
            </span>
          </div>
          <div className="mt-1 h-1.5 overflow-hidden rounded-full bg-sunken">
            <div className="h-full rounded-full bg-pine" style={{ width: `${(row.count / max) * 100}%` }} />
          </div>
        </li>
      ))}
    </ul>
  );
}

export function AdminAnalytics() {
  const [days, setDays] = useState<(typeof ranges)[number]>(30);
  const [data, setData] = useState<Analytics | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let current = true;
    setError("");
    void api<Analytics>(`/api/v1/admin/analytics?days=${days}`)
      .then((value) => current && setData(value))
      .catch((err: unknown) => current && setError(err instanceof ApiError ? err.message : t("Could not load analytics.")));
    return () => {
      current = false;
    };
  }, [days]);

  const picker = (
    <div role="group" aria-label={t("Time range")} className="inline-flex rounded-[8px] border border-line bg-surface p-0.5">
      {ranges.map((range) => (
        <button
          key={range}
          type="button"
          aria-pressed={days === range}
          onClick={() => setDays(range)}
          className={`rounded-[6px] px-3 py-1.5 text-[13px] font-medium transition-colors ${
            days === range ? "bg-pine text-white" : "text-muted hover:text-ink"
          }`}
        >
          {t("{days} days", { days: range })}
        </button>
      ))}
    </div>
  );

  if (error) return <Alert>{error}</Alert>;
  if (!data) return <PageSkeleton label={t("Loading analytics…")} />;

  const attempts = data.totals.logins_success + data.totals.logins_failed;
  const failureRate = percent(data.totals.logins_failed, attempts);
  const mfaRate = percent(data.totals.mfa_users, data.totals.users);

  return (
    <div className="grid gap-6">
      <PageTitle title={t("Analytics")} detail={t("Usage and sign-in trends across Knotree accounts.")} actions={picker} />

      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        <Metric
          label={t("Active users")}
          value={data.active_users.daily}
          icon={<UsersIcon />}
          detail={t("{week} this week · {month} this month", {
            week: numberFormat.format(data.active_users.weekly),
            month: numberFormat.format(data.active_users.monthly),
          })}
        />
        <Metric label={t("New users")} value={data.totals.signups} icon={<CheckCircleIcon />} detail={t("In the last {days} days.", { days: data.days })} />
        <Metric
          label={t("Sign-ins")}
          value={data.totals.logins_success}
          icon={<KeyIcon />}
          meter={{ value: failureRate, tone: failureRate > 20 ? "danger" : "amber" }}
          detail={t("{rate}% failed attempts", { rate: failureRate })}
        />
        <Metric
          label={t("Two-step verification")}
          value={data.totals.mfa_users}
          icon={<ShieldIcon />}
          meter={{ value: mfaRate }}
          detail={t("{rate}% of users", { rate: mfaRate })}
        />
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader title={t("Active users and sign-ups")} description={t("Distinct users signing in, and new accounts, per day (UTC).")} />
          <div className="grid gap-3 px-5 pb-5 pt-4 sm:px-6">
            <TrendLines series={data.series} />
            <Legend
              items={[
                { label: t("Active users"), className: "bg-pine" },
                { label: t("Sign-ups"), className: "bg-amber", dashed: true },
              ]}
            />
          </div>
        </Card>
        <Card>
          <CardHeader title={t("Sign-in attempts")} description={t("Successful and failed sign-ins per day (UTC).")} />
          <div className="grid gap-3 px-5 pb-5 pt-4 sm:px-6">
            <LoginBars series={data.series} />
            <Legend
              items={[
                { label: t("Successful"), className: "bg-pine" },
                { label: t("Failed"), className: "bg-danger/80" },
              ]}
            />
          </div>
        </Card>
      </div>

      <div className="grid gap-6 md:grid-cols-2">
        <Card>
          <CardHeader title={t("Sign-in methods")} description={t("Factors used in successful sign-ins.")} />
          <div className="pt-4">
            <BarList rows={data.login_methods} label={(key) => t(methodLabels[key] ?? key)} />
          </div>
        </Card>
        <Card>
          <CardHeader title={t("Linked identities")} description={t("Users by sign-in provider, all time.")} />
          <div className="pt-4">
            <BarList rows={data.identity_providers} label={(key) => t(methodLabels[key] ?? key)} />
          </div>
        </Card>
        <Card>
          <CardHeader title={t("Top security events")} description={t("Most frequent event types in this period.")} />
          <div className="pt-4">
            <BarList rows={data.event_types} label={eventLabel} />
          </div>
        </Card>
        <Card>
          <CardHeader title={t("Connected apps")} description={t("Authorizations by OAuth client in this period.")} />
          <div className="pt-4">
            <BarList rows={data.oauth_clients} label={(key) => key} />
          </div>
        </Card>
      </div>
    </div>
  );
}
