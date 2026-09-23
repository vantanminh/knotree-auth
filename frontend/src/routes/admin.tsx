import { useEffect, useState } from "react";
import { Link, useParams } from "react-router";
import { Alert, Button, PageTitle, TextField } from "../components/ui";
import { ApiError, api } from "../lib/api";
import { eventLabel, formatWhen } from "../lib/format";
import type { SecurityEvent } from "../lib/types";

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

function Metric({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="border-b border-line py-3">
      <p className="text-sm text-muted">{label}</p>
      <p className="text-2xl font-medium tracking-tight">{value}</p>
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
  if (!stats) return <p className="text-sm text-muted">Loading overview…</p>;
  return (
    <section>
      <PageTitle title="Overview" detail="Identity operations for Knotree." />
      <div className="grid gap-x-8 sm:grid-cols-2 lg:grid-cols-4">
        <Metric label="Users" value={stats.users.total} />
        <Metric label="New this month" value={stats.users.created_this_month} />
        <Metric label="MFA enabled" value={stats.mfa_enabled_users} />
        <Metric label="Active sessions" value={stats.active_sessions} />
        <Metric label="Verified" value={stats.users.verified} />
        <Metric label="Unverified" value={stats.users.unverified} />
        <Metric label="Sign-ins today" value={stats.logins_today.success} />
        <Metric label="Failed sign-ins today" value={stats.logins_today.failure} />
      </div>
      <p className="mt-6 text-sm text-muted">Security alerts today: {stats.security_alerts_today}</p>
      <p className="text-sm text-muted">
        New today {stats.users.created_today} · this week {stats.users.created_this_week}
      </p>
    </section>
  );
}

export function AdminUsers() {
  const [q, setQ] = useState("");
  const [items, setItems] = useState<UserRow[]>([]);
  const [error, setError] = useState("");
  async function search(event?: React.FormEvent) {
    event?.preventDefault();
    try {
      const body = await api<{ items: UserRow[] }>(`/api/v1/admin/users?q=${encodeURIComponent(q)}`);
      setItems(body.items);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not search users.");
    }
  }
  useEffect(() => {
    void search();
  }, []);
  return (
    <section>
      <PageTitle title="Users" />
      {error ? <Alert>{error}</Alert> : null}
      <form className="mb-4 flex gap-2" onSubmit={search}>
        <div className="flex-1">
          <TextField label="Search" name="q" value={q} onChange={setQ} hint="User ID, email, or name" />
        </div>
        <Button type="submit" className="mt-6">
          Search
        </Button>
      </form>
      <div className="overflow-x-auto">
        <table className="w-full min-w-[640px] text-left text-sm">
          <thead className="text-muted">
            <tr>
              <th className="py-2 font-medium">Email</th>
              <th className="py-2 font-medium">Status</th>
              <th className="py-2 font-medium">MFA</th>
              <th className="py-2 font-medium">Created</th>
            </tr>
          </thead>
          <tbody>
            {items.map((user) => (
              <tr key={user.id} className="border-t border-line">
                <td className="py-2">
                  <Link className="underline" to={`/admin/users/${user.id}`}>
                    {user.email}
                  </Link>
                  <div className="text-muted">{user.display_name}</div>
                </td>
                <td>{user.status}</td>
                <td>{user.mfa_enabled ? "On" : "Off"}</td>
                <td>{formatWhen(user.created_at)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {items.length === 0 ? <p className="mt-4 text-sm text-muted">No users match.</p> : null}
    </section>
  );
}

export function AdminUser() {
  const { id = "" } = useParams();
  const [user, setUser] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");

  async function load() {
    setUser(await api(`/api/v1/admin/users/${id}`));
  }
  useEffect(() => {
    void load().catch((err: unknown) => setError(err instanceof ApiError ? err.message : "Could not load this user."));
  }, [id]);

  async function act(path: string) {
    setError("");
    try {
      await api(path, { method: "POST" });
      setMessage("Action recorded.");
      await load();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "The action was not applied.");
    }
  }

  if (error && !user) return <Alert>{error}</Alert>;
  if (!user) return <p className="text-sm text-muted">Loading user…</p>;
  const events = (user.security_events as SecurityEvent[] | undefined) ?? [];
  return (
    <section className="max-w-2xl">
      <PageTitle title={String(user.email ?? "User")} detail={String(user.id ?? "")} />
      {error ? <Alert>{error}</Alert> : null}
      {message ? <p className="mb-3 text-sm">{message}</p> : null}
      <dl className="grid gap-2 text-sm">
        <div>Status: {String(user.status)}</div>
        <div>Email verified: {user.email_verified ? "Yes" : "No"}</div>
        <div>Created: {formatWhen(String(user.created_at ?? ""))}</div>
        <div>Last sign-in: {formatWhen(user.last_login_at ? String(user.last_login_at) : null)}</div>
        <div>Active sessions: {String(user.active_sessions ?? 0)}</div>
      </dl>
      <div className="mt-4 flex flex-wrap gap-2">
        <Button type="button" variant="quiet" onClick={() => void act(`/api/v1/admin/users/${id}/disable`)}>
          Disable
        </Button>
        <Button type="button" variant="quiet" onClick={() => void act(`/api/v1/admin/users/${id}/enable`)}>
          Enable
        </Button>
        <Button type="button" variant="quiet" onClick={() => void act(`/api/v1/admin/users/${id}/revoke-sessions`)}>
          Revoke sessions
        </Button>
        <Button type="button" variant="danger" onClick={() => void act(`/api/v1/admin/users/${id}/force-password-reset`)}>
          Force password reset
        </Button>
      </div>
      <h2 className="mt-8 text-base font-medium">Security activity</h2>
      <ul className="mt-2 divide-y divide-line text-sm">
        {events.map((event) => (
          <li key={event.id} className="flex justify-between py-2">
            <span>{eventLabel(event.event_type)}</span>
            <time dateTime={event.occurred_at}>{formatWhen(event.occurred_at)}</time>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function AdminSecurity() {
  const [items, setItems] = useState<SecurityEvent[]>([]);
  const [eventType, setEventType] = useState("");
  const [error, setError] = useState("");
  async function load(event?: React.FormEvent) {
    event?.preventDefault();
    const query = eventType ? `?event_type=${encodeURIComponent(eventType)}` : "";
    try {
      const body = await api<{ items: SecurityEvent[] }>(`/api/v1/admin/security-events${query}`);
      setItems(body.items);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not load events.");
    }
  }
  useEffect(() => {
    void load();
  }, []);
  return (
    <section>
      <PageTitle title="Security" detail="Authentication and administrative events. These records are not deleted from this screen." />
      {error ? <Alert>{error}</Alert> : null}
      <form className="mb-4 flex gap-2" onSubmit={load}>
        <div className="flex-1">
          <TextField label="Event type" name="event" value={eventType} onChange={setEventType} hint="For example LOGIN_FAILED" />
        </div>
        <Button type="submit" className="mt-6">
          Filter
        </Button>
      </form>
      <ul className="divide-y divide-line text-sm">
        {items.map((item) => (
          <li key={item.id} className="grid gap-1 py-3 md:grid-cols-[1fr_auto]">
            <div>
              <p>{eventLabel(item.event_type)}</p>
              <p className="text-muted">
                {item.result}
                {item.ip ? ` · ${item.ip}` : ""}
                {item.request_id ? ` · ${item.request_id}` : ""}
              </p>
            </div>
            <time className="text-muted" dateTime={item.occurred_at}>
              {formatWhen(item.occurred_at)}
            </time>
          </li>
        ))}
      </ul>
    </section>
  );
}

type MailLog = {
  id: string;
  to: string;
  template: string;
  status: string;
  created_at: string;
  error: string | null;
};

export function AdminLogs() {
  const [items, setItems] = useState<MailLog[]>([]);
  const [error, setError] = useState("");
  useEffect(() => {
    void api<{ items: MailLog[] }>("/api/v1/admin/logs")
      .then((body) => setItems(body.items))
      .catch((err: unknown) => setError(err instanceof ApiError ? err.message : "Could not load logs."));
  }, []);
  return (
    <section>
      <PageTitle title="Logs" detail="Email delivery attempts. Message bodies are not shown." />
      {error ? <Alert>{error}</Alert> : null}
      <ul className="divide-y divide-line text-sm">
        {items.map((item) => (
          <li key={item.id} className="py-3">
            <p>
              {item.template} · {item.status}
            </p>
            <p className="text-muted">
              {item.to} · {formatWhen(item.created_at)}
              {item.error ? ` · ${item.error}` : ""}
            </p>
          </li>
        ))}
      </ul>
    </section>
  );
}
