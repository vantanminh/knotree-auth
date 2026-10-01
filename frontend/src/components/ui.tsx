import { AlertDialog } from "@base-ui/react/alert-dialog";
import { Button as BaseButton } from "@base-ui/react/button";
import { Checkbox as BaseCheckbox } from "@base-ui/react/checkbox";
import { Field } from "@base-ui/react/field";
import { Input as BaseInput } from "@base-ui/react/input";
import { Switch as BaseSwitch } from "@base-ui/react/switch";
import { useEffect, useId, useState, type ComponentProps, type ReactNode } from "react";
import {
  AlertIcon,
  CheckCircleIcon,
  CheckIcon,
  CopyIcon,
  EyeIcon,
  EyeOffIcon,
  InfoIcon,
  WarningIcon,
} from "./icons";

/* ------------------------------------------------------------------ */
/* Buttons                                                             */
/* ------------------------------------------------------------------ */

type Variant = "primary" | "secondary" | "quiet" | "ghost" | "danger" | "danger-solid";
type Size = "sm" | "md" | "lg";

const variantStyles: Record<Variant, string> = {
  primary:
    "bg-pine text-white shadow-[inset_0_1px_0_rgb(255_255_255/0.08),0_1px_2px_rgb(28_25_23/0.18)] hover:bg-pine-hover active:bg-[#132a21]",
  secondary:
    "bg-surface text-ink border border-line-strong shadow-[0_1px_1px_rgb(28_25_23/0.04)] hover:bg-white hover:border-[#c4beb3] active:bg-sunken",
  quiet:
    "bg-surface text-ink border border-line-strong shadow-[0_1px_1px_rgb(28_25_23/0.04)] hover:bg-white hover:border-[#c4beb3] active:bg-sunken",
  ghost: "bg-transparent text-ink-soft hover:bg-sunken hover:text-ink active:bg-line/70",
  danger:
    "bg-surface text-danger border border-danger-line shadow-[0_1px_1px_rgb(28_25_23/0.04)] hover:bg-danger-soft hover:border-[#dcb3ac] active:bg-[#f1e0dd]",
  "danger-solid":
    "bg-danger text-white shadow-[inset_0_1px_0_rgb(255_255_255/0.08),0_1px_2px_rgb(28_25_23/0.18)] hover:bg-[#7a2525] active:bg-[#6a2020]",
};

const sizeStyles: Record<Size, string> = {
  sm: "h-8 gap-1.5 px-2.5 text-[13px]",
  md: "h-10 gap-2 px-3.5 text-[14px]",
  lg: "h-11 gap-2 px-4 text-[15px]",
};

export function buttonClass(variant: Variant = "primary", size: Size = "md", className = "") {
  return `relative inline-flex select-none items-center justify-center whitespace-nowrap rounded-[var(--radius-control)] font-medium leading-none transition-[background-color,border-color,color,box-shadow] duration-150 disabled:pointer-events-none disabled:opacity-55 aria-disabled:pointer-events-none aria-disabled:opacity-55 ${variantStyles[variant]} ${sizeStyles[size]} ${className}`;
}

type ButtonProps = Omit<ComponentProps<typeof BaseButton>, "className"> & {
  className?: string;
  variant?: Variant;
  size?: Size;
  pending?: boolean;
  icon?: ReactNode;
};

export function Button({
  variant = "primary",
  size = "md",
  pending,
  icon,
  className = "",
  children,
  disabled,
  ...props
}: ButtonProps) {
  return (
    <BaseButton
      className={buttonClass(variant, size, className)}
      disabled={disabled || pending}
      aria-busy={pending || undefined}
      {...props}
    >
      {pending ? <Spinner className="shrink-0" /> : icon ? <span className="-ml-0.5 shrink-0 opacity-90">{icon}</span> : null}
      {children}
    </BaseButton>
  );
}

export function Spinner({ className = "", size = 14 }: { className?: string; size?: number }) {
  return (
    <svg
      className={`animate-spin ${className}`}
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
    >
      <circle cx="8" cy="8" r="6.25" stroke="currentColor" strokeOpacity="0.25" strokeWidth="1.75" />
      <path d="M14.25 8A6.25 6.25 0 0 0 8 1.75" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" />
    </svg>
  );
}

/* ------------------------------------------------------------------ */
/* Fields                                                              */
/* ------------------------------------------------------------------ */

