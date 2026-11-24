import { useEffect, useState } from "react";
import { ArrowLeft, Boxes, Layers, AppWindow, Database, Container } from "lucide-react";
import { api, ApiError } from "@/lib/api";
import { ApplicationDetail } from "@/pages/application";
import { ResourceDetail } from "@/pages/resource";

type Sel = { kind: "application" | "databases" | "services"; uuid: string };

// Small state helpers (kept local; promote to a shared module when a 3rd page needs them).
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
  return (
    <div className="space-y-2">
      {[0, 1, 2].map((i) => (
        <div key={i} className="h-16 animate-pulse rounded-[var(--radius)] border bg-[var(--color-card)]" />
      ))}
    </div>
  );
}
function Dot({ status }: { status?: string | null }) {
  const color =
    status == null
      ? "var(--color-muted-foreground)"
      : status.startsWith("running")
        ? "var(--color-success)"
        : status.startsWith("exited") || status.includes("unhealthy")
          ? "var(--color-destructive)"
          : "var(--color-muted-foreground)";
  return <span className="inline-block h-2 w-2 shrink-0 rounded-full" style={{ background: color }} />;
}

interface Project {
  uuid: string;
  name: string;
  description?: string | null;
}
interface Environment {
  uuid: string;
  name: string;
}
interface Resource {
  uuid: string;
  name: string;
  status?: string | null;
}
// Env payload: applications[], services[], and one array per database engine.
const DB_KEYS = [
  "postgresqls",
  "mysqls",
  "mariadbs",
  "mongodbs",
  "redis",
  "keydbs",
  "dragonflies",
  "clickhouses",
  "sqlites",
] as const;
type EnvDetail = {
  applications?: Resource[];
  services?: Resource[];
} & Partial<Record<(typeof DB_KEYS)[number], Resource[]>>;

export function ProjectsPage() {
  const [project, setProject] = useState<Project | null>(null);
  const [env, setEnv] = useState<Environment | null>(null);
  const [sel, setSel] = useState<Sel | null>(null);

  if (project && env && sel) {
    return sel.kind === "application" ? (
      <ApplicationDetail uuid={sel.uuid} onBack={() => setSel(null)} backLabel={env.name} />
    ) : (
      <ResourceDetail kind={sel.kind} uuid={sel.uuid} onBack={() => setSel(null)} backLabel={env.name} />
    );
  }
  if (project && env) {
    return (
      <EnvironmentDetail
        project={project}
        env={env}
        onBack={() => setEnv(null)}
        onOpen={(kind, uuid) => setSel({ kind, uuid })}
      />
    );
  }
  if (project) {
    return <EnvironmentList project={project} onOpen={setEnv} onBack={() => setProject(null)} />;
  }
  return <ProjectList onOpen={setProject} />;
}

function ProjectList({ onOpen }: { onOpen: (p: Project) => void }) {
  const [rows, setRows] = useState<Project[] | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api
      .get<Project[]>("/projects")
      .then(setRows)
      .catch((e: unknown) => setErr(e instanceof ApiError ? e.message : "Failed to load projects"));
  }, []);

  return (
    <div className="mx-auto max-w-5xl space-y-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Projects</h1>
        <p className="mt-1 text-sm text-[var(--color-muted-foreground)]">Group environments and resources.</p>
      </div>
      {err && <ErrorBox message={err} />}
      {!err && rows === null && <Skeleton />}
      {!err && rows?.length === 0 && <EmptyBox message="No projects yet." />}
      {rows && rows.length > 0 && (
        <div className="divide-y overflow-hidden rounded-[var(--radius)] border bg-[var(--color-card)]">
          {rows.map((p) => (
            <Row key={p.uuid} icon={<Boxes size={18} />} title={p.name} subtitle={p.description} onClick={() => onOpen(p)} />
          ))}
        </div>
      )}
    </div>
  );
}

function EnvironmentList({
  project,
  onOpen,
  onBack,
}: {
  project: Project;
  onOpen: (e: Environment) => void;
  onBack: () => void;
}) {
  const [rows, setRows] = useState<Environment[] | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api
      .get<Environment[]>(`/projects/${project.uuid}/environments`)
      .then(setRows)
      .catch((e: unknown) => setErr(e instanceof ApiError ? e.message : "Failed to load environments"));
  }, [project.uuid]);

  return (
    <div className="mx-auto max-w-5xl space-y-6">
      <Back onClick={onBack} label="Projects" />
      <h1 className="text-2xl font-semibold tracking-tight">{project.name}</h1>
      {err && <ErrorBox message={err} />}
      {!err && rows === null && <Skeleton />}
      {!err && rows?.length === 0 && <EmptyBox message="No environments." />}
      {rows && rows.length > 0 && (
        <div className="divide-y overflow-hidden rounded-[var(--radius)] border bg-[var(--color-card)]">
          {rows.map((e) => (
            <Row key={e.uuid} icon={<Layers size={18} />} title={e.name} onClick={() => onOpen(e)} />
          ))}
        </div>
      )}
    </div>
  );
}

