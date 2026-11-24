import { useEffect, useState, type ReactNode } from "react";
import {
  LayoutDashboard,
  Server as ServerIcon,
  Boxes,
  Rocket,
  Settings,
  Activity,
  LogOut,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { MetricsChart, type MetricPoint } from "@/components/metrics-chart";
import { api, getToken, setToken, clearToken, type Server, type Project } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";
import { ServersPage } from "@/pages/servers";
import { ProjectsPage } from "@/pages/projects";

const NAV = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "servers", label: "Servers", icon: ServerIcon },
  { id: "projects", label: "Projects", icon: Boxes },
  { id: "deployments", label: "Deployments", icon: Rocket },
  { id: "settings", label: "Settings", icon: Settings },
] as const;
type NavId = (typeof NAV)[number]["id"];

export default function App() {
  const [token, setTok] = useState<string | null>(getToken());
  if (!token) return <TokenGate onConnect={(t) => { setToken(t); setTok(t); }} />;
  return <Shell onDisconnect={() => { clearToken(); setTok(null); }} />;
}

function Shell({ onDisconnect }: { onDisconnect: () => void }) {
  const [active, setActive] = useState<NavId>("dashboard");
  return (
    <div className="flex min-h-screen">
      <aside className="flex w-60 shrink-0 flex-col border-r bg-[var(--color-card)]">
        <div className="flex h-16 items-center px-5">
          <img src="/tinyhelm-logo.png" alt="TinyHelm" className="h-7 w-auto" />
        </div>
        <nav className="flex-1 space-y-1 px-3 py-2">
          {NAV.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => setActive(id)}
              className={
                "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors " +
                (active === id
                  ? "bg-[var(--color-elevated)] text-[var(--color-foreground)]"
                  : "text-[var(--color-muted-foreground)] hover:bg-[var(--color-muted)] hover:text-[var(--color-foreground)]")
              }
            >
              <Icon size={17} />
              {label}
            </button>
          ))}
        </nav>
        <button
          onClick={onDisconnect}
          className="m-3 flex items-center gap-3 rounded-lg px-3 py-2 text-sm text-[var(--color-muted-foreground)] transition-colors hover:bg-[var(--color-muted)] hover:text-[var(--color-foreground)]"
        >
          <LogOut size={17} /> Disconnect
        </button>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-16 items-center justify-between border-b px-6">
          <h2 className="text-sm font-medium capitalize text-[var(--color-muted-foreground)]">{active}</h2>
          <ApiStatus />
        </header>
        <main className="flex-1 overflow-auto p-6">
          {active === "dashboard" ? (
            <Dashboard />
          ) : active === "servers" ? (
            <ServersPage />
          ) : active === "projects" ? (
            <ProjectsPage />
          ) : (
            <Placeholder title={active} />
          )}
        </main>
      </div>
    </div>
  );
}

function ApiStatus() {
  const [ok, setOk] = useState<boolean | null>(null);
  useEffect(() => {
    api.get("/version").then(() => setOk(true)).catch(() => setOk(false));
  }, []);
  const color = ok === null ? "var(--color-muted-foreground)" : ok ? "var(--color-success)" : "var(--color-destructive)";
  return (
    <div className="flex items-center gap-2 text-xs text-[var(--color-muted-foreground)]">
      <span className={"inline-block h-2 w-2 rounded-full" + (ok ? " animate-pulse" : "")} style={{ background: color }} />
      {ok === null ? "connecting…" : ok ? "Live" : "API error"}
    </div>
  );
}

const demo = (base: number, amp: number, fn: (i: number) => number): MetricPoint[] =>
  Array.from({ length: 12 }, (_, i) => ({ t: `${i * 5}m`, value: Math.round(base + amp * fn(i)) }));

function Dashboard() {
  const { data, error: err } = usePolledData<[Server[], Project[]]>(
    () => Promise.all([api.get<Server[]>("/servers"), api.get<Project[]>("/projects")]),
    10000,
  );
  const servers = data?.[0] ?? null;
  const projects = data?.[1] ?? null;

  const num = (v: unknown[] | null) => (v === null ? "—" : String(v.length));

  return (
    <div className="mx-auto max-w-6xl space-y-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Overview</h1>
        <p className="mt-1 text-sm text-[var(--color-muted-foreground)]">Your infrastructure at a glance.</p>
      </div>

      {err && (
        <div className="rounded-[var(--radius)] border border-[var(--color-destructive)]/40 bg-[var(--color-destructive)]/10 px-4 py-3 text-sm">
          {err}
        </div>
      )}

      <section className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <Stat icon={<ServerIcon size={18} />} label="Servers" value={num(servers)} />
        <Stat icon={<Boxes size={18} />} label="Projects" value={num(projects)} />
        <Stat icon={<Activity size={18} />} label="Reachable" value={servers ? String(servers.filter((s) => s.reachable).length) : "—"} />
      </section>

      <section className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <MetricsChart title="CPU (%)" unit="%" data={demo(35, 25, (i) => Math.sin(i / 2))} />
        <MetricsChart title="Memory (%)" unit="%" data={demo(55, 15, (i) => Math.cos(i / 3))} />
      </section>
    </div>
  );
}

function Stat({ icon, label, value }: { icon: ReactNode; label: string; value: string }) {
  return (
    <div className="rounded-[var(--radius)] border bg-[var(--color-card)] p-5 transition-colors hover:bg-[var(--color-elevated)]">
      <div className="flex items-center gap-2 text-[var(--color-muted-foreground)]">
        {icon}
        <span className="text-sm">{label}</span>
      </div>
      <div className="mt-3 text-3xl font-semibold tabular-nums">{value}</div>
    </div>
  );
}

function Placeholder({ title }: { title: string }) {
  return (
    <div className="mx-auto flex max-w-6xl flex-col items-center justify-center rounded-[var(--radius)] border border-dashed py-24 text-center">
      <h1 className="text-xl font-semibold capitalize">{title}</h1>
      <p className="mt-2 max-w-sm text-sm text-[var(--color-muted-foreground)]">
        Being ported from Livewire to Rust + shadcn. See rust-migration/MIGRATION.md.
      </p>
    </div>
  );
}

function TokenGate({ onConnect }: { onConnect: (token: string) => void }) {
  const [value, setValue] = useState("");
  return (
    <div className="flex min-h-screen items-center justify-center p-4">
      <div className="w-full max-w-sm rounded-[var(--radius)] border bg-[var(--color-card)] p-8">
        <img src="/tinyhelm-logo.png" alt="TinyHelm" className="mx-auto h-9 w-auto" />
        <h1 className="mt-6 text-center text-lg font-semibold">Connect to TinyHelm</h1>
        <p className="mt-1 text-center text-sm text-[var(--color-muted-foreground)]">
          Paste an API token (Security › API Tokens).
        </p>
        <input
          type="password"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && value.trim() && onConnect(value)}
          placeholder="API token"
          className="mt-6 w-full rounded-lg border bg-[var(--color-background)] px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-[var(--color-ring)]"
        />
        <Button className="mt-4 w-full" disabled={!value.trim()} onClick={() => onConnect(value)}>
          Connect
        </Button>
      </div>
    </div>
  );
}