const inputBase =
  "h-10 w-full rounded-[var(--radius-control)] border border-line-strong bg-white px-3 text-[15px] text-ink shadow-[inset_0_1px_1px_rgb(28_25_23/0.03)] outline-none transition-[border-color,box-shadow] duration-150 placeholder:text-faint hover:border-[#c4beb3] focus:border-pine focus:shadow-[0_0_0_3px_rgb(31_61_50/0.12)] focus-visible:outline-none disabled:cursor-not-allowed disabled:bg-sunken disabled:text-muted data-[invalid]:border-danger data-[invalid]:focus:shadow-[0_0_0_3px_rgb(143_45_45/0.12)]";

export function TextField({
  label,
  labelAside,
  name,
  type = "text",
  autoComplete,
  value,
  onChange,
  error,
  hint,
  required,
  placeholder,
  autoFocus,
  inputMode,
  leading,
  disabled,
  mono,
  maxLength,
  className = "",
}: {
  label: string;
  labelAside?: ReactNode;
  name: string;
  type?: string;
  autoComplete?: string;
  value: string;
  onChange: (value: string) => void;
  error?: string;
  hint?: ReactNode;
  required?: boolean;
  placeholder?: string;
  autoFocus?: boolean;
  inputMode?: React.HTMLAttributes<HTMLInputElement>["inputMode"];
  leading?: ReactNode;
  disabled?: boolean;
  mono?: boolean;
  maxLength?: number;
  className?: string;
}) {
  const [revealed, setRevealed] = useState(false);
  const isPassword = type === "password";
  return (
    <Field.Root className={`grid gap-1.5 ${className}`} invalid={Boolean(error)} disabled={disabled}>
      <div className="flex items-baseline justify-between gap-3">
        <Field.Label className="text-[13px] font-medium text-ink-soft">{label}</Field.Label>
        {labelAside ? <span className="text-[13px]">{labelAside}</span> : null}
      </div>
      <div className="relative">
        {leading ? (
          <span className="pointer-events-none absolute inset-y-0 left-3 flex items-center text-faint">{leading}</span>
        ) : null}
        <BaseInput
          name={name}
          type={isPassword && revealed ? "text" : type}
          autoComplete={autoComplete}
          required={required}
          value={value}
          placeholder={placeholder}
          autoFocus={autoFocus}
          inputMode={inputMode}
          maxLength={maxLength}
          spellCheck={isPassword || mono ? false : undefined}
          onValueChange={(next) => onChange(next)}
          className={`${inputBase} ${leading ? "pl-9" : ""} ${isPassword ? "pr-10" : ""} ${mono ? "font-mono tracking-wide" : ""}`}
        />
        {isPassword ? (
          <button
            type="button"
            className="absolute inset-y-0 right-0 flex w-10 items-center justify-center rounded-r-[var(--radius-control)] text-faint transition-colors hover:text-ink"
            aria-label={revealed ? "Hide password" : "Show password"}
            aria-pressed={revealed}
            onClick={() => setRevealed((v) => !v)}
            tabIndex={-1}
          >
            {revealed ? <EyeOffIcon /> : <EyeIcon />}
          </button>
        ) : null}
      </div>
      {hint && !error ? <Field.Description className="text-[13px] text-muted">{hint}</Field.Description> : null}
      {error ? (
        <Field.Error match className="flex items-center gap-1.5 text-[13px] text-danger">
          <AlertIcon size={14} />
          {error}
        </Field.Error>
      ) : null}
    </Field.Root>
  );
}

/** Single one-time-code input, per the design system: not six boxes. */
export function CodeField({
  label,
  value,
  onChange,
  recovery,
  autoFocus,
  name = "code",
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  recovery?: boolean;
  autoFocus?: boolean;
  name?: string;
}) {
  const id = useId();
  return (
    <div className="grid gap-1.5">
      <label htmlFor={id} className="text-[13px] font-medium text-ink-soft">
        {label}
      </label>
      <input
        id={id}
        name={name}
        className={`${inputBase} h-12 text-center font-mono text-[20px] tracking-[0.35em] placeholder:tracking-[0.35em]`}
        inputMode={recovery ? "text" : "numeric"}
        autoComplete="one-time-code"
        autoCapitalize="off"
        spellCheck={false}
        placeholder={recovery ? "xxxx-xxxx" : "000000"}
        maxLength={recovery ? 32 : 8}
        autoFocus={autoFocus}
        value={value}
        onChange={(event) => onChange(recovery ? event.target.value : event.target.value.replace(/\D/g, ""))}
        required
      />
    </div>
  );
}

export function passwordScore(value: string) {
  if (!value) return 0;
  let score = 0;
  if (value.length >= 10) score++;
  if (value.length >= 14) score++;
  if (/[a-z]/.test(value) && /[A-Z]/.test(value)) score++;
  if (/\d/.test(value) && /[^A-Za-z0-9]/.test(value)) score++;
  if (value.length < 10) return Math.min(score, 1) || 1;
  return Math.max(2, Math.min(score + 1, 4));
}

