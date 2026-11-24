import { useState, type ComponentType } from "react";
import { AppWindow, Boxes, Container, Database, Plus, Trash2, ChevronRight, Layers } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Card } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { ConfirmButton, EmptyState, ErrorAlert, LoadingRows, Page, StatusBadge, errorMessage } from "@/components/page";
import { api } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";
import { ApplicationDetail } from "@/pages/application";
import { ResourceDetail } from "@/pages/resource";

interface Project {
  uuid: string;
  name: string;
  description?: string | null;
}

interface DashProject extends Project {
  environments: number;
  resources: number;
}

interface Environment {
  uuid: string;
  name: string;
}

interface ResourceItem {
  uuid: string;
  name: string;
  status?: string | null;
  fqdn?: string | null;
}

const DB_KEYS = ["postgresqls", "mysqls", "mariadbs", "mongodbs", "redis", "keydbs", "dragonflies", "clickhouses", "sqlites"] as const;
const DB_LABEL: Record<(typeof DB_KEYS)[number], string> = {
  postgresqls: "PostgreSQL",
  mysqls: "MySQL",
  mariadbs: "MariaDB",
  mongodbs: "MongoDB",
  redis: "Redis",
  keydbs: "KeyDB",
  dragonflies: "Dragonfly",
  clickhouses: "ClickHouse",
  sqlites: "SQLite",
};

type EnvDetail = { applications?: ResourceItem[]; services?: ResourceItem[] } & Partial<Record<(typeof DB_KEYS)[number], ResourceItem[]>>;

type ResourceKind = "application" | "services" | "databases";

interface Row extends ResourceItem {
  kind: ResourceKind;
  type: string;
  icon: ComponentType<{ className?: string }>;
}

type View =
  | { name: "list" }
  | { name: "project"; project: Project }
  | { name: "resource"; project: Project; kind: ResourceKind; uuid: string };

export function ProjectsPage() {
  const [view, setView] = useState<View>({ name: "list" });

  if (view.name === "resource") {
    const back = () => setView({ name: "project", project: view.project });
    return view.kind === "application" ? (
      <ApplicationDetail uuid={view.uuid} onBack={back} backLabel={view.project.name} />
    ) : (
      <ResourceDetail kind={view.kind} uuid={view.uuid} onBack={back} backLabel={view.project.name} />
    );
  }
  if (view.name === "project") {
    return (
      <ProjectDetail
        project={view.project}
        onBack={() => setView({ name: "list" })}
        onOpen={(kind, uuid) => setView({ name: "resource", project: view.project, kind, uuid })}
      />
    );
  }
  return <ProjectList onOpen={(project) => setView({ name: "project", project })} />;
}

function ProjectList({ onOpen }: { onOpen: (p: Project) => void }) {
  // /dashboard carries per-project environment and resource counts in one call.
  const { data, error, refresh } = usePolledData<{ projects: DashProject[] }>(() => api.get("/dashboard"), 15000);

  return (
    <Page title="Projects" description="Group environments and the resources deployed into them." actions={<CreateProjectDialog onCreated={refresh} />}>
      {error && <ErrorAlert message={error} />}
      {!data && !error && <LoadingRows rows={2} />}
      {data?.projects.length === 0 && (
        <EmptyState icon={Boxes} title="No projects yet" description="Create a project to start deploying applications, databases and services." action={<CreateProjectDialog onCreated={refresh} />} />
      )}
      {!!data?.projects.length && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {data.projects.map((p) => (
            <button
              key={p.uuid}
              onClick={() => onOpen(p)}
              className="group flex flex-col rounded-2xl border bg-card p-5 text-left transition-all hover:-translate-y-0.5 hover:shadow-md focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
            >
              <div className="flex items-start justify-between">
                <span className="flex size-10 items-center justify-center rounded-xl bg-gradient-to-br from-violet-500/20 to-orange-400/20 text-sm font-semibold">
                  {p.name.slice(0, 1).toUpperCase()}
                </span>
                <ChevronRight className="size-4 text-muted-foreground transition-transform group-hover:translate-x-0.5" />
              </div>
              <div className="mt-5 font-medium">{p.name}</div>
              <div className="mt-1 line-clamp-2 min-h-10 text-sm text-muted-foreground">{p.description || "No description"}</div>
              <div className="mt-4 flex gap-4 border-t pt-3 text-xs text-muted-foreground">
                <span className="flex items-center gap-1.5">
                  <Layers className="size-3.5" /> {p.environments} {p.environments === 1 ? "environment" : "environments"}
                </span>
                <span className="flex items-center gap-1.5">
                  <Container className="size-3.5" /> {p.resources} {p.resources === 1 ? "resource" : "resources"}
                </span>
              </div>
            </button>
          ))}
        </div>
      )}
    </Page>
  );
}

