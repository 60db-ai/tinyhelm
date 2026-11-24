import { useEffect, useState } from "react";
import {
  LayoutDashboard,
  Server as ServerIcon,
  Boxes,
  Settings,
  Activity,
  SquareTerminal,
  Plug,
  Variable,
  Users,
  Bell,
  KeyRound,
  Tag,
  LogOut,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { api, getToken, setToken, clearToken } from "@/lib/api";
import { connectRealtime } from "@/lib/realtime";
import { ServersPage } from "@/pages/servers";
import { ProjectsPage } from "@/pages/projects";
import { DashboardPage } from "@/pages/dashboard";

const NAV_GROUPS = [
  {
    label: null,
    items: [
      { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
      { id: "projects", label: "Projects", icon: Boxes },
      { id: "monitoring", label: "Monitoring", icon: Activity },
      { id: "terminal", label: "Terminal", icon: SquareTerminal },
    ],
  },
  {
    label: "Infrastructure",
    items: [
      { id: "servers", label: "Servers", icon: ServerIcon },
      { id: "integrations", label: "Integrations", icon: Plug },
      { id: "shared-variables", label: "Shared Variables", icon: Variable },
    ],
  },
  {
    label: "Manage",
    items: [
      { id: "team", label: "Team", icon: Users },
      { id: "notifications", label: "Notifications", icon: Bell },
      { id: "keys", label: "Keys & Tokens", icon: KeyRound },
      { id: "tags", label: "Tags", icon: Tag },
      { id: "settings", label: "Settings", icon: Settings },
    ],
  },
] as const;
type NavId = (typeof NAV_GROUPS)[number]["items"][number]["id"];

export default function App() {
  const [token, setTok] = useState<string | null>(getToken());
  if (!token) return <TokenGate onConnect={(t) => { setToken(t); setTok(t); }} />;
  return <Shell onDisconnect={() => { clearToken(); setTok(null); }} />;
}

function Shell({ onDisconnect }: { onDisconnect: () => void }) {
  const [active, setActive] = useState<NavId>("dashboard");
  const [live, setLive] = useState(false);

  useEffect(() => {
    let disconnect: (() => void) | undefined;
    connectRealtime(() => window.dispatchEvent(new Event("tinyhelm:refresh")))
      .then((d) => {
        disconnect = d;
        setLive(true);
      })
      .catch(() => setLive(false));
    return () => disconnect?.();
  }, []);

  return (
    <div className="flex min-h-screen">
      <aside className="flex w-60 shrink-0 flex-col border-r bg-[var(--color-card)]">
        <div className="flex h-16 items-center px-5">
          <img src="/tinyhelm-logo-dark.png" alt="TinyHelm" className="h-10 w-auto" />
        </div>
        <nav className="flex-1 space-y-3 overflow-y-auto px-3 py-2">
          {NAV_GROUPS.map((group, gi) => (
            <div key={gi} className="space-y-1">
              {group.label && (
                <div className="px-3 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wider text-[var(--color-muted-foreground)]">
                  {group.label}
                </div>
              )}
              {group.items.map(({ id, label, icon: Icon }) => (
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
            </div>
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
          <div className="flex items-center gap-4">
            {live && (
              <span className="flex items-center gap-1.5 text-xs text-[var(--color-success)]">
                <span className="inline-block h-2 w-2 animate-pulse rounded-full bg-[var(--color-success)]" /> Realtime
              </span>
            )}
            <ApiStatus />
          </div>
        </header>
        <main className="flex-1 overflow-auto p-6">
          {active === "dashboard" ? (
            <DashboardPage onNavigate={setActive} />
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
    api.get("/team").then(() => setOk(true)).catch(() => setOk(false));
  }, []);
  const color = ok === null ? "var(--color-muted-foreground)" : ok ? "var(--color-success)" : "var(--color-destructive)";
  return (
    <div className="flex items-center gap-2 text-xs text-[var(--color-muted-foreground)]">
      <span className={"inline-block h-2 w-2 rounded-full" + (ok ? " animate-pulse" : "")} style={{ background: color }} />
      {ok === null ? "connecting…" : ok ? "Live" : "API error"}
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
        <img src="/tinyhelm-logo-dark.png" alt="TinyHelm" className="mx-auto h-9 w-auto" />
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