export function PasswordStrength({ value }: { value: string }) {
  const score = passwordScore(value);
  const labels = ["", "Too short", "Fair", "Good", "Strong"];
  const colors = ["bg-line", "bg-danger", "bg-amber", "bg-pine/70", "bg-pine"];
  return (
    <div className="grid gap-1.5" aria-live="polite">
      <div className="grid grid-cols-4 gap-1">
        {[1, 2, 3, 4].map((step) => (
          <span
            key={step}
            className={`h-1 rounded-full transition-colors duration-200 ${score >= step ? colors[score] : "bg-line"}`}
          />
        ))}
      </div>
      <p className="text-[12.5px] text-muted">
        {value ? (
          <>
            <span className={score <= 1 ? "text-danger" : "text-ink-soft"}>{labels[score]}</span>
            {score <= 1 ? " · Use at least 10 characters." : null}
          </>
        ) : (
          "Use at least 10 characters. A short phrase works well."
        )}
      </p>
    </div>
  );
}

export function Switch({
  checked,
  onCheckedChange,
  disabled,
  label,
}: {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  label: string;
}) {
  return (
    <BaseSwitch.Root
      checked={checked}
      onCheckedChange={(next) => onCheckedChange(next)}
      disabled={disabled}
      aria-label={label}
      className="relative inline-flex h-[22px] w-[38px] shrink-0 cursor-pointer items-center rounded-full border border-line-strong bg-sunken p-[2px] transition-colors duration-150 data-[checked]:border-pine data-[checked]:bg-pine data-[disabled]:cursor-not-allowed data-[disabled]:opacity-55"
    >
      <BaseSwitch.Thumb className="block h-4 w-4 rounded-full bg-white shadow-[0_1px_2px_rgb(28_25_23/0.25)] transition-transform duration-150 data-[checked]:translate-x-4" />
    </BaseSwitch.Root>
  );
}

export function Checkbox({
  checked,
  onCheckedChange,
  children,
}: {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  children: ReactNode;
}) {
  return (
    <label className="flex cursor-pointer items-start gap-2.5 text-sm text-ink-soft">
      <BaseCheckbox.Root
        checked={checked}
        onCheckedChange={(next) => onCheckedChange(next)}
        className="mt-[2px] flex h-[18px] w-[18px] shrink-0 items-center justify-center rounded-[4px] border border-line-strong bg-white transition-colors data-[checked]:border-pine data-[checked]:bg-pine"
      >
        <BaseCheckbox.Indicator className="text-white">
          <CheckIcon size={13} strokeWidth={2.2} />
        </BaseCheckbox.Indicator>
      </BaseCheckbox.Root>
      <span>{children}</span>
    </label>
  );
}

/* ------------------------------------------------------------------ */
/* Feedback                                                            */
/* ------------------------------------------------------------------ */

type Tone = "error" | "success" | "info" | "warning";

const toneStyles: Record<Tone, string> = {
  error: "border-danger-line bg-danger-soft text-danger",
  success: "border-pine-line bg-pine-soft text-pine",
  info: "border-line bg-sunken/60 text-ink-soft",
  warning: "border-amber-line bg-amber-soft text-amber",
};

const toneIcons: Record<Tone, ReactNode> = {
  error: <AlertIcon />,
  success: <CheckCircleIcon />,
  info: <InfoIcon />,
  warning: <WarningIcon />,
};

export function Alert({
  children,
  tone = "error",
  title,
  className = "",
}: {
  children: ReactNode;
  tone?: Tone;
  title?: string;
  className?: string;
}) {
  return (
    <div
      role={tone === "error" ? "alert" : "status"}
      className={`flex animate-fade-up gap-2.5 rounded-[8px] border px-3.5 py-3 text-sm ${toneStyles[tone]} ${className}`}
    >
      <span className="mt-[2px] shrink-0">{toneIcons[tone]}</span>
      <div className="min-w-0">
        {title ? <p className="font-medium">{title}</p> : null}
        <div className={title ? "mt-0.5 opacity-90" : ""}>{children}</div>
      </div>
    </div>
  );
}

type BadgeTone = "neutral" | "success" | "danger" | "warning" | "outline";

