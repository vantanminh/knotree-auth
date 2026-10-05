import { t } from "../lib/i18n";
import { useRef, useState, type DragEvent, type ReactNode } from "react";
import { Link, useNavigate } from "react-router";
import { ArrowLeftIcon, KeyIcon, WarningIcon } from "../components/icons";
import {
  Alert,
  Button,
  Card,
  CardHeader,
  Checkbox,
  ClientLogo,
  ConfirmDialog,
  CopyButton,
  PageTitle,
  Switch,
  TextField,
} from "../components/ui";
import { ApiError, api } from "../lib/api";

export type ServiceProfile = {
  id: string;
  name: string;
  client_type: "public" | "confidential" | "service";
  status: string;
  first_party: boolean;
  require_pkce: boolean;
  allowed_scopes: string[];
  redirect_uris: string[];
  has_secret: boolean;
  description: string | null;
  homepage_url: string | null;
  logo_url: string | null;
};

const STANDARD_SCOPES = ["openid", "profile", "email", "offline_access"];

type Form = {
  id: string;
  name: string;
  description: string;
  homepage_url: string;
  client_type: ServiceProfile["client_type"];
  redirect_uris: string;
  scopes: string[];
  extra_scopes: string;
  first_party: boolean;
  require_pkce: boolean;
  status: string;
};

function toForm(client?: ServiceProfile): Form {
  const scopes = client?.allowed_scopes ?? ["openid", "profile", "email"];
  return {
    id: client?.id ?? "",
    name: client?.name ?? "",
    description: client?.description ?? "",
    homepage_url: client?.homepage_url ?? "",
    client_type: client?.client_type ?? "confidential",
    redirect_uris: (client?.redirect_uris ?? []).join("\n"),
    scopes: scopes.filter((scope) => STANDARD_SCOPES.includes(scope)),
    extra_scopes: scopes.filter((scope) => !STANDARD_SCOPES.includes(scope)).join(" "),
    first_party: client?.first_party ?? false,
    require_pkce: client?.require_pkce ?? true,
    status: client?.status ?? "active",
  };
}

function toBody(form: Form) {
  return {
    name: form.name,
    description: form.description,
    homepage_url: form.homepage_url,
    redirect_uris: form.redirect_uris.split(/\s+/).filter(Boolean),
    allowed_scopes: [...form.scopes, ...form.extra_scopes.split(/[\s,]+/).filter(Boolean)],
    first_party: form.first_party,
    require_pkce: form.require_pkce,
  };
}

function FieldBlock({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <label className="grid gap-1.5">
      <span className="text-[13.5px] font-medium text-ink">{label}</span>
      {children}
      {hint ? <span className="text-[12.5px] text-muted">{hint}</span> : null}
    </label>
  );
}

const textareaClass =
  "w-full rounded-[var(--radius-control)] border border-line-strong bg-white px-3 py-2 text-[14.5px] text-ink outline-none transition-[border-color,box-shadow] duration-150 placeholder:text-faint hover:border-[#c4beb3] focus:border-pine focus:shadow-[0_0_0_3px_rgb(31_61_50/0.12)]";

