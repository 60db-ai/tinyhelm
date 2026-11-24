import type { ComponentType, ReactNode } from "react";
import { Boxes, Server, Rocket, Variable, ArrowUpRight, CircleAlert } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle, CardAction } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import type { NavId } from "@/components/app-sidebar";
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
  teamName: string | null;
  onNavigate: (page: NavId) => void;
}

const DEPLOY_STATUS: Record<string, { label: string; className: string }> = {
  finished: { label: "Finished", className: "bg-success/10 text-success border-success/20" },
  failed: { label: "Failed", className: "bg-destructive/10 text-destructive border-destructive/20" },
  in_progress: { label: "Deploying", className: "bg-brand/10 text-brand border-brand/20" },
  queued: { label: "Queued", className: "bg-muted text-muted-foreground" },
};

function greeting(): string {
  const hour = new Date().getHours();
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
}

function timeAgo(iso: string | null): string {
  if (!iso) return "";
  const seconds = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000);
  if (seconds < 60) return "just now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
  return `${Math.floor(seconds / 86400)}d ago`;
}

export function DashboardPage({ teamName, onNavigate }: DashboardPageProps) {
  const { data, error } = usePolledData<DashboardData>(() => api.get("/dashboard"), 10000);

  return (
    <div className="mx-auto max-w-6xl space-y-10">
      <header>
        <p className="text-sm text-muted-foreground">{teamName ?? " "}</p>
        <h1 className="mt-1 text-3xl font-semibold tracking-tight">{greeting()}</h1>
      </header>

      <section className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <QuickAction icon={Boxes} title="Projects" detail={data ? `${data.projects.length} total` : null} tint="from-violet-500/15" onClick={() => onNavigate("projects")} />
        <QuickAction icon={Server} title="Servers" detail={data ? `${data.servers.filter((s) => s.is_reachable).length}/${data.servers.length} reachable` : null} tint="from-sky-500/15" onClick={() => onNavigate("servers")} />
        <QuickAction icon={Rocket} title="Deployments" detail={data ? `${data.deployments.length} recent` : null} tint="from-orange-500/15" onClick={() => onNavigate("projects")} />
        <QuickAction icon={Variable} title="Shared variables" detail="Team-wide secrets" tint="from-emerald-500/15" onClick={() => onNavigate("shared-variables")} />
      </section>

      {error && (
        <div className="flex items-center gap-2 rounded-xl border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive">
          <CircleAlert className="size-4" /> {error}
        </div>
      )}

      <div className="grid gap-6 xl:grid-cols-[1fr_340px]">
        <div className="min-w-0 space-y-6">
          <Card className="gap-0 py-0">
            <SectionHeader title="Projects" onAll={() => onNavigate("projects")} />
            <Table>
              <TableHeader>
                <TableRow className="hover:bg-transparent">
                  <TableHead className="pl-6">Name</TableHead>
                  <TableHead className="text-right">Environments</TableHead>
                  <TableHead className="pr-6 text-right">Resources</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {!data && <SkeletonRows cols={3} />}
                {data?.projects.length === 0 && <EmptyRow cols={3} text="No projects yet." />}
                {data?.projects.map((p) => (
                  <TableRow key={p.uuid} className="cursor-pointer" onClick={() => onNavigate("projects")}>
                    <TableCell className="pl-6">
                      <div className="font-medium">{p.name}</div>
                      <div className="text-xs text-muted-foreground">{p.description || "No description"}</div>
                    </TableCell>
                    <TableCell className="text-right tabular-nums">{p.environments}</TableCell>
                    <TableCell className="pr-6 text-right tabular-nums">{p.resources}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </Card>

          <Card className="gap-0 py-0">
            <SectionHeader title="Servers" onAll={() => onNavigate("servers")} />
            <Table>
              <TableHeader>
                <TableRow className="hover:bg-transparent">
                  <TableHead className="pl-6">Name</TableHead>
                  <TableHead>Address</TableHead>
                  <TableHead className="pr-6 text-right">Status</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {!data && <SkeletonRows cols={3} />}
                {data?.servers.length === 0 && <EmptyRow cols={3} text="No servers yet." />}
                {data?.servers.map((s) => (
                  <TableRow key={s.uuid} className="cursor-pointer" onClick={() => onNavigate("servers")}>
                    <TableCell className="pl-6 font-medium">{s.name}</TableCell>
                    <TableCell className="font-mono text-xs text-muted-foreground">{s.ip}</TableCell>
                    <TableCell className="pr-6 text-right">
                      <ServerStatus reachable={!!s.is_reachable} usable={!!s.is_usable} />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </Card>
        </div>

        <Card className="h-fit gap-0 py-0">
          <CardHeader className="border-b px-5 py-4 [.border-b]:pb-4">
            <CardTitle className="text-sm">Recent deployments</CardTitle>
          </CardHeader>
          <CardContent className="px-0">
            {!data && (
              <div className="space-y-3 p-5">
                <Skeleton className="h-10" />
                <Skeleton className="h-10" />
              </div>
            )}
            {data?.deployments.length === 0 && <p className="p-6 text-center text-sm text-muted-foreground">No deployments yet.</p>}
            <ul className="divide-y">
              {data?.deployments.map((d) => {
                const status = DEPLOY_STATUS[d.status] ?? DEPLOY_STATUS.queued;
                return (
                  <li key={d.deployment_uuid} className="px-5 py-3.5">
                    <div className="flex items-center justify-between gap-3">
                      <span className="truncate text-sm font-medium">{d.application_name || d.application_uuid}</span>
                      <Badge variant="outline" className={status.className}>
                        {status.label}
                      </Badge>
                    </div>
                    <div className="mt-1 flex justify-between gap-3 text-xs text-muted-foreground">
                      <span className="truncate">
                        {d.project} · {d.environment}
                      </span>
                      <span className="shrink-0">{timeAgo(d.created_at)}</span>
                    </div>
                  </li>
                );
              })}
            </ul>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

interface QuickActionProps {
  icon: ComponentType<{ className?: string }>;
  title: string;
  detail: string | null;
  tint: string;
  onClick: () => void;
}

function QuickAction({ icon: Icon, title, detail, tint, onClick }: QuickActionProps) {
  return (
    <button
      onClick={onClick}
      className={`group relative overflow-hidden rounded-2xl border bg-gradient-to-br ${tint} to-transparent p-4 text-left transition-all hover:-translate-y-0.5 hover:shadow-md focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none`}
    >
      <div className="flex items-start justify-between">
        <span className="flex size-9 items-center justify-center rounded-xl border bg-background shadow-xs">
          <Icon className="size-4" />
        </span>
        <ArrowUpRight className="size-4 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100" />
      </div>
      <div className="mt-6 text-sm font-medium">{title}</div>
      <div className="mt-0.5 text-xs text-muted-foreground">{detail ?? <Skeleton className="h-3 w-16" />}</div>
    </button>
  );
}

function SectionHeader({ title, onAll }: { title: string; onAll: () => void }) {
  return (
    <CardHeader className="items-center border-b px-6 py-3 [.border-b]:pb-3">
      <CardTitle className="row-span-2 self-center text-sm">{title}</CardTitle>
      <CardAction>
        <Button variant="ghost" size="sm" className="h-7 text-muted-foreground" onClick={onAll}>
          View all
        </Button>
      </CardAction>
    </CardHeader>
  );
}

function ServerStatus({ reachable, usable }: { reachable: boolean; usable: boolean }): ReactNode {
  if (reachable && usable) return <Badge variant="outline" className="border-success/20 bg-success/10 text-success">Healthy</Badge>;
  if (reachable) return <Badge variant="outline" className="border-warning/30 bg-warning/10 text-warning">Not usable</Badge>;
  return <Badge variant="outline" className="border-destructive/20 bg-destructive/10 text-destructive">Unreachable</Badge>;
}

function SkeletonRows({ cols }: { cols: number }) {
  return (
    <>
      {[0, 1].map((r) => (
        <TableRow key={r}>
          {Array.from({ length: cols }, (_, c) => (
            <TableCell key={c} className="first:pl-6 last:pr-6">
              <Skeleton className="h-4" />
            </TableCell>
          ))}
        </TableRow>
      ))}
    </>
  );
}

function EmptyRow({ cols, text }: { cols: number; text: string }) {
  return (
    <TableRow className="hover:bg-transparent">
      <TableCell colSpan={cols} className="py-8 text-center text-muted-foreground">
        {text}
      </TableCell>
    </TableRow>
  );
}