export function Badge({ children, tone = "neutral", dot }: { children: ReactNode; tone?: BadgeTone; dot?: boolean }) {
  const styles: Record<BadgeTone, string> = {
    neutral: "bg-sunken text-ink-soft border-line",
    success: "bg-pine-soft text-pine border-pine-line",
    danger: "bg-danger-soft text-danger border-danger-line",
    warning: "bg-amber-soft text-amber border-amber-line",
    outline: "bg-transparent text-muted border-line-strong",
  };
  const dots: Record<BadgeTone, string> = {
    neutral: "bg-faint",
    success: "bg-pine",
    danger: "bg-danger",
    warning: "bg-amber",
    outline: "bg-faint",
  };
  return (
    <span
      className={`inline-flex h-[22px] items-center gap-1.5 whitespace-nowrap rounded-full border px-2 text-[12px] font-medium leading-none ${styles[tone]}`}
    >
      {dot ? <span className={`h-1.5 w-1.5 rounded-full ${dots[tone]}`} /> : null}
      {children}
    </span>
  );
}

export function Skeleton({ className = "" }: { className?: string }) {
  return <span aria-hidden="true" className={`block animate-shimmer rounded-[6px] bg-line/70 ${className}`} />;
}

export function PageSkeleton({ label }: { label: string }) {
  return (
    <div role="status" aria-live="polite" className="grid gap-6">
      <span className="sr-only">{label}</span>
      <div className="grid gap-2">
        <Skeleton className="h-7 w-48" />
        <Skeleton className="h-4 w-72 max-w-full" />
      </div>
      {[0, 1].map((key) => (
        <div key={key} className="rounded-[var(--radius-card)] border border-line bg-surface p-5 shadow-card">
          <Skeleton className="h-4 w-36" />
          <Skeleton className="mt-2 h-3.5 w-64 max-w-full" />
          <div className="mt-5 grid gap-3">
            <Skeleton className="h-10 w-full" />
            <Skeleton className="h-10 w-2/3" />
          </div>
        </div>
      ))}
    </div>
  );
}