/** Create and edit form shared by the new-service page and the service page. */
function ServiceForm({
  initial,
  creating,
  onSaved,
}: {
  initial?: ServiceProfile;
  creating?: boolean;
  onSaved: (client: ServiceProfile & { client_secret?: string | null }) => void;
}) {
  const [form, setForm] = useState<Form>(() => toForm(initial));
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState(false);
  const set = <K extends keyof Form>(key: K, value: Form[K]) => {
    setSaved(false);
    setForm((current) => ({ ...current, [key]: value }));
  };

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setPending(true);
    setError("");
    try {
      const body = creating
        ? { ...toBody(form), id: form.id, client_type: form.client_type }
        : { ...toBody(form), status: form.status };
      const result = await api<ServiceProfile & { client_secret?: string | null }>(
        creating ? "/api/v1/admin/clients" : `/api/v1/admin/clients/${encodeURIComponent(form.id)}`,
        { method: creating ? "POST" : "PATCH", body: JSON.stringify(body) },
      );
      setSaved(true);
      onSaved(result);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : t("Could not save this service."));
    } finally {
      setPending(false);
    }
  }

  return (
    <form className="grid gap-5 p-5 sm:p-6" onSubmit={submit} aria-busy={pending || undefined}>
      {error ? <Alert>{error}</Alert> : null}
      {saved && !creating ? <Alert tone="success">{t("Changes saved.")}</Alert> : null}
      <div className="grid gap-5 sm:grid-cols-2">
        {creating ? (
          <TextField
            label={t("Service ID")}
            name="id"
            value={form.id}
            onChange={(value) => set("id", value.toLowerCase())}
            hint={t("Lowercase letters, numbers and hyphens. Used as client_id and cannot change.")}
            required
            mono
            maxLength={64}
          />
        ) : null}
        <TextField label={t("Name")} name="name" value={form.name} onChange={(value) => set("name", value)} required maxLength={80} />
      </div>
      <FieldBlock label={t("Description")} hint={t("Shown to users on the authorization screen. {count}/1000", { count: form.description.length })}>
        <textarea
          className={textareaClass}
          rows={3}
          maxLength={1000}
          value={form.description}
          onChange={(event) => set("description", event.target.value)}
          placeholder={t("What this service does and why it needs a Knotree account.")}
        />
      </FieldBlock>
      <TextField
        label={t("Homepage")}
        name="homepage_url"
        type="url"
        value={form.homepage_url}
        onChange={(value) => set("homepage_url", value)}
        placeholder="https://"
      />
      {creating ? (
        <FieldBlock label={t("Type")}>
          <select className={`${textareaClass} h-10 py-0`} value={form.client_type} onChange={(event) => set("client_type", event.target.value as Form["client_type"])}>
            <option value="confidential">{t("Confidential")} — {t("server app with a secret")}</option>
            <option value="public">{t("Public")} — {t("browser or mobile app, PKCE only")}</option>
            <option value="service">{t("Service")} — {t("machine to machine")}</option>
          </select>
        </FieldBlock>
      ) : null}
      <FieldBlock label={t("Redirect URIs")} hint={t("One per line. HTTPS only, except http://localhost.")}>
        <textarea
          className={`${textareaClass} font-mono text-[13px]`}
          rows={3}
          value={form.redirect_uris}
          onChange={(event) => set("redirect_uris", event.target.value)}
          placeholder="https://app.example.com/auth/callback"
        />
      </FieldBlock>
      <fieldset className="grid gap-2.5">
        <legend className="mb-1 text-[13.5px] font-medium text-ink">{t("Allowed scopes")}</legend>
        <div className="grid gap-2 sm:grid-cols-2">
          {STANDARD_SCOPES.map((scope) => (
            <Checkbox
              key={scope}
              checked={form.scopes.includes(scope) || scope === "openid"}
              onCheckedChange={(checked) =>
                scope !== "openid" &&
                set("scopes", checked ? [...form.scopes, scope] : form.scopes.filter((item) => item !== scope))
              }
            >
              <code className="font-mono text-[13px]">{scope}</code>
            </Checkbox>
          ))}
        </div>
        <TextField
          label={t("Custom scopes")}
          name="extra_scopes"
          value={form.extra_scopes}
          onChange={(value) => set("extra_scopes", value)}
          placeholder="study:read study:write"
          mono
        />
      </fieldset>
      <div className="grid gap-3 rounded-[var(--radius-control)] border border-line bg-paper/60 p-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <p className="text-[14px] font-medium text-ink">{t("Knotree first-party service")}</p>
            <p className="mt-0.5 text-[12.5px] text-muted">
              {form.first_party
                ? t("Users sign in directly without an authorization screen.")
                : t("Users see an authorization screen, like “Sign in with Google”, before the service gets access.")}
            </p>
          </div>
          <Switch checked={form.first_party} onCheckedChange={(value) => set("first_party", value)} label={t("Knotree first-party service")} />
        </div>
        {form.client_type !== "public" ? (
          <div className="flex items-start justify-between gap-4 border-t border-line pt-3">
            <p className="text-[14px] font-medium text-ink">{t("Require PKCE")}</p>
            <Switch checked={form.require_pkce} onCheckedChange={(value) => set("require_pkce", value)} label={t("Require PKCE")} />
          </div>
        ) : null}
        {!creating ? (
          <div className="flex items-start justify-between gap-4 border-t border-line pt-3">
            <div>
              <p className="text-[14px] font-medium text-ink">{t("Active")}</p>
              <p className="mt-0.5 text-[12.5px] text-muted">{t("Disabling signs users out of this service and blocks new sign-ins.")}</p>
            </div>
            <Switch checked={form.status === "active"} onCheckedChange={(value) => set("status", value ? "active" : "disabled")} label={t("Active")} />
          </div>
        ) : null}
      </div>
      <div className="flex justify-end">
        <Button type="submit" pending={pending}>
          {creating ? t("Create service") : t("Save changes")}
        </Button>
      </div>
    </form>
  );
}

