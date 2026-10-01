import { getLocale, locales, setLocale, t, useLocale, type Locale } from "../lib/i18n";
import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { Link, NavLink, Outlet, useLocation } from "react-router";
import { ApiError, api, signInLocation } from "../lib/api";
import type { Profile } from "../lib/types";
import {
  ActivityIcon,
  AppIcon,
  ArrowLeftIcon,
  ChartIcon,
  DevicesIcon,
  HomeIcon,
  LinkIcon,
  LogOutIcon,
  Logo,
  MailIcon,
  ShieldIcon,
  UserIcon,
  UsersIcon,
} from "./icons";
import { Avatar, Skeleton, Spinner } from "./ui";

/* ------------------------------------------------------------------ */
/* Auth                                                                */
/* ------------------------------------------------------------------ */

export function AuthShell({ children, footer }: { children: ReactNode; footer?: ReactNode }) {
  return (
    <div className="flex min-h-dvh flex-col">
      <header className="flex items-center justify-between px-5 py-5 sm:px-8">
        <Logo />
        <LanguageSwitcher />
      </header>
      <main className="flex flex-1 items-start justify-center px-4 pb-12 pt-4 sm:items-center sm:pt-0">
        <div className="w-full max-w-[400px] animate-fade-up">
          <div className="rounded-[14px] border border-line bg-surface px-6 py-7 shadow-raised sm:px-8 sm:py-8">
            {children}
          </div>
          {footer ? <div className="mt-6 text-center text-sm text-muted">{footer}</div> : null}
        </div>
      </main>
      <footer className="px-5 pb-6 text-center text-[12.5px] text-faint sm:px-8">
        © {new Date().getFullYear()} Knotree · {t("One account for every Knotree service")}
      </footer>
    </div>
  );
}

export function AuthHeading({ icon, title, children }: { icon?: ReactNode; title: string; children?: ReactNode }) {
  return (
    <div className="mb-6">
      {icon ? (
        <span className="mb-4 flex h-10 w-10 items-center justify-center rounded-[10px] border border-pine-line bg-pine-soft text-pine">
          {icon}
        </span>
      ) : null}
      <h1 className="text-[22px] font-semibold leading-tight tracking-[-0.015em] text-ink">{title}</h1>
      {children ? <p className="mt-1.5 text-[14.5px] text-muted">{children}</p> : null}
    </div>
  );
}

export function TextLink({ to, children, className = "" }: { to: string; children: ReactNode; className?: string }) {
  return (
    <Link
      to={to}
      className={`font-medium text-pine underline decoration-pine/25 underline-offset-[3px] transition-colors hover:decoration-pine ${className}`}
    >
      {children}
    </Link>
  );
}

/* ------------------------------------------------------------------ */
/* Language                                                            */
/* ------------------------------------------------------------------ */

/**
 * Switches the interface language. When signed in, the choice is also saved
 * to the account so email and notifications use the same language.
 */
