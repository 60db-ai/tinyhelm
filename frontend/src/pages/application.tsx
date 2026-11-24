import { useEffect, useState, type ReactNode } from "react";
import { ArrowLeft, GitBranch, Globe, Box, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { api, ApiError } from "@/lib/api";

// Local state helpers (same small set used by the other ported pages).
function ErrorBox({ message }: { message: string }) {
  return (
    <div className="rounded-[var(--radius)] border border-[var(--color-destructive)]/40 bg-[var(--color-destructive)]/10 px-4 py-3 text-sm">
      {message}
    </div>
  );
}
function EmptyBox({ message }: { message: string }) {
  return (
    <div className="rounded-[var(--radius)] border border-dashed py-16 text-center text-sm text-[var(--color-muted-foreground)]">
      {message}
    </div>
  );
}
function Skeleton() {
  return <div className="h-40 animate-pulse rounded-[var(--radius)] border bg-[var(--color-card)]" />;
}
function statusColor(status?: string | null): string {
  if (!status) return "var(--color-muted-foreground)";
  if (status.startsWith("running")) return "var(--color-success)";
  if (status.startsWith("exited") || status.includes("unhealthy")) return "var(--color-destructive)";
  return "var(--color-muted-foreground)";
}

interface Application {
  uuid: string;
  name: string;
  description?: string | null;
  status?: string | null;
  fqdn?: string | null;
  git_repository?: string | null;
  git_branch?: string | null;
  build_pack?: string | null;
}
interface EnvVar {
  uuid?: string;
  key: string;
  value?: string | null;
  is_build_time?: boolean;
}
interface Deployment {
  id?: number;
  deployment_uuid?: string;
  status: string;
  commit?: string | null;
  created_at?: string | null;
}

const TABS = ["overview", "environment", "deployments", "logs"] as const;
type Tab = (typeof TABS)[number];

export function ApplicationDetail({ uuid, onBack, backLabel = "Back" }: { uuid: string; onBack: () => void; backLabel?: string }) {
  const [app, setApp] = useState<Application | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("overview");

  useEffect(() => {
    api
      .get<Application>(`/applications/${uuid}`)
      .then(setApp)
      .catch((e: unknown) => setErr(e instanceof ApiError ? e.message : "Failed to load application"));
  }, [uuid]);

  return (
    <div className="mx-auto max-w-5xl space-y-6">
      <button
        onClick={onBack}
        className="flex items-center gap-2 text-sm text-[var(--color-muted-foreground)] transition-colors hover:text-[var(--color-foreground)]"
      >
        <ArrowLeft size={16} /> {backLabel}
      </button>

      {err && <ErrorBox message={err} />}
      {!err && !app && <Skeleton />}

      {app && (
        <>
          <div className="flex items-center gap-3">
            <span className="inline-block h-2.5 w-2.5 rounded-full" style={{ background: statusColor(app.status) }} />
            <h1 className="text-2xl font-semibold tracking-tight">{app.name}</h1>
            <span className="text-sm text-[var(--color-muted-foreground)]">{app.status ?? "unknown"}</span>
          </div>

          <div className="flex gap-1 border-b">
            {TABS.map((t) => (
              <button
                key={t}
                onClick={() => setTab(t)}
                className={
                  "relative px-3 py-2 text-sm capitalize transition-colors " +
                  (tab === t
                    ? "text-[var(--color-foreground)] after:absolute after:inset-x-3 after:-bottom-px after:h-0.5 after:bg-[var(--color-accent)]"
                    : "text-[var(--color-muted-foreground)] hover:text-[var(--color-foreground)]")
                }
              >
                {t}
              </button>
            ))}
          </div>

          {tab === "overview" && <Overview app={app} />}
          {tab === "environment" && <EnvTab uuid={uuid} />}
          {tab === "deployments" && <DeploymentsTab uuid={uuid} />}
          {tab === "logs" && <LogsTab uuid={uuid} />}
        </>
      )}
    </div>
  );
}

function Overview({ app }: { app: Application }) {
  return (
    <section className="grid grid-cols-1 gap-4 sm:grid-cols-2">
      <Meta icon={<Globe size={15} />} label="Domains" value={app.fqdn || "—"} />
      <Meta icon={<Box size={15} />} label="Build pack" value={app.build_pack || "—"} />
      <Meta icon={<GitBranch size={15} />} label="Repository" value={app.git_repository || "—"} />
      <Meta icon={<GitBranch size={15} />} label="Branch" value={app.git_branch || "—"} />
      {app.description && <Meta label="Description" value={app.description} full />}
    </section>
  );
}

function Meta({ icon, label, value, full }: { icon?: ReactNode; label: string; value: string; full?: boolean }) {
  return (
    <div className={"rounded-[var(--radius)] border bg-[var(--color-card)] p-4" + (full ? " sm:col-span-2" : "")}>
      <div className="flex items-center gap-1.5 text-xs text-[var(--color-muted-foreground)]">
        {icon} {label}
      </div>
      <div className="mt-1 break-all font-medium">{value}</div>
    </div>
  );
}

function EnvTab({ uuid }: { uuid: string }) {
  const [rows, setRows] = useState<EnvVar[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    api
      .get<EnvVar[]>(`/applications/${uuid}/envs`)
      .then(setRows)
      .catch((e: unknown) => setErr(e instanceof ApiError ? e.message : "Failed to load variables"));
  }, [uuid]);

  if (err) return <ErrorBox message={err} />;
  if (rows === null) return <Skeleton />;
  if (rows.length === 0) return <EmptyBox message="No environment variables." />;
  return (
    <div className="divide-y overflow-hidden rounded-[var(--radius)] border bg-[var(--color-card)] font-mono text-sm">
      {rows.map((e) => (
        <div key={e.uuid ?? e.key} className="flex items-start gap-3 px-4 py-2.5">
          <span className="shrink-0 font-medium text-[var(--color-accent)]">{e.key}</span>
          <span className="text-[var(--color-muted-foreground)]">=</span>
          <span className="min-w-0 flex-1 break-all text-[var(--color-muted-foreground)]">{e.value ?? ""}</span>
          {e.is_build_time && (
            <span className="shrink-0 rounded bg-[var(--color-muted)] px-1.5 py-0.5 text-[10px] uppercase">build</span>
          )}
        </div>
      ))}
    </div>
  );
}