function CreateProjectDialog({ onCreated }: { onCreated: () => void | Promise<void> }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.post("/projects", { name: name.trim(), description: description.trim() || null });
      toast.success(`Created ${name.trim()}`);
      setName("");
      setDescription("");
      setOpen(false);
      await onCreated();
    } catch (e: unknown) {
      setError(errorMessage(e, "Could not create project"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>
          <Plus /> New project
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>New project</DialogTitle>
          <DialogDescription>A production environment is created with it.</DialogDescription>
        </DialogHeader>
        <form
          className="space-y-4"
          onSubmit={(e) => {
            e.preventDefault();
            if (name.trim()) void submit();
          }}
        >
          <div className="space-y-2">
            <Label htmlFor="project-name">Name</Label>
            <Input id="project-name" autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="My SaaS" />
          </div>
          <div className="space-y-2">
            <Label htmlFor="project-description">Description</Label>
            <Textarea id="project-description" value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Optional" rows={3} />
          </div>
          {error && <ErrorAlert message={error} />}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={!name.trim() || busy}>
              Create project
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

interface ProjectDetailProps {
  project: Project;
  onBack: () => void;
  onOpen: (kind: ResourceKind, uuid: string) => void;
}

function ProjectDetail({ project, onBack, onOpen }: ProjectDetailProps) {
  const { data: envs, error } = usePolledData<Environment[]>(() => api.get(`/projects/${project.uuid}/environments`), 30000);
  const [tab, setTab] = useState<string | null>(null);
  const active = tab ?? envs?.[0]?.uuid ?? null;

  const remove = async () => {
    try {
      await api.delete(`/projects/${project.uuid}`);
      toast.success(`Deleted ${project.name}`);
      onBack();
    } catch (e: unknown) {
      toast.error(errorMessage(e, "Delete failed"));
    }
  };

  return (
    <Page
      title={project.name}
      description={project.description || "No description"}
      back={{ label: "Projects", onClick: onBack }}
      actions={
        <ConfirmButton title={`Delete ${project.name}?`} description="Only empty projects can be deleted. This cannot be undone." confirmLabel="Delete project" onConfirm={remove}>
          <Button variant="outline" className="text-muted-foreground hover:text-destructive">
            <Trash2 /> Delete
          </Button>
        </ConfirmButton>
      }
    >
      {error && <ErrorAlert message={error} />}
      {!envs && !error && <LoadingRows rows={3} />}
      {envs?.length === 0 && <EmptyState icon={Layers} title="No environments" description="This project has no environments." />}
      {envs && active && (
        <Tabs value={active} onValueChange={setTab}>
          <TabsList>
            {envs.map((e) => (
              <TabsTrigger key={e.uuid} value={e.uuid} className="capitalize">
                {e.name}
              </TabsTrigger>
            ))}
          </TabsList>
          {envs.map((e) => (
            <TabsContent key={e.uuid} value={e.uuid} className="mt-4">
              {active === e.uuid && <EnvironmentResources project={project} env={e} onOpen={onOpen} />}
            </TabsContent>
          ))}
        </Tabs>
      )}
    </Page>
  );
}

function toRows(detail: EnvDetail): Row[] {
  const apps: Row[] = (detail.applications ?? []).map((r) => ({ ...r, kind: "application", type: "Application", icon: AppWindow }));
  const services: Row[] = (detail.services ?? []).map((r) => ({ ...r, kind: "services", type: "Service", icon: Container }));
  const dbs: Row[] = DB_KEYS.flatMap((k) => (detail[k] ?? []).map((r) => ({ ...r, kind: "databases" as const, type: DB_LABEL[k], icon: Database })));
  return [...apps, ...services, ...dbs];
}

function EnvironmentResources({ project, env, onOpen }: { project: Project; env: Environment; onOpen: ProjectDetailProps["onOpen"] }) {
  const { data, error } = usePolledData<EnvDetail>(() => api.get(`/projects/${project.uuid}/${env.uuid}`), 10000);
  const rows = data ? toRows(data) : null;

  if (error) return <ErrorAlert message={error} />;
  if (!rows) return <LoadingRows rows={3} />;
  if (rows.length === 0) {
    return <EmptyState icon={Container} title="No resources in this environment" description="Applications, databases and services you deploy here show up in this list." />;
  }
  return (
    <Card className="gap-0 overflow-hidden py-0">
      <Table>
        <TableHeader>
          <TableRow className="hover:bg-transparent">
            <TableHead className="pl-5">Resource</TableHead>
            <TableHead>Type</TableHead>
            <TableHead>Domain</TableHead>
            <TableHead className="pr-5 text-right">Status</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {rows.map(({ icon: Icon, ...r }) => (
            <TableRow key={`${r.kind}-${r.uuid}`} className="cursor-pointer" onClick={() => onOpen(r.kind, r.uuid)}>
              <TableCell className="pl-5">
                <div className="flex items-center gap-3">
                  <span className="flex size-8 items-center justify-center rounded-lg border bg-muted/50">
                    <Icon className="size-4 text-muted-foreground" />
                  </span>
                  <span className="font-medium">{r.name}</span>
                </div>
              </TableCell>
              <TableCell className="text-muted-foreground">{r.type}</TableCell>
              <TableCell className="max-w-56 truncate font-mono text-xs text-muted-foreground">{r.fqdn || "—"}</TableCell>
              <TableCell className="pr-5 text-right">
                <StatusBadge status={r.status} />
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Card>
  );
}