export function LanguageSwitcher({ signedIn = false, className = "" }: { signedIn?: boolean; className?: string }) {
  const locale = useLocale();
  return (
    <select
      aria-label={t("Language")}
      value={locale}
      className={`h-8 rounded-[7px] border border-line bg-surface px-2 text-[13px] text-ink-soft transition-colors hover:text-ink ${className}`}
      onChange={(event) => {
        const next = event.target.value as Locale;
        if (!signedIn) {
          setLocale(next);
          return;
        }
        // Save first so the profile reload after the switch sees the new value.
        void api("/api/v1/me/profile", { method: "PATCH", body: JSON.stringify({ locale: next }) })
          .catch(() => undefined)
          .finally(() => setLocale(next));
      }}
    >
      {locales.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  );
}

/* ------------------------------------------------------------------ */
/* Signed-in profile context                                           */
/* ------------------------------------------------------------------ */

type ProfileState = {
  profile: Profile | null;
  error: string;
  reload: () => Promise<void>;
};

const ProfileContext = createContext<ProfileState | null>(null);

function useLoadProfile(): ProfileState {
  const [profile, setProfile] = useState<Profile | null>(null);
  const [error, setError] = useState("");
  const reload = useCallback(async () => {
    const loaded = await api<Profile>("/api/v1/me");
    // The account's saved language wins over the browser's.
    if (loaded.locale && loaded.locale !== getLocale()) setLocale(loaded.locale);
    setProfile(loaded);
  }, []);
  useEffect(() => {
    void reload().catch((err: unknown) => {
      if (err instanceof ApiError && err.status === 401) window.location.assign(signInLocation());
      else setError(err instanceof ApiError ? err.message : t("Could not load your account."));
    });
  }, [reload]);
  return { profile, error, reload };
}

export function useProfile(): ProfileState {
  const shared = useContext(ProfileContext);
  if (!shared) throw new Error("useProfile must be used inside an account shell");
  return shared;
}

/* ------------------------------------------------------------------ */
/* App shells                                                          */
/* ------------------------------------------------------------------ */

type NavItem = { to: string; label: string; icon: ReactNode; end?: boolean };

function SideLink({ item }: { item: NavItem }) {
  return (
    <NavLink
      to={item.to}
      end={item.end}
      className={({ isActive }) =>
        `group relative flex h-9 items-center gap-2.5 rounded-[7px] px-2.5 text-[14px] transition-colors duration-150 ${
          isActive
            ? "bg-surface font-medium text-ink shadow-[0_0_0_1px_var(--color-line),0_1px_2px_rgb(28_25_23/0.05)]"
            : "text-muted hover:bg-sunken hover:text-ink"
        }`
      }
    >
      {({ isActive }) => (
        <>
          <span className={isActive ? "text-pine" : "text-faint transition-colors group-hover:text-ink-soft"}>
            {item.icon}
          </span>
          {t(item.label)}
        </>
      )}
    </NavLink>
  );
}

function TabLink({ item }: { item: NavItem }) {
  return (
    <NavLink
      to={item.to}
      end={item.end}
      className={({ isActive }) =>
        `relative flex h-11 shrink-0 items-center gap-2 px-1 text-[14px] transition-colors ${
          isActive
            ? "font-medium text-ink after:absolute after:inset-x-0 after:bottom-0 after:h-[2px] after:rounded-full after:bg-pine"
            : "text-muted hover:text-ink"
        }`
      }
    >
      {t(item.label)}
    </NavLink>
  );
}

function SignOutButton({ compact }: { compact?: boolean }) {
  const [pending, setPending] = useState(false);
  return (
    <button
      type="button"
      aria-label={t("Sign out")}
      title={t("Sign out")}
      disabled={pending}
      className={`inline-flex shrink-0 items-center justify-center gap-2 rounded-[7px] text-muted transition-colors hover:bg-sunken hover:text-ink disabled:opacity-60 ${
        compact ? "h-9 w-9" : "h-8 w-8"
      }`}
      onClick={() => {
        setPending(true);
        void api("/api/v1/auth/logout", { method: "POST" }).finally(() => {
          window.location.assign("/sign-in");
        });
      }}
    >
      {pending ? <Spinner /> : <LogOutIcon />}
    </button>
  );
}

function UserChip({ profile }: { profile: Profile | null }) {
  if (!profile) {
    return (
      <div className="flex items-center gap-2.5 px-1">
        <Skeleton className="h-8 w-8 rounded-full" />
        <div className="grid flex-1 gap-1.5">
          <Skeleton className="h-3 w-24" />
          <Skeleton className="h-3 w-32" />
        </div>
      </div>
    );
  }
  const name = profile.display_name || profile.email.split("@")[0] || t("Account");
  return (
    <div className="flex items-center gap-2.5">
      <Avatar name={profile.display_name || profile.email} />
      <div className="min-w-0 flex-1 leading-tight">
        <p className="truncate text-[13.5px] font-medium text-ink">{name}</p>
        <p className="truncate text-[12.5px] text-muted" title={profile.email}>
          {profile.email}
        </p>
      </div>
      <SignOutButton />
    </div>
  );
}

function AppFrame({
  label,
  badge,
  nav,
  secondary,
  state,
}: {
  label: string;
  badge?: string;
  nav: NavItem[];
  secondary?: NavItem[];
  state: ProfileState;
}) {
  const location = useLocation();
  useEffect(() => {
    window.scrollTo({ top: 0 });
  }, [location.pathname]);

  return (
    <ProfileContext.Provider value={state}>
      <div className="min-h-dvh md:grid md:grid-cols-[256px_1fr]">
        {/* Desktop sidebar */}
        <aside className="sticky top-0 hidden h-dvh flex-col border-r border-line bg-paper md:flex">
          <div className="flex h-16 items-center gap-2 px-5">
            <Logo suffix={badge} />
          </div>
          <nav className="grid gap-0.5 px-3 pt-2" aria-label={label}>
            {nav.map((item) => (
              <SideLink key={item.to} item={item} />
            ))}
          </nav>
          {secondary?.length ? (
            <div className="mx-3 mt-5 grid gap-0.5 border-t border-line pt-4">
              {secondary.map((item) => (
                <SideLink key={item.to} item={item} />
              ))}
            </div>
          ) : null}
          <div className="mt-auto border-t border-line px-4 py-4">
            <LanguageSwitcher signedIn className="mb-3 w-full" />
            <UserChip profile={state.profile} />
          </div>
        </aside>

        {/* Mobile header */}
        <header className="sticky top-0 z-30 border-b border-line bg-paper/95 backdrop-blur md:hidden">
          <div className="flex h-14 items-center justify-between px-4">
            <Logo suffix={badge} />
            <div className="flex items-center gap-1">
              <LanguageSwitcher signedIn className="mr-1" />
              {state.profile ? <Avatar name={state.profile.display_name || state.profile.email} size={28} /> : null}
              <SignOutButton compact />
            </div>
          </div>
          <nav className="flex gap-5 overflow-x-auto px-4 [mask-image:linear-gradient(to_right,black_88%,transparent)] [scrollbar-width:none]" aria-label={`${label} (mobile)`}>
            {[...nav, ...(secondary ?? [])].map((item) => (
              <TabLink key={item.to} item={item} />
            ))}
          </nav>
        </header>

        <main className="min-w-0">
          <div key={location.pathname} className="mx-auto w-full max-w-[880px] animate-fade-up px-4 py-8 sm:px-8 md:py-12">
            <Outlet />
          </div>
        </main>
      </div>
    </ProfileContext.Provider>
  );
}

const accountNav: NavItem[] = [
  { to: "/account", label: "Overview", icon: <HomeIcon />, end: true },
  { to: "/account/profile", label: "Profile", icon: <UserIcon /> },
  { to: "/account/security", label: "Security", icon: <ShieldIcon /> },
  { to: "/account/sessions", label: "Sessions", icon: <DevicesIcon /> },
  { to: "/account/connected-accounts", label: "Connected accounts", icon: <LinkIcon /> },
];

const adminNav: NavItem[] = [
  { to: "/admin", label: "Overview", icon: <ChartIcon />, end: true },
  { to: "/admin/users", label: "Users", icon: <UsersIcon /> },
  { to: "/admin/security", label: "Security events", icon: <ActivityIcon /> },
  { to: "/admin/logs", label: "Email logs", icon: <MailIcon /> },
];

export function AccountShell() {
  const state = useLoadProfile();
  const secondary = state.profile?.is_admin
    ? [{ to: "/admin", label: "Administration", icon: <AppIcon /> }]
    : undefined;
  return <AppFrame label={t("Account")} nav={accountNav} secondary={secondary} state={state} />;
}

export function AdminShell() {
  const state = useLoadProfile();
  return (
    <AppFrame
      label={t("Administration")}
      badge={t("Admin")}
      nav={adminNav}
      secondary={[{ to: "/account", label: "Back to account", icon: <ArrowLeftIcon />, end: true }]}
      state={state}
    />
  );
}
