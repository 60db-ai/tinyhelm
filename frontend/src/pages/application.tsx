import { GitBranch, Globe, Rocket, ExternalLink } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { DeploymentBadge, EmptyState, ErrorAlert, Field, LoadingRows, Page, StatusBadge, ToneBadge, timeAgo } from "@/components/page";
import { EnvEditor } from "@/components/env-editor";
import { LogsPanel, ResourceActions } from "@/pages/resource";
import { api } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";

interface Application {
  uuid: string;
  name: string;
  description?: string | null;
  status?: string | null;
  fqdn?: string | null;
  git_repository?: string | null;
  git_branch?: string | null;
  build_pack?: string | null;
  docker_registry_image_name?: string | null;
  docker_registry_image_tag?: string | null;
  ports_mappings?: string | null;
  ports_exposes?: string | null;
  base_directory?: string | null;
  health_check_path?: string | null;
  created_at?: string | null;
  updated_at?: string | null;
}

interface Deployment {
  deployment_uuid: string;
  status: string;
  commit?: string | null;
  commit_message?: string | null;
  created_at?: string | null;
  finished_at?: string | null;
}

interface ApplicationDetailProps {
  uuid: string;
  onBack: () => void;
  backLabel?: string;
}

export function ApplicationDetail({ uuid, onBack, backLabel = "Back" }: ApplicationDetailProps) {
  const { data: app, error, refresh } = usePolledData<Application>(() => api.get(`/applications/${uuid}`), 8000);

  return (
    <Page
      back={{ label: backLabel, onClick: onBack }}
      title={
        <span className="flex items-center gap-3">
          {app?.name ?? "…"}
          {app && <StatusBadge status={app.status} />}
        </span>
      }
      description={app?.description || "Application"}
      actions={app && <ResourceActions kind="applications" uuid={uuid} noun="application" startLabel="Deploy" onDone={refresh} />}
    >
      {error && <ErrorAlert message={error} />}
      {!app && !error && <LoadingRows rows={3} />}
      {app && (
        <Tabs defaultValue="overview">
          <TabsList>
            <TabsTrigger value="overview">Overview</TabsTrigger>
            <TabsTrigger value="environment">Environment</TabsTrigger>
            <TabsTrigger value="deployments">Deployments</TabsTrigger>
            <TabsTrigger value="logs">Logs</TabsTrigger>
          </TabsList>
          <TabsContent value="overview" className="mt-4">
            <Overview app={app} />
          </TabsContent>
          <TabsContent value="environment" className="mt-4">
            <EnvEditor kind="applications" uuid={uuid} />
          </TabsContent>
          <TabsContent value="deployments" className="mt-4">
            <Deployments uuid={uuid} />
          </TabsContent>
          <TabsContent value="logs" className="mt-4">
            <LogsPanel kind="applications" uuid={uuid} />
          </TabsContent>
        </Tabs>
      )}
    </Page>
  );
}

function Overview({ app }: { app: Application }) {
  const image = app.docker_registry_image_name ? `${app.docker_registry_image_name}:${app.docker_registry_image_tag || "latest"}` : null;
  const domains = (app.fqdn ?? "").split(",").map((d) => d.trim()).filter(Boolean);

  return (
    <div className="grid gap-4 lg:grid-cols-3">
      <Card className="lg:col-span-2">
        <CardHeader>
          <CardTitle className="text-sm">Source</CardTitle>
        </CardHeader>
        <CardContent>
          <dl className="grid gap-6 sm:grid-cols-2">
            <Field label="Repository" mono>
              {app.git_repository}
            </Field>
            <Field label="Branch">
              {app.git_branch && (
                <span className="inline-flex items-center gap-1.5">
                  <GitBranch className="size-3.5 text-muted-foreground" /> {app.git_branch}
                </span>
              )}
            </Field>
            <Field label="Build pack">{app.build_pack && <ToneBadge tone="neutral" dot={false}>{app.build_pack}</ToneBadge>}</Field>
            <Field label="Image" mono>
              {image}
            </Field>
            <Field label="Base directory" mono>
              {app.base_directory}
            </Field>
            <Field label="Health check" mono>
              {app.health_check_path}
            </Field>
          </dl>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-sm">Network</CardTitle>
        </CardHeader>
        <CardContent className="space-y-6">
          <div>
            <div className="text-xs text-muted-foreground">Domains</div>
            {domains.length === 0 ? (
              <p className="mt-1 text-sm text-muted-foreground">No domain configured</p>
            ) : (
              <ul className="mt-1 space-y-1">
                {domains.map((d) => (
                  <li key={d}>
                    <a href={d} target="_blank" rel="noreferrer" className="inline-flex items-center gap-1.5 font-mono text-[13px] hover:underline">
                      <Globe className="size-3.5 text-muted-foreground" /> {d.replace(/^https?:\/\//, "")} <ExternalLink className="size-3 text-muted-foreground" />
                    </a>
                  </li>
                ))}
              </ul>
            )}
          </div>
          <dl className="grid gap-6">
            <Field label="Port mappings" mono>
              {app.ports_mappings}
            </Field>
            <Field label="Exposed ports" mono>
              {app.ports_exposes}
            </Field>
            <Field label="Last updated">{timeAgo(app.updated_at)}</Field>
          </dl>
        </CardContent>
      </Card>
    </div>
  );
}

function Deployments({ uuid }: { uuid: string }) {
  const { data, error } = usePolledData<{ deployments?: Deployment[] }>(() => api.get(`/deployments/applications/${uuid}`), 8000);
  const rows = data?.deployments ?? null;

  if (error) return <ErrorAlert message={error} />;
  if (!rows) return <LoadingRows rows={3} />;
  if (rows.length === 0) return <EmptyState icon={Rocket} title="No deployments yet" description="Press Deploy to build and start this application." />;
  return (
    <Card className="gap-0 overflow-hidden py-0">
      <Table>
        <TableHeader>
          <TableRow className="hover:bg-transparent">
            <TableHead className="pl-5">Deployment</TableHead>
            <TableHead>Commit</TableHead>
            <TableHead>Started</TableHead>
            <TableHead className="pr-5 text-right">Status</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {rows.map((d) => (
            <TableRow key={d.deployment_uuid}>
              <TableCell className="pl-5 font-mono text-xs">{d.deployment_uuid}</TableCell>
              <TableCell className="max-w-64 truncate text-muted-foreground">
                {d.commit ? <span className="font-mono text-xs">{d.commit.slice(0, 7)}</span> : "—"}
                {d.commit_message && <span className="ml-2">{d.commit_message}</span>}
              </TableCell>
              <TableCell className="text-muted-foreground">{timeAgo(d.created_at)}</TableCell>
              <TableCell className="pr-5 text-right">
                <DeploymentBadge status={d.status} />
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Card>
  );
}