/** Reads an image file's dimensions in the browser so non-square logos are caught before upload. */
function imageSize(file: File): Promise<{ width: number; height: number }> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const image = new Image();
    image.onload = () => {
      resolve({ width: image.naturalWidth, height: image.naturalHeight });
      URL.revokeObjectURL(url);
    };
    image.onerror = () => {
      reject(new Error("decode"));
      URL.revokeObjectURL(url);
    };
    image.src = url;
  });
}

function LogoCard({ client, onChange }: { client: ServiceProfile; onChange: (logo: string | null) => void }) {
  const input = useRef<HTMLInputElement>(null);
  const [pending, setPending] = useState<"upload" | "remove" | null>(null);
  const [preview, setPreview] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [dragging, setDragging] = useState(false);

  async function upload(file: File) {
    setError("");
    if (!["image/png", "image/jpeg", "image/webp"].includes(file.type)) {
      setError(t("Upload a PNG, JPEG or WebP image."));
      return;
    }
    if (file.size > 1024 * 1024) {
      setError(t("The logo must be 1 MB or smaller."));
      return;
    }
    try {
      const { width, height } = await imageSize(file);
      const side = Math.min(width, height);
      if (side < 64) return setError(t("The logo must be at least 64×64 pixels."));
      if (Math.max(width, height) - side > side / 50) return setError(t("The logo must be a square image."));
    } catch {
      return setError(t("Upload a PNG, JPEG or WebP image."));
    }
    const localUrl = URL.createObjectURL(file);
    setPreview(localUrl);
    setPending("upload");
    try {
      const body = new FormData();
      body.append("logo", file);
      const result = await api<{ logo_url: string }>(`/api/v1/admin/clients/${encodeURIComponent(client.id)}/logo`, {
        method: "PUT",
        body,
      });
      onChange(result.logo_url);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : t("Could not upload the logo."));
    } finally {
      setPreview(null);
      URL.revokeObjectURL(localUrl);
      setPending(null);
    }
  }

  async function remove() {
    setPending("remove");
    setError("");
    try {
      await api(`/api/v1/admin/clients/${encodeURIComponent(client.id)}/logo`, { method: "DELETE" });
      onChange(null);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : t("Could not remove the logo."));
    } finally {
      setPending(null);
    }
  }

  function onDrop(event: DragEvent) {
    event.preventDefault();
    setDragging(false);
    const file = event.dataTransfer.files[0];
    if (file) void upload(file);
  }

  return (
    <Card>
      <CardHeader title={t("Logo")} description={t("A square PNG, JPEG or WebP image, at least 64×64 and up to 1 MB. Shown with rounded corners.")} />
      <div className="mt-4 grid gap-4 border-t border-line p-5 sm:p-6">
        {error ? <Alert>{error}</Alert> : null}
        <div
          onDragOver={(event) => {
            event.preventDefault();
            setDragging(true);
          }}
          onDragLeave={() => setDragging(false)}
          onDrop={onDrop}
          className={`flex flex-col items-center gap-4 rounded-[var(--radius-card)] border border-dashed p-6 transition-colors sm:flex-row ${
            dragging ? "border-pine bg-pine-soft" : "border-line-strong bg-paper/50"
          }`}
        >
          <div className={`relative transition-opacity ${pending === "upload" ? "opacity-70" : ""}`}>
            <ClientLogo name={client.name} src={preview ?? client.logo_url} size={88} />
          </div>
          <div className="flex flex-1 flex-col items-center gap-2 text-center sm:items-start sm:text-left">
            <p className="text-[13.5px] text-ink-soft">{t("Drag an image here, or choose a file.")}</p>
            <div className="flex flex-wrap gap-2">
              <Button type="button" variant="secondary" size="sm" pending={pending === "upload"} disabled={pending !== null} onClick={() => input.current?.click()}>
                {client.logo_url ? t("Replace logo") : t("Upload logo")}
              </Button>
              {client.logo_url ? (
                <Button type="button" variant="ghost" size="sm" pending={pending === "remove"} disabled={pending !== null} onClick={() => void remove()}>
                  {t("Remove")}
                </Button>
              ) : null}
            </div>
          </div>
          <input
            ref={input}
            type="file"
            accept="image/png,image/jpeg,image/webp"
            className="sr-only"
            onChange={(event) => {
              const file = event.target.files?.[0];
              event.target.value = "";
              if (file) void upload(file);
            }}
          />
        </div>
      </div>
    </Card>
  );
}