function EnvironmentDetail({
  project,
  env,
  onBack,
  onOpen,
}: {
  project: Project;
  env: Environment;
  onBack: () => void;
  onOpen: (kind: Sel["kind"], uuid: string) => void;
}) {
  const [data, setData] = useState<EnvDetail | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api
      .get<EnvDetail>(`/projects/${project.uuid}/${env.uuid}`)
      .then(setData)
      .catch((e: unknown) => setErr(e instanceof ApiError ? e.message : "Failed to load environment"));
  }, [project.uuid, env.uuid]);

  const databases = data ? DB_KEYS.flatMap((k) => data[k] ?? []) : [];

  return (
    <div className="mx-auto max-w-5xl space-y-6">
      <Back onClick={onBack} label={project.name} />
      <h1 className="text-2xl font-semibold tracking-tight">
        {project.name} <span className="text-[var(--color-muted-foreground)]">/ {env.name}</span>
      </h1>
      {err && <ErrorBox message={err} />}
      {!err && data === null && <Skeleton />}
      {data && (
        <>
          <Group icon={<AppWindow size={16} />} title="Applications" items={data.applications ?? []} onOpen={(u) => onOpen("application", u)} />
          <Group icon={<Database size={16} />} title="Databases" items={databases} onOpen={(u) => onOpen("databases", u)} />
          <Group icon={<Container size={16} />} title="Services" items={data.services ?? []} onOpen={(u) => onOpen("services", u)} />
        </>
      )}
    </div>
  );
}

function Group({
  icon,
  title,
  items,
  onOpen,
}: {
  icon: React.ReactNode;
  title: string;
  items: Resource[];
  onOpen?: (uuid: string) => void;
}) {
  return (
    <section className="space-y-2">
      <h2 className="flex items-center gap-2 text-sm font-medium text-[var(--color-muted-foreground)]">
        {icon} {title} <span className="tabular-nums">({items.length})</span>
      </h2>
      {items.length ? (
        <div className="divide-y overflow-hidden rounded-[var(--radius)] border bg-[var(--color-card)]">
          {items.map((r) => {
            const inner = (
              <>
                <Dot status={r.status} />
                <div className="min-w-0 flex-1 truncate font-medium">{r.name}</div>
                <span className="hidden text-sm text-[var(--color-muted-foreground)] sm:block">{r.status ?? "—"}</span>
              </>
            );
            return onOpen ? (
              <button
                key={r.uuid}
                onClick={() => onOpen(r.uuid)}
                className="flex w-full items-center gap-4 px-5 py-3 text-left transition-colors hover:bg-[var(--color-elevated)]"
              >
                {inner}
              </button>
            ) : (
              <div key={r.uuid} className="flex items-center gap-4 px-5 py-3">
                {inner}
              </div>
            );
          })}
        </div>
      ) : (
        <EmptyBox message={`No ${title.toLowerCase()}.`} />
      )}
    </section>
  );
}

function Row({
  icon,
  title,
  subtitle,
  onClick,
}: {
  icon: React.ReactNode;
  title: string;
  subtitle?: string | null;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="flex w-full items-center gap-4 px-5 py-4 text-left transition-colors hover:bg-[var(--color-elevated)]"
    >
      <span className="shrink-0 text-[var(--color-muted-foreground)]">{icon}</span>
      <div className="min-w-0 flex-1">
        <div className="truncate font-medium">{title}</div>
        {subtitle && <div className="truncate text-sm text-[var(--color-muted-foreground)]">{subtitle}</div>}
      </div>
    </button>
  );
}

function Back({ onClick, label }: { onClick: () => void; label: string }) {
  return (
    <button
      onClick={onClick}
      className="flex items-center gap-2 text-sm text-[var(--color-muted-foreground)] transition-colors hover:text-[var(--color-foreground)]"
    >
      <ArrowLeft size={16} /> {label}
    </button>
  );
}
