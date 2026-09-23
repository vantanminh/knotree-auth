import { Button as BaseButton } from "@base-ui/react/button";
import { Field } from "@base-ui/react/field";
import { Input as BaseInput } from "@base-ui/react/input";
import type { ComponentProps, ReactNode } from "react";

type ButtonProps = ComponentProps<typeof BaseButton> & {
  variant?: "primary" | "quiet" | "danger";
  pending?: boolean;
};

export function Button({ variant = "primary", pending, className = "", children, disabled, ...props }: ButtonProps) {
  const styles = {
    primary: "bg-pine text-white hover:bg-[#183228]",
    quiet: "bg-transparent text-ink border border-line hover:bg-white",
    danger: "bg-transparent text-danger border border-danger/30 hover:bg-danger/5",
  }[variant];
  return (
    <BaseButton
      className={`inline-flex h-10 items-center justify-center rounded-[6px] px-3 text-[15px] font-medium transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-60 ${styles} ${className}`}
      disabled={disabled || pending}
      {...props}
    >
      {children}
    </BaseButton>
  );
}

export function TextField({
  label,
  name,
  type = "text",
  autoComplete,
  value,
  onChange,
  error,
  hint,
  required,
}: {
  label: string;
  name: string;
  type?: string;
  autoComplete?: string;
  value: string;
  onChange: (value: string) => void;
  error?: string;
  hint?: string;
  required?: boolean;
}) {
  return (
    <Field.Root className="grid gap-1.5" invalid={Boolean(error)}>
      <Field.Label className="text-sm text-ink">
        {label}
      </Field.Label>
      <BaseInput
        name={name}
        type={type}
        autoComplete={autoComplete}
        required={required}
        value={value}
        onValueChange={(next) => onChange(next)}
        className="h-10 w-full rounded-[6px] border border-line bg-surface px-3 text-[15px] text-ink outline-none placeholder:text-muted"
      />
      {hint ? <Field.Description className="text-sm text-muted">{hint}</Field.Description> : null}
      {error ? <Field.Error className="text-sm text-danger">{error}</Field.Error> : null}
    </Field.Root>
  );
}

export function Alert({ children }: { children: ReactNode }) {
  return (
    <div role="alert" className="rounded-[6px] border border-danger/25 bg-[#f8f1ef] px-3 py-2 text-sm text-danger">
      {children}
    </div>
  );
}

export function PageTitle({ title, detail }: { title: string; detail?: string }) {
  return (
    <header className="mb-6">
      <h1 className="text-[22px] font-medium tracking-tight">{title}</h1>
      {detail ? <p className="mt-1 text-sm text-muted">{detail}</p> : null}
    </header>
  );
}
