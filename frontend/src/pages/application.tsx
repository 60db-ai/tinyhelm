import { useEffect, useState, type ReactNode } from "react";
import { ArrowLeft, GitBranch, Globe, Box, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { api, ApiError } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";
import { EnvEditor } from "@/components/env-editor";

// Local state helpers (same small set used by the other ported pages).
function ErrorBox({ message }: { message: string }) {
  return (
    <div className="rounded-xl border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm">
      {message}
    </div>
  );
}
function EmptyBox({ message }: { message: string }) {
  return (
    <div className="rounded-xl border border-dashed py-16 text-center text-sm text-muted-foreground">
      {message}
    </div>
  );
}
function Skeleton() {
  return <div className="h-40 animate-pulse rounded-xl border bg-card" />;
}
function statusColor(status?: string | null): string {
  if (!status) return "var(--muted-foreground)";
  if (status.startsWith("running")) return "var(--success)";
  if (status.startsWith("exited") || status.includes("unhealthy")) return "var(--destructive)";
  return "var(--muted-foreground)";
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
  const { data: app, error: err, refresh } = usePolledData<Application>(
    () => api.get<Application>(`/applications/${uuid}`),
    6000,
  );
  const [tab, setTab] = useState<Tab>("overview");

  return (
    <div className="mx-auto max-w-5xl space-y-6">
      <button
        onClick={onBack}
        className="flex items-center gap-2 text-sm text-muted-foreground transition-colors hover:text-foreground"
      >
        <ArrowLeft size={16} /> {backLabel}
      </button>

      {err && <ErrorBox message={err} />}
      {!err && !app && <Skeleton />}

      {app && (
        <>
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex items-center gap-3">
              <span className="inline-block h-2.5 w-2.5 rounded-full" style={{ background: statusColor(app.status) }} />
              <h1 className="text-2xl font-semibold tracking-tight">{app.name}</h1>
              <span className="text-sm text-muted-foreground">{app.status ?? "unknown"}</span>
            </div>
            <Actions uuid={uuid} onDone={refresh} />
          </div>

          <div className="flex gap-1 border-b">
            {TABS.map((t) => (
              <button
                key={t}
                onClick={() => setTab(t)}
                className={
                  "relative px-3 py-2 text-sm capitalize transition-colors " +
                  (tab === t
                    ? "text-foreground after:absolute after:inset-x-3 after:-bottom-px after:h-0.5 after:bg-accent"
                    : "text-muted-foreground hover:text-foreground")
                }
              >
                {t}
              </button>
            ))}
          </div>

          {tab === "overview" && <Overview app={app} />}
          {tab === "environment" && <EnvEditor kind="applications" uuid={uuid} />}
          {tab === "deployments" && <DeploymentsTab uuid={uuid} />}
          {tab === "logs" && <LogsTab uuid={uuid} />}
        </>
      )}
    </div>
  );
}

function Actions({ uuid, onDone }: { uuid: string; onDone: () => void | Promise<void> }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  const run = async (label: string, path: string, confirmMsg?: string) => {
    if (confirmMsg && !window.confirm(confirmMsg)) return;
    setBusy(label);
    setMsg(null);
    try {
      await api.post(path);
      setMsg(`${label} triggered`);
      await onDone();
    } catch (e: unknown) {
      setMsg(e instanceof ApiError ? e.message : `${label} failed`);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="flex flex-wrap items-center gap-2">
      {msg && <span className="text-xs text-muted-foreground">{msg}</span>}
      <Button variant="default" size="sm" disabled={!!busy} onClick={() => run("Deploy", `/applications/${uuid}/start`)}>
        {busy === "Deploy" ? "Deploying…" : "Deploy"}
      </Button>
      <Button variant="outline" size="sm" disabled={!!busy} onClick={() => run("Restart", `/applications/${uuid}/restart`)}>
        {busy === "Restart" ? "Restarting…" : "Restart"}
      </Button>
      <Button
        variant="outline"
        size="sm"
        disabled={!!busy}
        onClick={() => run("Stop", `/applications/${uuid}/stop`, "Stop this application?")}
      >
        {busy === "Stop" ? "Stopping…" : "Stop"}
      </Button>
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
    <div className={"rounded-xl border bg-card p-4" + (full ? " sm:col-span-2" : "")}>
      <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
        {icon} {label}
      </div>
      <div className="mt-1 break-all font-medium">{value}</div>
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
    <div className="divide-y overflow-hidden rounded-xl border bg-card">
      {rows.map((d, i) => (
        <div key={d.deployment_uuid ?? d.id ?? i} className="flex items-center gap-4 px-5 py-3">
          <span className="inline-block h-2 w-2 shrink-0 rounded-full" style={{ background: deployColor(d.status) }} />
          <span className="w-28 shrink-0 text-sm capitalize">{d.status}</span>
          <span className="min-w-0 flex-1 truncate font-mono text-sm text-muted-foreground">
            {d.commit ? d.commit.slice(0, 8) : "—"}
          </span>
          <span className="hidden text-sm text-muted-foreground sm:block">{fmtDate(d.created_at)}</span>
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
        <pre className="max-h-[480px] overflow-auto rounded-xl border bg-black/40 p-4 font-mono text-xs leading-relaxed text-muted-foreground">
          {logs || "No log output."}
        </pre>
      )}
    </div>
  );
}

function deployColor(status: string): string {
  if (status.includes("finished") || status.includes("success")) return "var(--success)";
  if (status.includes("failed") || status.includes("error") || status.includes("cancelled")) return "var(--destructive)";
  if (status.includes("progress") || status.includes("queued")) return "var(--accent)";
  return "var(--muted-foreground)";
}

function fmtDate(s?: string | null): string {
  if (!s) return "—";
  const d = new Date(s);
  return isNaN(d.getTime()) ? s : d.toLocaleString();
}
