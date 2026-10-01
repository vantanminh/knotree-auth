export type Profile = {
  id: string;
  display_name: string | null;
  status: string;
  created_at: string;
  email: string;
  email_verified: boolean;
  identities: { provider: string; email: string | null }[];
  mfa: MfaSummary;
  is_admin: boolean;
  locale?: "en" | "vi";
};

export type MfaSummary = {
  totp_enabled: boolean;
  email_enabled: boolean;
  recovery_codes_remaining: number;
  password_changed_at: string | null;
};

export type SessionItem = {
  id: string;
  created_at: string;
  last_active_at: string;
  expires_at: string;
  ip: string | null;
  user_agent: string | null;
  device: string;
  mfa: boolean;
  current: boolean;
};

export type SecurityEvent = {
  id: string;
  occurred_at: string;
  event_type: string;
  result: string;
  user_agent?: string | null;
  request_id?: string | null;
  ip?: string | null;
  target_user_id?: string | null;
  metadata?: Record<string, unknown>;
};

export type LoginResponse =
  | { status: "authenticated" }
  | { status: "mfa_required"; mfa_token: string; methods: string[]; masked_email: string | null };
