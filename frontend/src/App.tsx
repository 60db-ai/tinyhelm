import { useEffect, useState, type ReactNode } from "react";
import { Activity, Server, Boxes } from "lucide-react";
import { Button } from "@/components/ui/button";
import { MetricsChart, type MetricPoint } from "@/components/metrics-chart";

// Demo series until the real /api/v1 metrics endpoints are wired.
const demoCpu: MetricPoint[] = Array.from({ length: 12 }, (_, i) => ({
  t: `${i * 5}m`,
  value: Math.round(30 + 25 * Math.sin(i / 2) + Math.random() * 10),
}));
const demoMem: MetricPoint[] = Array.from({ length: 12 }, (_, i) => ({
  t: `${i * 5}m`,
  value: Math.round(55 + 15 * Math.cos(i / 3) + Math.random() * 6),
}));

type Health = "checking" | "ok" | "down";

export default function App() {
  const [health, setHealth] = useState<Health>("checking");

  useEffect(() => {
    // Proves the API wiring: the gateway/Vite proxy forwards /api to Laravel.
    fetch("/api/health")
      .then((r) => setHealth(r.ok ? "ok" : "down"))
      .catch(() => setHealth("down"));
  }, []);

  return (
    <div className="min-h-screen">
      <header className="flex items-center justify-between border-b px-6 py-4">
        <div className="flex items-center gap-3">
          <img src="/tinyhelm-logo.png" alt="TinyHelm" className="h-8 w-auto" />
        </div>
        <div className="flex items-center gap-2 text-sm">
          <span
            className="inline-block h-2 w-2 rounded-full"
            style={{
              background:
                health === "ok"
                  ? "var(--color-accent)"
                  : health === "down"
                    ? "var(--color-destructive)"
                    : "var(--color-muted-foreground)",
            }}
          />
          <span className="text-[var(--color-muted-foreground)]">API: {health}</span>
          <Button variant="accent" size="sm">New resource</Button>
        </div>
      </header>

      <main className="mx-auto max-w-6xl px-6 py-8">
        <h1 className="text-2xl font-semibold">Dashboard</h1>
        <p className="mt-1 text-sm text-[var(--color-muted-foreground)]">
          Rust gateway + TypeScript SPA — the new TinyHelm frontend shell.
        </p>

        <section className="mt-6 grid grid-cols-1 gap-4 sm:grid-cols-3">
          <StatCard icon={<Server size={18} />} label="Servers" value="—" />
          <StatCard icon={<Boxes size={18} />} label="Resources" value="—" />
          <StatCard icon={<Activity size={18} />} label="Deployments" value="—" />
        </section>

        <section className="mt-6 grid grid-cols-1 gap-4 lg:grid-cols-2">
          <MetricsChart title="CPU (%)" data={demoCpu} unit="%" />
          <MetricsChart title="Memory (%)" data={demoMem} unit="%" />
        </section>
      </main>
    </div>
  );
}

function StatCard({
  icon,
  label,
  value,
}: {
  icon: ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="rounded-[var(--radius)] border bg-[var(--color-card)] p-4">
      <div className="flex items-center gap-2 text-[var(--color-muted-foreground)]">
        {icon}
        <span className="text-sm">{label}</span>
      </div>
      <div className="mt-2 text-2xl font-semibold">{value}</div>
    </div>
  );
}
