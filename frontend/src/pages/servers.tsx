import { useEffect, useState } from "react";
import { ArrowLeft, Server as ServerIcon, Globe, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { api, ApiError } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";

interface ServerRow {
  uuid: string;
  name: string;
  ip?: string;
  description?: string | null;
  settings?: { is_reachable?: boolean; is_usable?: boolean } | null;
}

interface ServerResource {
  uuid: string;
  name: string;
  type: string;
  status?: string | null;
}

interface ServerDetail extends ServerRow {
  resources: ServerResource[];
}

function reachable(s: ServerRow): boolean | undefined {
  return s.settings?.is_reachable;
}

function statusColor(ok: boolean | undefined): string {
  if (ok === undefined) return "var(--color-muted-foreground)";
  return ok ? "var(--color-success)" : "var(--color-destructive)";
}

function Dot({ ok }: { ok: boolean | undefined }) {
  return <span className="inline-block h-2 w-2 shrink-0 rounded-full" style={{ background: statusColor(ok) }} />;
}

export function ServersPage() {
  const [selected, setSelected] = useState<string | null>(null);
  return selected ? (
    <ServerDetailView uuid={selected} onBack={() => setSelected(null)} />
  ) : (
    <ServerList onOpen={setSelected} />
  );
}

function ServerList({ onOpen }: { onOpen: (uuid: string) => void }) {
  const { data: rows, error: err, refresh } = usePolledData<ServerRow[]>(() => api.get<ServerRow[]>("/servers"), 8000);

  return (
    <div className="mx-auto max-w-5xl space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Servers</h1>
          <p className="mt-1 text-sm text-[var(--color-muted-foreground)]">Hosts TinyHelm manages over SSH.</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => void refresh()}>
          <RefreshCw size={15} /> Refresh
        </Button>
      </div>

      {err && <ErrorBox message={err} />}
      {!err && rows === null && <SkeletonRows />}
      {!err && rows?.length === 0 && <EmptyBox message="No servers yet." />}

      {rows && rows.length > 0 && (
        <div className="divide-y overflow-hidden rounded-[var(--radius)] border bg-[var(--color-card)]">
          {rows.map((s) => (
            <button
              key={s.uuid}
              onClick={() => onOpen(s.uuid)}
              className="flex w-full items-center gap-4 px-5 py-4 text-left transition-colors hover:bg-[var(--color-elevated)]"
            >
              <Dot ok={reachable(s)} />
              <ServerIcon size={18} className="shrink-0 text-[var(--color-muted-foreground)]" />
              <div className="min-w-0 flex-1">
                <div className="truncate font-medium">{s.name}</div>
                {s.description && (
                  <div className="truncate text-sm text-[var(--color-muted-foreground)]">{s.description}</div>
                )}
              </div>
              {s.ip && (
                <div className="hidden items-center gap-1.5 text-sm text-[var(--color-muted-foreground)] sm:flex">
                  <Globe size={14} /> <span className="tabular-nums">{s.ip}</span>
                </div>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function ServerDetailView({ uuid, onBack }: { uuid: string; onBack: () => void }) {
  const [server, setServer] = useState<ServerDetail | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api
      .get<ServerDetail>(`/servers/${uuid}/resources`)
      .then(setServer)
      .catch((e: unknown) => setErr(e instanceof ApiError ? e.message : "Failed to load server"));
  }, [uuid]);

  return (
    <div className="mx-auto max-w-5xl space-y-6">
      <button
        onClick={onBack}
        className="flex items-center gap-2 text-sm text-[var(--color-muted-foreground)] transition-colors hover:text-[var(--color-foreground)]"
      >
        <ArrowLeft size={16} /> Servers
      </button>

      {err && <ErrorBox message={err} />}
      {!err && !server && <SkeletonRows />}

      {server && (
        <>
          <div className="flex items-center gap-3">
            <Dot ok={reachable(server)} />
            <h1 className="text-2xl font-semibold tracking-tight">{server.name}</h1>
          </div>

          <section className="grid grid-cols-2 gap-4 sm:grid-cols-4">
            <Meta label="IP" value={server.ip ?? "—"} />
            <Meta label="Reachable" value={fmtBool(server.settings?.is_reachable)} />
            <Meta label="Usable" value={fmtBool(server.settings?.is_usable)} />
            <Meta label="Resources" value={String(server.resources?.length ?? 0)} />
          </section>

          <section className="space-y-2">
            <h2 className="text-sm font-medium text-[var(--color-muted-foreground)]">Resources</h2>
            {server.resources?.length ? (
              <div className="divide-y overflow-hidden rounded-[var(--radius)] border bg-[var(--color-card)]">
                {server.resources.map((r) => (
                  <div key={r.uuid} className="flex items-center gap-4 px-5 py-3">
                    <Dot ok={r.status ? r.status.startsWith("running") : undefined} />
                    <div className="min-w-0 flex-1 truncate font-medium">{r.name}</div>
                    <span className="rounded-md bg-[var(--color-muted)] px-2 py-0.5 text-xs text-[var(--color-muted-foreground)]">
                      {r.type}
                    </span>
                    <span className="hidden text-sm text-[var(--color-muted-foreground)] sm:block">
                      {r.status ?? "unknown"}
                    </span>
                  </div>
                ))}
              </div>
            ) : (
              <EmptyBox message="No resources on this server." />
            )}
          </section>
        </>
      )}
    </div>
  );
}

function fmtBool(v: boolean | undefined): string {
  return v === undefined ? "—" : v ? "Yes" : "No";
}

function Meta({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-[var(--radius)] border bg-[var(--color-card)] p-4">
      <div className="text-xs text-[var(--color-muted-foreground)]">{label}</div>
      <div className="mt-1 truncate font-medium tabular-nums">{value}</div>
    </div>
  );
}

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

function SkeletonRows() {
  return (
    <div className="space-y-2">
      {[0, 1, 2].map((i) => (
        <div key={i} className="h-16 animate-pulse rounded-[var(--radius)] border bg-[var(--color-card)]" />
      ))}
    </div>
  );
}
