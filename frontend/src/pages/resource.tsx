import { useEffect, useState, type ReactNode } from "react";
import { ArrowLeft, Globe, Box, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { api, ApiError } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";
import { EnvEditor } from "@/components/env-editor";

// Generic detail page for Databases and Services (Applications have their own page with git/deployments).
export type ResourceKind = "databases" | "services";

function ErrorBox({ message }: { message: string }) {
  return (
    <div className="rounded-[var(--radius)] border border-[var(--color-destructive)]/40 bg-[var(--color-destructive)]/10 px-4 py-3 text-sm">
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

interface Resource {
  uuid: string;
  name: string;
  status?: string | null;
  description?: string | null;
  image?: string | null;
  fqdn?: string | null;
}
const TABS = ["overview", "environment", "logs"] as const;
type Tab = (typeof TABS)[number];

export function ResourceDetail({
  kind,
  uuid,
  onBack,
  backLabel = "Back",
}: {
  kind: ResourceKind;
  uuid: string;
  onBack: () => void;
  backLabel?: string;
}) {
  const { data: res, error: err, refresh } = usePolledData<Resource>(
    () => api.get<Resource>(`/${kind}/${uuid}`),
    6000,
  );
  const [tab, setTab] = useState<Tab>("overview");

  const label = kind === "databases" ? "Database" : "Service";

  return (
    <div className="mx-auto max-w-5xl space-y-6">
      <button
        onClick={onBack}
        className="flex items-center gap-2 text-sm text-[var(--color-muted-foreground)] transition-colors hover:text-[var(--color-foreground)]"
      >
        <ArrowLeft size={16} /> {backLabel}
      </button>

      {err && <ErrorBox message={err} />}
      {!err && !res && <Skeleton />}

      {res && (
        <>
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex items-center gap-3">
              <span className="inline-block h-2.5 w-2.5 rounded-full" style={{ background: statusColor(res.status) }} />
              <h1 className="text-2xl font-semibold tracking-tight">{res.name}</h1>
              <span className="rounded-md bg-[var(--color-muted)] px-2 py-0.5 text-xs text-[var(--color-muted-foreground)]">{label}</span>
              <span className="text-sm text-[var(--color-muted-foreground)]">{res.status ?? "unknown"}</span>
            </div>
            <Actions kind={kind} uuid={uuid} onDone={refresh} />
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

          {tab === "overview" && <Overview res={res} />}
          {tab === "environment" && <EnvEditor kind={kind} uuid={uuid} />}
          {tab === "logs" && <LogsTab kind={kind} uuid={uuid} />}
        </>
      )}
    </div>
  );
}

function Actions({ kind, uuid, onDone }: { kind: ResourceKind; uuid: string; onDone: () => void | Promise<void> }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  const run = async (verb: string, confirmMsg?: string) => {
    if (confirmMsg && !window.confirm(confirmMsg)) return;
    setBusy(verb);
    setMsg(null);
    try {
      await api.post(`/${kind}/${uuid}/${verb}`);
      setMsg(`${verb} triggered`);
      await onDone();
    } catch (e: unknown) {
      setMsg(e instanceof ApiError ? e.message : `${verb} failed`);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="flex flex-wrap items-center gap-2">
      {msg && <span className="text-xs text-[var(--color-muted-foreground)]">{msg}</span>}
      <Button variant="accent" size="sm" disabled={!!busy} onClick={() => run("start")}>
        {busy === "start" ? "Starting…" : "Start"}
      </Button>
      <Button variant="outline" size="sm" disabled={!!busy} onClick={() => run("restart")}>
        {busy === "restart" ? "Restarting…" : "Restart"}
      </Button>
      <Button variant="outline" size="sm" disabled={!!busy} onClick={() => run("stop", "Stop this resource?")}>
        {busy === "stop" ? "Stopping…" : "Stop"}
      </Button>
    </div>
  );
}

function Overview({ res }: { res: Resource }) {
  return (
    <section className="grid grid-cols-1 gap-4 sm:grid-cols-2">
      {res.image && <Meta icon={<Box size={15} />} label="Image" value={res.image} />}
      {res.fqdn && <Meta icon={<Globe size={15} />} label="Domains" value={res.fqdn} />}
      {res.description && <Meta label="Description" value={res.description} full />}
      {!res.image && !res.fqdn && !res.description && <Meta label="Status" value={res.status ?? "unknown"} />}
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

function LogsTab({ kind, uuid }: { kind: ResourceKind; uuid: string }) {
  const [logs, setLogs] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [nonce, setNonce] = useState(0);
  useEffect(() => {
    setLogs(null);
    setErr(null);
    api
      .get<{ logs?: string; message?: string }>(`/${kind}/${uuid}/logs`)
      .then((d) => setLogs(d.logs ?? d.message ?? ""))
      .catch((e: unknown) =>
        setErr(e instanceof ApiError ? (e.status === 403 ? "Needs a token with read:sensitive." : e.message) : "Failed to load logs"),
      );
  }, [kind, uuid, nonce]);

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
