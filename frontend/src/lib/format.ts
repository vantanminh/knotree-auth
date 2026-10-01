import { intlLocale, t } from "./i18n";

export function formatWhen(value: string | null | undefined) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return new Intl.DateTimeFormat(intlLocale(), {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

export const eventLabels: Record<string, string> = {
  USER_REGISTERED: "Account created",
  LOGIN_SUCCESS: "Signed in",
  LOGIN_FAILED: "Sign-in failed",
  MFA_CHALLENGE_CREATED: "Verification code sent",
  MFA_SUCCESS: "Verification succeeded",
  MFA_FAILED: "Verification failed",
  MFA_ENABLED: "Two-factor authentication enabled",
  MFA_DISABLED: "Two-factor authentication disabled",
  PASSWORD_RESET_REQUESTED: "Password reset requested",
  PASSWORD_CHANGED: "Password changed",
  EMAIL_VERIFIED: "Email verified",
  EMAIL_CHANGED: "Email changed",
  SESSION_REVOKED: "Session revoked",
  ADMIN_ACTION: "Admin action",
  OAUTH_AUTHORIZED: "Application authorized",
  OAUTH_CONSENT_REVOKED: "Application access revoked",
  RECOVERY_CODES_REGENERATED: "Recovery codes regenerated",
  ACCOUNT_DELETED: "Account deletion requested",
  IDENTITY_LINKED: "Account connected",
  IDENTITY_UNLINKED: "Account disconnected",
  ACCOUNT_DELETION_REQUESTED: "Account deletion requested",
  EMAIL_CHANGE_REQUESTED: "Email change requested",
  PROFILE_UPDATED: "Profile updated",
};

export function eventLabel(type: string) {
  const label = eventLabels[type];
  if (label) return t(label);
  const words = type.replaceAll("_", " ").toLowerCase();
  return words.charAt(0).toUpperCase() + words.slice(1);
}

export function formatRelative(value: string | null | undefined) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  const seconds = Math.round((date.getTime() - Date.now()) / 1000);
  const abs = Math.abs(seconds);
  const rtf = new Intl.RelativeTimeFormat(intlLocale(), { numeric: "auto" });
  if (abs < 45) return t("just now");
  if (abs < 3600) return rtf.format(Math.round(seconds / 60), "minute");
  if (abs < 86400) return rtf.format(Math.round(seconds / 3600), "hour");
  if (abs < 86400 * 7) return rtf.format(Math.round(seconds / 86400), "day");
  return formatWhen(value);
}

export function formatDate(value: string | null | undefined) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return new Intl.DateTimeFormat(intlLocale(), { dateStyle: "medium" }).format(date);
}

export function statusLabel(status: string) {
  const labels: Record<string, string> = {
    active: "Active",
    disabled: "Disabled",
    pending_deletion: "Pending deletion",
  };
  const label = labels[status];
  return label ? t(label) : status.replaceAll("_", " ");
}

export function providerLabel(provider: string) {
  const labels: Record<string, string> = { google: "Google", github: "GitHub", password: "Email and password" };
  const label = labels[provider];
  return label ? t(label) : provider;
}

export function isMobileDevice(device: string) {
  return /iphone|ipad|android|mobile|ios/i.test(device);
}
