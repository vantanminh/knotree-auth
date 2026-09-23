export function formatWhen(value: string | null | undefined) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

export function eventLabel(type: string) {
  const labels: Record<string, string> = {
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
    RECOVERY_CODES_REGENERATED: "Recovery codes regenerated",
    ACCOUNT_DELETED: "Account deletion requested",
    IDENTITY_LINKED: "Account connected",
    IDENTITY_UNLINKED: "Account disconnected",
  };
  return labels[type] ?? type.replaceAll("_", " ").toLowerCase();
}