function DeploymentsTab({ uuid }: { uuid: string }) {
  const [rows, setRows] = useState<Deployment[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    api
      .get<{ deployments?: Deployment[] }>(`/deployments/applications/${uuid}`)
      .then((d) => setRows(d.deployments ?? []))
      .catch((e: unknown) => setErr(e instanceof ApiError ? e.message : "Failed to load deployments"));
  }, [uuid]);

  if (err) return <ErrorBox message={err} />;
  if (rows === null) return <Skeleton />;
  if (rows.length === 0) return <EmptyBox message="No deployments yet." />;
  return (
    <div className="divide-y overflow-hidden rounded-[var(--radius)] border bg-[var(--color-card)]">
      {rows.map((d, i) => (
        <div key={d.deployment_uuid ?? d.id ?? i} className="flex items-center gap-4 px-5 py-3">
          <span className="inline-block h-2 w-2 shrink-0 rounded-full" style={{ background: deployColor(d.status) }} />
          <span className="w-28 shrink-0 text-sm capitalize">{d.status}</span>
          <span className="min-w-0 flex-1 truncate font-mono text-sm text-[var(--color-muted-foreground)]">
            {d.commit ? d.commit.slice(0, 8) : "—"}
          </span>
          <span className="hidden text-sm text-[var(--color-muted-foreground)] sm:block">{fmtDate(d.created_at)}</span>
        </div>
      ))}
    </div>
  );
}

function LogsTab({ uuid }: { uuid: string }) {
  const [logs, setLogs] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [nonce, setNonce] = useState(0);
  useEffect(() => {
    setLogs(null);
    setErr(null);
    api
      .get<{ logs?: string; message?: string }>(`/applications/${uuid}/logs`)
      .then((d) => setLogs(d.logs ?? d.message ?? ""))
      .catch((e: unknown) =>
        setErr(e instanceof ApiError ? (e.status === 403 ? "Needs a token with read:sensitive." : e.message) : "Failed to load logs"),
      );
  }, [uuid, nonce]);

  return (
    <div className="space-y-2">
      <div className="flex justify-end">
        <Button variant="outline" size="sm" onClick={() => setNonce((n) => n + 1)}>
          <RefreshCw size={15} /> Refresh
        </Button>
      </div>
      {err && <ErrorBox message={err} />}
      {!err && logs === null && <Skeleton />}
      {!err && logs !== null && (
        <pre className="max-h-[480px] overflow-auto rounded-[var(--radius)] border bg-black/40 p-4 font-mono text-xs leading-relaxed text-[var(--color-muted-foreground)]">
          {logs || "No log output."}
        </pre>
      )}
    </div>
  );
}

function deployColor(status: string): string {
  if (status.includes("finished") || status.includes("success")) return "var(--color-success)";
  if (status.includes("failed") || status.includes("error") || status.includes("cancelled")) return "var(--color-destructive)";
  if (status.includes("progress") || status.includes("queued")) return "var(--color-accent)";
  return "var(--color-muted-foreground)";
}

function fmtDate(s?: string | null): string {
  if (!s) return "—";
  const d = new Date(s);
  return isNaN(d.getTime()) ? s : d.toLocaleString();
}