function SecretNotice({ secret }: { secret: string }) {
  return (
    <Alert tone="warning">
      <div className="grid gap-2">
        <span>{t("Copy the client secret now. It will not be shown again.")}</span>
        <span className="flex flex-wrap items-center gap-2">
          <code className="break-all rounded-[6px] bg-white/70 px-2 py-1 font-mono text-[13px] text-ink">{secret}</code>
          <CopyButton value={secret} />
        </span>
      </div>
    </Alert>
  );
}

/** Profile, logo and secret management shown on a service's admin page. */
export function ServiceManagement({ client, onChange }: { client: ServiceProfile; onChange: (client: ServiceProfile) => void }) {
  const [secret, setSecret] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState("");

  async function rotate() {
    setError("");
    try {
      const result = await api<{ client_secret: string }>(`/api/v1/admin/clients/${encodeURIComponent(client.id)}/secret`, {
        method: "POST",
        body: "{}",
      });
      setSecret(result.client_secret);
      onChange({ ...client, has_secret: true });
      setConfirm(false);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : t("Could not rotate the secret."));
      setConfirm(false);
    }
  }

  return (
    <>
      <LogoCard client={client} onChange={(logo_url) => onChange({ ...client, logo_url })} />
      <Card>
        <CardHeader title={t("Edit service")} description={t("Name, description and sign-in settings.")} />
        <div className="mt-4 border-t border-line">
          <ServiceForm key={client.id} initial={client} onSaved={(next) => onChange({ ...client, ...next })} />
        </div>
      </Card>
      {client.client_type !== "public" ? (
        <Card>
          <CardHeader
            title={t("Client secret")}
            description={t("Rotating creates a new secret immediately. The old secret stops working.")}
            action={
              <Button type="button" variant="danger" size="sm" icon={<KeyIcon size={14} />} onClick={() => setConfirm(true)}>
                {t("Rotate secret")}
              </Button>
            }
          />
          {secret || error ? (
            <div className="mt-4 border-t border-line p-5 sm:p-6">{error ? <Alert>{error}</Alert> : secret ? <SecretNotice secret={secret} /> : null}</div>
          ) : (
            <div className="pb-5" />
          )}
        </Card>
      ) : null}
      <ConfirmDialog
        open={confirm}
        onOpenChange={setConfirm}
        title={t("Rotate client secret?")}
        description={t("The service must be updated with the new secret, or its sign-ins will fail.")}
        confirmLabel={t("Rotate secret")}
        pendingLabel={t("Rotating…")}
        danger
        onConfirm={rotate}
      />
    </>
  );
}

export function AdminClientNew() {
  const navigate = useNavigate();
  const [created, setCreated] = useState<(ServiceProfile & { client_secret?: string | null }) | null>(null);

  return (
    <div className="grid gap-6">
      <Link to="/admin/clients" className="inline-flex w-fit items-center gap-1.5 text-[13px] font-medium text-muted transition-colors hover:text-ink">
        <ArrowLeftIcon size={14} />
        {t("All services")}
      </Link>
      <PageTitle title={t("New service")} detail={t("Register an application that signs users in with Knotree.")} />
      {created ? (
        <Card>
          <div className="grid gap-4 p-5 sm:p-6">
            <div className="flex items-center gap-3">
              <ClientLogo name={created.name} src={created.logo_url} size={48} />
              <div>
                <p className="font-medium text-ink">{created.name}</p>
                <code className="font-mono text-[12.5px] text-muted">{created.id}</code>
              </div>
            </div>
            {created.client_secret ? <SecretNotice secret={created.client_secret} /> : null}
            <div className="flex justify-end">
              <Button type="button" onClick={() => navigate(`/admin/clients/${encodeURIComponent(created.id)}`)}>
                {t("Continue to add a logo")}
              </Button>
            </div>
          </div>
        </Card>
      ) : (
        <Card>
          <Alert tone="info" className="m-5 mb-0 sm:m-6 sm:mb-0">
            <span className="flex items-start gap-2">
              <WarningIcon size={15} className="mt-[3px] shrink-0" />
              {t("New services are third-party by default: users approve access on an authorization screen.")}
            </span>
          </Alert>
          <ServiceForm creating onSaved={setCreated} />
        </Card>
      )}
    </div>
  );
}
