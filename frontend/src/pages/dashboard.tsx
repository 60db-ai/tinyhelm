import type { ReactNode } from "react";
import { Boxes, Server as ServerIcon, Rocket, ChevronRight } from "lucide-react";
import { api } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";

interface DashProject {
  uuid: string;
  name: string;
  description: string | null;
  environments: number;
  resources: number;
}

interface DashServer {
  uuid: string;
  name: string;
  description: string | null;
  ip: string;
  is_reachable: boolean | null;
  is_usable: boolean | null;
}

interface DashDeployment {
  deployment_uuid: string;
  status: string;
  application_name: string | null;
  application_uuid: string;
  project: string;
  environment: string;
  created_at: string | null;
}

interface DashboardData {
  projects: DashProject[];
  servers: DashServer[];
  deployments: DashDeployment[];
}

interface DashboardPageProps {
  onNavigate: (page: "projects" | "servers") => void;
}

const STATUS_COLOR: Record<string, string> = {
  finished: "var(--color-success)",
  failed: "var(--color-destructive)",
  in_progress: "var(--color-primary)",
  queued: "var(--color-muted-foreground)",
};

function timeAgo(iso: string | null): string {
  if (!iso) return "";
  const seconds = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000);
  if (seconds < 60) return "just now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
  return `${Math.floor(seconds / 86400)}d ago`;
}

export function DashboardPage({ onNavigate }: DashboardPageProps) {
  const { data, error } = usePolledData<DashboardData>(() => api.get("/dashboard"), 10000);

  return (
    <div className="mx-auto grid max-w-7xl gap-6 xl:grid-cols-[1fr_360px]">
      <div className="min-w-0 space-y-8">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Dashboard</h1>
          <p className="mt-1 text-sm text-[var(--color-muted-foreground)]">Your self-hosted infrastructure.</p>
        </div>

        {error && (
          <div className="rounded-[var(--radius)] border border-[var(--color-destructive)]/40 bg-[var(--color-destructive)]/10 px-4 py-3 text-sm">
            {error}
          </div>
        )}

        <Section title="Projects" icon={<Boxes size={16} />} count={data?.projects.length} onAll={() => onNavigate("projects")}>
          {data?.projects.length === 0 && <Empty text="No projects yet." />}
          {data?.projects.map((p) => (
            <TableRow key={p.uuid} onClick={() => onNavigate("projects")}>
              <div className="min-w-0">
                <div className="truncate font-medium">{p.name}</div>
                <div className="truncate text-xs text-[var(--color-muted-foreground)]">{p.description || "No description"}</div>
              </div>
              <Meta>{p.environments} env</Meta>
              <Meta>{p.resources} resources</Meta>
            </TableRow>
          ))}
        </Section>

        <Section title="Servers" icon={<ServerIcon size={16} />} count={data?.servers.length} onAll={() => onNavigate("servers")}>
          {data?.servers.length === 0 && <Empty text="No servers yet." />}
          {data?.servers.map((s) => (
            <TableRow key={s.uuid} onClick={() => onNavigate("servers")}>
              <div className="min-w-0">
                <div className="truncate font-medium">{s.name}</div>
                <div className="truncate font-mono text-xs text-[var(--color-muted-foreground)]">{s.ip}</div>
              </div>
              <StatusPill ok={!!s.is_reachable} okText="Reachable" badText="Unreachable" />
              <StatusPill ok={!!s.is_usable} okText="Usable" badText="Not usable" />
            </TableRow>
          ))}
        </Section>
      </div>

      <aside className="h-fit rounded-[var(--radius)] border bg-[var(--color-card)]">
        <div className="flex items-center gap-2 border-b px-4 py-3 text-sm font-medium">
          <Rocket size={16} /> Recent deployments
        </div>
        {data?.deployments.length === 0 && <Empty text="No deployments yet." />}
        <ul className="divide-y">
          {data?.deployments.map((d) => (
            <li key={d.deployment_uuid} className="px-4 py-3">
              <div className="flex items-center justify-between gap-2">
                <span className="truncate text-sm font-medium">{d.application_name || d.application_uuid}</span>
                <span className="flex shrink-0 items-center gap-1.5 text-xs capitalize" style={{ color: STATUS_COLOR[d.status] }}>
                  <span className="h-1.5 w-1.5 rounded-full" style={{ background: STATUS_COLOR[d.status] }} />
                  {d.status.replace("_", " ")}
                </span>
              </div>
              <div className="mt-0.5 flex justify-between text-xs text-[var(--color-muted-foreground)]">
                <span className="truncate">{d.project} / {d.environment}</span>
                <span className="shrink-0">{timeAgo(d.created_at)}</span>
              </div>
            </li>
          ))}
        </ul>
      </aside>
    </div>
  );
}

function Section({ title, icon, count, onAll, children }: { title: string; icon: ReactNode; count?: number; onAll: () => void; children: ReactNode }) {
  return (
    <section>
      <div className="mb-3 flex items-center justify-between">
        <h2 className="flex items-center gap-2 text-sm font-medium">
          {icon} {title}
          {count !== undefined && <span className="text-[var(--color-muted-foreground)]">{count}</span>}
        </h2>
        <button onClick={onAll} className="text-xs text-[var(--color-muted-foreground)] hover:text-[var(--color-foreground)]">
          View all
        </button>
      </div>
      <div className="divide-y overflow-hidden rounded-[var(--radius)] border bg-[var(--color-card)]">{children}</div>
    </section>
  );
}

function TableRow({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button onClick={onClick} className="grid w-full grid-cols-[1fr_auto_auto_16px] items-center gap-6 px-4 py-3 text-left text-sm transition-colors hover:bg-[var(--color-elevated)]">
      {children}
      <ChevronRight size={16} className="text-[var(--color-muted-foreground)]" />
    </button>
  );
}

function Meta({ children }: { children: ReactNode }) {
  return <span className="text-xs tabular-nums text-[var(--color-muted-foreground)]">{children}</span>;
}

function StatusPill({ ok, okText, badText }: { ok: boolean; okText: string; badText: string }) {
  const color = ok ? "var(--color-success)" : "var(--color-destructive)";
  return (
    <span className="flex items-center gap-1.5 text-xs" style={{ color }}>
      <span className="h-1.5 w-1.5 rounded-full" style={{ background: color }} />
      {ok ? okText : badText}
    </span>
  );
}

function Empty({ text }: { text: string }) {
  return <div className="px-4 py-6 text-center text-sm text-[var(--color-muted-foreground)]">{text}</div>;
}
