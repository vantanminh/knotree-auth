import { useEffect, useState, type ReactNode } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { ActivityList } from "./account";
import {
  ArrowLeftIcon,
  BanIcon,
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
  ConfirmDialog,
  CopyButton,
  EmptyState,
  PageSkeleton,
  PageTitle,
  Skeleton,
  buttonClass,
} from "../components/ui";
import { ApiError, api } from "../lib/api";
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

const numberFormat = new Intl.NumberFormat();

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
      .catch((err: unknown) => setError(err instanceof ApiError ? err.message : "Could not load overview."));
  }, []);
  if (error) return <Alert>{error}</Alert>;
  if (!stats) return <PageSkeleton label="Loading overview…" />;

  const attempts = stats.logins_today.success + stats.logins_today.failure;
  const successRate = percent(stats.logins_today.success, attempts);

  return (
    <div className="grid gap-6">
      <PageTitle
        title="Overview"
        detail="Identity operations for Knotree."
        actions={
          <Link to="/admin/users" className={buttonClass("secondary", "md")}>
            <UsersIcon size={15} />
            Browse users
          </Link>
        }
      />

      {stats.security_alerts_today > 0 ? (
        <Alert tone="warning" title={`${stats.security_alerts_today} security alerts today`}>
          Failed sign-ins, failed verifications, and disabled accounts.{" "}
          <Link to="/admin/security" className="font-medium underline underline-offset-2">
            Review events
          </Link>
        </Alert>
      ) : null}

      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        <Metric
          label="Users"
          value={stats.users.total}
          icon={<UsersIcon />}
          detail={
            <>
              +{numberFormat.format(stats.users.created_today)} today · +{numberFormat.format(stats.users.created_this_week)} this week
            </>
          }
        />
        <Metric label="New this month" value={stats.users.created_this_month} icon={<CheckCircleIcon />} detail="Accounts created since the 1st." />
        <Metric
          label="MFA enabled"
          value={stats.mfa_enabled_users}
          icon={<ShieldIcon />}
          meter={{ value: percent(stats.mfa_enabled_users, stats.users.total) }}
          detail={`${percent(stats.mfa_enabled_users, stats.users.total)}% of users`}
        />
        <Metric label="Active sessions" value={stats.active_sessions} icon={<DevicesIcon />} detail="Signed-in browsers right now." />
      </div>

      <div className="grid gap-3 lg:grid-cols-2">
        <Card>
          <CardHeader title="Email verification" description="Share of accounts with a verified address." />
          <div className="px-5 pb-5 pt-5 sm:px-6">
            <div className="flex h-2 overflow-hidden rounded-full bg-sunken">
              <div className="h-full bg-pine" style={{ width: `${percent(stats.users.verified, stats.users.total)}%` }} />
              <div className="h-full bg-amber/70" style={{ width: `${percent(stats.users.unverified, stats.users.total)}%` }} />
            </div>
            <dl className="mt-4 grid grid-cols-2 gap-4 text-sm">
              <div>
                <dt className="flex items-center gap-2 text-muted">
                  <span className="h-2 w-2 rounded-full bg-pine" />
                  Verified
                </dt>
                <dd className="tabular mt-1 text-[20px] font-semibold tracking-tight text-ink">{numberFormat.format(stats.users.verified)}</dd>
              </div>
              <div>
                <dt className="flex items-center gap-2 text-muted">
                  <span className="h-2 w-2 rounded-full bg-amber/70" />
                  Unverified
                </dt>
                <dd className="tabular mt-1 text-[20px] font-semibold tracking-tight text-ink">{numberFormat.format(stats.users.unverified)}</dd>
              </div>
            </dl>
          </div>
        </Card>
        <Card>
          <CardHeader
            title="Sign-ins today"
            description={attempts ? `${successRate}% succeeded across ${numberFormat.format(attempts)} attempts.` : "No sign-in attempts yet today."}
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
                  Sign-ins today
                </dt>
                <dd className="tabular mt-1 text-[20px] font-semibold tracking-tight text-ink">
                  {numberFormat.format(stats.logins_today.success)}
                </dd>
              </div>
              <div>
                <dt className="flex items-center gap-2 text-muted">
                  <span className="h-2 w-2 rounded-full bg-danger/80" />
                  Failed sign-ins today
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
      setError(err instanceof ApiError ? err.message : "Could not search users.");
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
      <PageTitle title="Users" detail="Search by user ID, email, or name." />
      {error ? <Alert>{error}</Alert> : null}
      <Card>
        <form className="flex gap-2 border-b border-line p-3 sm:p-4" onSubmit={search}>
          <SearchBar label="Search" value={q} onChange={setQ} placeholder="User ID, email, or name" />
          <Button type="submit" pending={pending}>
            Search
          </Button>
        </form>
        {!items ? (
          <TableSkeleton />
        ) : items.length === 0 ? (
          <EmptyState icon={<UsersIcon />} title="No users match">
            Try a different email, name, or user ID.
          </EmptyState>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[680px] text-left text-sm">
              <thead>
                <tr className="border-b border-line bg-paper/70 text-[12px] uppercase tracking-[0.06em] text-muted">
                  <th className="px-5 py-2.5 font-medium">User</th>
                  <th className="px-3 py-2.5 font-medium">Status</th>
                  <th className="px-3 py-2.5 font-medium">MFA</th>
                  <th className="px-3 py-2.5 font-medium">Last sign-in</th>
                  <th className="px-3 py-2.5 font-medium">Created</th>
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
                            {user.display_name || "No name"}
                            {!user.email_verified ? <span className="text-amber">· Unverified</span> : null}
                          </span>
                        </div>
                      </div>
                    </td>
                    <td className="px-3 py-3">
                      <StatusBadge status={user.status} />
                    </td>
                    <td className="px-3 py-3">
                      {user.mfa_enabled ? <Badge tone="success">On</Badge> : <Badge tone="outline">Off</Badge>}
                    </td>
                    <td className="tabular px-3 py-3 text-muted" title={formatWhen(user.last_login_at)}>
                      {user.last_login_at ? formatRelative(user.last_login_at) : "Never"}
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
    void load().catch((err: unknown) => setError(err instanceof ApiError ? err.message : "Could not load this user."));
  }, [id]);

  async function act(action: Action) {
    setError("");
    setMessage("");
    try {
      await api(action.path, { method: "POST" });
      setMessage(`${action.label}: action recorded.`);
      await load();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "The action was not applied.");
    } finally {
      setConfirm(null);
    }
  }

  if (error && !user) return <Alert>{error}</Alert>;
  if (!user) return <PageSkeleton label="Loading user…" />;

  const events = user.security_events ?? [];
  const base = `/api/v1/admin/users/${id}`;
  const disabled = user.status === "disabled";
  const actions: Action[] = [
    disabled
      ? {
          key: "enable",
          path: `${base}/enable`,
          label: "Enable",
          title: "Enable this account?",
          description: "The user will be able to sign in again.",
          icon: <CheckCircleIcon size={15} />,
        }
      : {
          key: "disable",
          path: `${base}/disable`,
          label: "Disable",
          title: "Disable this account?",
          description: "The user is signed out everywhere and can’t sign in until the account is enabled again.",
          icon: <BanIcon size={15} />,
          danger: true,
        },
    {
      key: "revoke",
      path: `${base}/revoke-sessions`,
      label: "Revoke sessions",
      title: "Revoke all sessions?",
      description: "Every browser signed in to this account will be signed out.",
      icon: <DevicesIcon size={15} />,
    },
    {
      key: "reset",
      path: `${base}/force-password-reset`,
      label: "Force password reset",
      title: "Force a password reset?",
      description: "The user must choose a new password at their next sign-in.",
      icon: <KeyIcon size={15} />,
      danger: true,
    },
  ];

  return (
    <div className="grid gap-6">
      <Link to="/admin/users" className="inline-flex w-fit items-center gap-1.5 text-[13px] font-medium text-muted transition-colors hover:text-ink">
        <ArrowLeftIcon size={14} />
        All users
      </Link>
      <header className="flex flex-col gap-4 sm:flex-row sm:items-center">
        <Avatar name={user.display_name || user.email} size={52} />
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-[24px] font-semibold leading-tight tracking-[-0.015em] text-ink">{user.email}</h1>
          <div className="mt-1.5 flex flex-wrap items-center gap-2">
            <StatusBadge status={user.status} />
            {user.email_verified ? <Badge tone="success">Email verified</Badge> : <Badge tone="warning">Email unverified</Badge>}
            {user.must_reset_password ? <Badge tone="warning">Password reset pending</Badge> : null}
          </div>
        </div>
      </header>

      {error ? <Alert>{error}</Alert> : null}
      {message ? <Alert tone="success">{message}</Alert> : null}

      <Card>
        <CardHeader title="Details" />
        <dl className="mt-4 divide-y divide-line border-t border-line">
          <Detail label="User ID">
            <span className="flex flex-wrap items-center gap-2">
              <code className="break-all font-mono text-[13px]">{user.id}</code>
              <CopyButton value={user.id} />
            </span>
          </Detail>
          <Detail label="Name">{user.display_name || <span className="text-muted">Not set</span>}</Detail>
          <Detail label="Created">{formatWhen(user.created_at)}</Detail>
          <Detail label="Last sign-in">{formatWhen(user.last_login_at)}</Detail>
          <Detail label="Active sessions">
            <span className="tabular">{user.active_sessions ?? 0}</span>
          </Detail>
          {user.mfa ? (
            <Detail label="Two-factor">
              <span className="flex flex-wrap gap-1.5">
                <Badge tone={user.mfa.totp_enabled ? "success" : "outline"}>Authenticator {user.mfa.totp_enabled ? "on" : "off"}</Badge>
                <Badge tone={user.mfa.email_enabled ? "success" : "outline"}>Email codes {user.mfa.email_enabled ? "on" : "off"}</Badge>
                {user.mfa.totp_enabled ? <Badge tone="neutral">{user.mfa.recovery_codes_remaining} recovery codes</Badge> : null}
              </span>
            </Detail>
          ) : null}
          {user.identities?.length ? (
            <Detail label="Sign-in methods">
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
        <CardHeader title="Actions" description="Every action is recorded in the security log." />
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
        <CardHeader title="Security activity" />
        <div className="mt-4 border-t border-line">
          <ActivityList events={events} empty="This user has no recorded security events." />
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
        confirmLabel={confirm?.label ?? "Confirm"}
        pendingLabel="Applying…"
        danger={confirm?.danger}
        onConfirm={() => (confirm ? act(confirm) : undefined)}
      />
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
      setError(err instanceof ApiError ? err.message : "Could not load events.");
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
        title="Security"
        detail="Authentication and administrative events. These records are not deleted from this screen."
      />
      {error ? <Alert>{error}</Alert> : null}
      <Card>
        <div className="flex flex-col gap-2 border-b border-line p-3 sm:flex-row sm:items-center sm:p-4">
          <label className="relative flex-1">
            <span className="sr-only">Event type</span>
            <select
              value={eventType}
              onChange={(event) => setEventType(event.target.value)}
              className="h-10 w-full appearance-none rounded-[6px] border border-line-strong bg-white pl-3 pr-9 text-[14.5px] text-ink outline-none transition-[border-color,box-shadow] hover:border-[#c4beb3] focus:border-pine focus:shadow-[0_0_0_3px_rgb(31_61_50/0.12)]"
            >
              <option value="">All event types</option>
              {options.map((type) => (
                <option key={type} value={type}>
                  {eventLabel(type)} ({type})
                </option>
              ))}
            </select>
            <ChevronRightIcon className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 rotate-90 text-faint" />
          </label>
          <Button type="button" variant="secondary" icon={<RefreshIcon size={15} />} onClick={() => void load(eventType)}>
            Refresh
          </Button>
        </div>
        {!items ? (
          <TableSkeleton />
        ) : items.length === 0 ? (
          <EmptyState icon={<ShieldIcon />} title="No events">
            No security events match this filter.
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
                          User {item.target_user_id.slice(0, 8)}
                        </Link>
                      ) : null}
                      {item.request_id ? <span className="font-mono">req {item.request_id}</span> : null}
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
        setError(err instanceof ApiError ? err.message : "Could not load logs.");
        setItems([]);
      });
  }, []);
  return (
    <div className="grid gap-6">
      <PageTitle title="Logs" detail="Email delivery attempts. Message bodies are not shown." />
      {error ? <Alert>{error}</Alert> : null}
      <Card>
        {!items ? (
          <TableSkeleton />
        ) : items.length === 0 ? (
          <EmptyState icon={<MailIcon />} title="No email sent yet">
            Delivery attempts will appear here.
          </EmptyState>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[640px] text-left text-sm">
              <thead>
                <tr className="border-b border-line bg-paper/70 text-[12px] uppercase tracking-[0.06em] text-muted">
                  <th className="px-5 py-2.5 font-medium">Template</th>
                  <th className="px-3 py-2.5 font-medium">Recipient</th>
                  <th className="px-3 py-2.5 font-medium">Status</th>
                  <th className="px-5 py-2.5 text-right font-medium">Sent</th>
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
