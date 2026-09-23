import { NavLink, Outlet } from "react-router";
import { api } from "../lib/api";

function Item({ to, children, end }: { to: string; children: string; end?: boolean }) {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) =>
        `block rounded-[6px] px-2.5 py-1.5 text-sm ${isActive ? "bg-white text-ink" : "text-muted hover:text-ink"}`
      }
    >
      {children}
    </NavLink>
  );
}

export function AuthShell({ children }: { children: React.ReactNode }) {
  return (
    <main className="grid min-h-full place-items-center px-4 py-10">
      <div className="w-full max-w-[360px]">
        <p className="mb-6 text-sm font-medium tracking-tight">Knotree</p>
        {children}
      </div>
    </main>
  );
}

export function AccountShell() {
  return (
    <div className="mx-auto grid min-h-full max-w-5xl md:grid-cols-[216px_1fr]">
      <aside className="border-b border-line px-4 py-5 md:border-b-0 md:border-r">
        <p className="mb-4 text-sm font-medium">Knotree</p>
        <nav className="grid gap-0.5" aria-label="Account">
          <Item to="/account" end>
            Account
          </Item>
          <Item to="/account/profile">Profile</Item>
          <Item to="/account/security">Security</Item>
          <Item to="/account/sessions">Sessions</Item>
          <Item to="/account/connected-accounts">Connected accounts</Item>
        </nav>
        <button
          className="mt-6 text-sm text-muted hover:text-ink"
          type="button"
          onClick={() => {
            void api("/api/v1/auth/logout", { method: "POST" }).finally(() => {
              window.location.assign("/sign-in");
            });
          }}
        >
          Sign out
        </button>
      </aside>
      <div className="px-4 py-6 md:px-8">
        <Outlet />
      </div>
    </div>
  );
}

export function AdminShell() {
  return (
    <div className="mx-auto grid min-h-full max-w-6xl md:grid-cols-[216px_1fr]">
      <aside className="border-b border-line px-4 py-5 md:border-b-0 md:border-r">
        <p className="mb-1 text-sm font-medium">Knotree</p>
        <p className="mb-4 text-xs text-muted">Administration</p>
        <nav className="grid gap-0.5" aria-label="Administration">
          <Item to="/admin" end>
            Overview
          </Item>
          <Item to="/admin/users">Users</Item>
          <Item to="/admin/security">Security</Item>
          <Item to="/admin/logs">Logs</Item>
        </nav>
        <NavLink to="/account" className="mt-6 block text-sm text-muted hover:text-ink">
          Back to account
        </NavLink>
      </aside>
      <div className="px-4 py-6 md:px-8">
        <Outlet />
      </div>
    </div>
  );
}