export function EmptyState({ icon, title, children }: { icon?: ReactNode; title: string; children?: ReactNode }) {
  return (
    <div className="flex flex-col items-center px-6 py-10 text-center">
      {icon ? (
        <span className="mb-3 flex h-10 w-10 items-center justify-center rounded-full border border-line bg-sunken text-muted">
          {icon}
        </span>
      ) : null}
      <p className="text-sm font-medium text-ink">{title}</p>
      {children ? <p className="mt-1 max-w-sm text-sm text-muted">{children}</p> : null}
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Layout                                                              */
/* ------------------------------------------------------------------ */

export function PageTitle({ title, detail, actions }: { title: string; detail?: ReactNode; actions?: ReactNode }) {
  return (
    <header className="mb-7 flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
      <div className="min-w-0">
        <h1 className="text-[24px] font-semibold leading-tight tracking-[-0.015em] text-ink">{title}</h1>
        {detail ? <p className="mt-1.5 max-w-2xl text-[14.5px] text-muted">{detail}</p> : null}
      </div>
      {actions ? <div className="flex shrink-0 flex-wrap gap-2">{actions}</div> : null}
    </header>
  );
}

export function Card({ children, className = "", tone }: { children: ReactNode; className?: string; tone?: "danger" }) {
  return (
    <section
      className={`overflow-hidden rounded-[var(--radius-card)] border bg-surface shadow-card ${tone === "danger" ? "border-danger-line" : "border-line"} ${className}`}
    >
      {children}
    </section>
  );
}

export function CardHeader({
  title,
  description,
  action,
  icon,
}: {
  title: string;
  description?: ReactNode;
  action?: ReactNode;
  icon?: ReactNode;
}) {
  return (
    <div className="flex items-start justify-between gap-4 px-5 pt-5 sm:px-6">
      <div className="flex min-w-0 gap-3">
        {icon ? (
          <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-[8px] border border-line bg-paper text-ink-soft">
            {icon}
          </span>
        ) : null}
        <div className="min-w-0">
          <h2 className="text-[15px] font-semibold tracking-[-0.005em] text-ink">{title}</h2>
          {description ? <p className="mt-1 text-sm text-muted">{description}</p> : null}
        </div>
      </div>
      {action ? <div className="shrink-0">{action}</div> : null}
    </div>
  );
}

export function CardBody({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <div className={`px-5 py-5 sm:px-6 ${className}`}>{children}</div>;
}

export function CardFooter({ children, note }: { children?: ReactNode; note?: ReactNode }) {
  return (
    <div className="flex flex-col gap-3 border-t border-line bg-paper/70 px-5 py-3 sm:flex-row sm:items-center sm:justify-between sm:px-6">
      <p className="text-[13px] text-muted">{note}</p>
      {children ? <div className="flex flex-wrap justify-end gap-2">{children}</div> : null}
    </div>
  );
}

export function Row({
  icon,
  title,
  description,
  aside,
  children,
}: {
  icon?: ReactNode;
  title: ReactNode;
  description?: ReactNode;
  aside?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="px-5 py-4 sm:px-6">
      <div className="flex items-center justify-between gap-4">
        <div className="flex min-w-0 items-start gap-3">
          {icon ? (
            <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-[8px] border border-line bg-paper text-ink-soft">
              {icon}
            </span>
          ) : null}
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2 text-sm font-medium text-ink">{title}</div>
            {description ? <div className="mt-0.5 text-[13px] text-muted">{description}</div> : null}
          </div>
        </div>
        {aside ? <div className="flex shrink-0 items-center gap-2">{aside}</div> : null}
      </div>
      {children}
    </div>
  );
}

export function Avatar({ name, size = 32 }: { name: string; size?: number }) {
  const initials =
    name
      .replace(/@.*/, "")
      .split(/[\s._-]+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((part) => part[0]?.toUpperCase())
      .join("") || "?";
  return (
    <span
      aria-hidden="true"
      style={{ width: size, height: size, fontSize: Math.round(size * 0.38) }}
      className="inline-flex shrink-0 items-center justify-center rounded-full border border-pine-line bg-pine-soft font-semibold tracking-tight text-pine"
    >
      {initials}
    </span>
  );
}

/* ------------------------------------------------------------------ */
/* Dialogs & utilities                                                 */
/* ------------------------------------------------------------------ */

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel,
  pendingLabel,
  danger,
  onConfirm,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: ReactNode;
  confirmLabel: string;
  pendingLabel?: string;
  danger?: boolean;
  onConfirm: () => Promise<void> | void;
  children?: ReactNode;
}) {
  const [pending, setPending] = useState(false);
  useEffect(() => {
    if (!open) setPending(false);
  }, [open]);
  return (
    <AlertDialog.Root open={open} onOpenChange={(next) => onOpenChange(next)}>
      <AlertDialog.Portal>
        <AlertDialog.Backdrop className="fixed inset-0 z-40 bg-[rgb(28_25_23/0.32)] transition-opacity duration-150 data-[ending-style]:opacity-0 data-[starting-style]:opacity-0" />
        <AlertDialog.Popup className="fixed left-1/2 top-1/2 z-50 w-[calc(100vw-32px)] max-w-[420px] -translate-x-1/2 -translate-y-1/2 rounded-[12px] border border-line bg-surface shadow-overlay outline-none transition-[opacity,transform] duration-150 data-[ending-style]:scale-[0.98] data-[ending-style]:opacity-0 data-[starting-style]:scale-[0.98] data-[starting-style]:opacity-0">
          <div className="px-6 pt-6">
            <AlertDialog.Title className="text-[16px] font-semibold tracking-tight text-ink">{title}</AlertDialog.Title>
            <AlertDialog.Description className="mt-2 text-sm text-muted">{description}</AlertDialog.Description>
            {children ? <div className="mt-4">{children}</div> : null}
          </div>
          <div className="mt-6 flex flex-col-reverse gap-2 border-t border-line bg-paper/70 px-6 py-4 sm:flex-row sm:justify-end">
            <AlertDialog.Close className={buttonClass("secondary", "md")} disabled={pending}>
              Cancel
            </AlertDialog.Close>
            <Button
              type="button"
              variant={danger ? "danger-solid" : "primary"}
              pending={pending}
              onClick={async () => {
                setPending(true);
                try {
                  await onConfirm();
                } finally {
                  setPending(false);
                }
              }}
            >
              {pending && pendingLabel ? pendingLabel : confirmLabel}
            </Button>
          </div>
        </AlertDialog.Popup>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

export function CopyButton({ value, label = "Copy" }: { value: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 1600);
    return () => window.clearTimeout(timer);
  }, [copied]);
  return (
    <Button
      type="button"
      variant="secondary"
      size="sm"
      icon={copied ? <CheckIcon size={14} /> : <CopyIcon size={14} />}
      onClick={() => {
        void navigator.clipboard?.writeText(value).then(() => setCopied(true));
      }}
    >
      {copied ? "Copied" : label}
    </Button>
  );
}

export function useCountdown() {
  const [seconds, setSeconds] = useState(0);
  useEffect(() => {
    if (seconds <= 0) return;
    const timer = window.setTimeout(() => setSeconds((value) => value - 1), 1000);
    return () => window.clearTimeout(timer);
  }, [seconds]);
  return [seconds, setSeconds] as const;
}
