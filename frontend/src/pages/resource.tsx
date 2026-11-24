import { useState } from "react";
import { Play, RotateCw, Square, RefreshCw, ScrollText, Loader2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ConfirmButton, EmptyState, ErrorAlert, Field, LoadingRows, Page, StatusBadge, errorMessage } from "@/components/page";
import { EnvEditor } from "@/components/env-editor";
import { api } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";

/** Detail page for databases and services (applications have their own page with git + deployments). */
export type ResourceKind = "databases" | "services";
type ActionKind = ResourceKind | "applications";

interface Resource {
  uuid: string;
  name: string;
  status?: string | null;
  description?: string | null;
  image?: string | null;
  fqdn?: string | null;
}

interface ResourceDetailProps {
  kind: ResourceKind;
  uuid: string;
  onBack: () => void;
  backLabel?: string;
}

export function ResourceDetail({ kind, uuid, onBack, backLabel = "Back" }: ResourceDetailProps) {
  const { data: res, error, refresh } = usePolledData<Resource>(() => api.get(`/${kind}/${uuid}`), 8000);
  const noun = kind === "databases" ? "database" : "service";

  return (
    <Page
      back={{ label: backLabel, onClick: onBack }}
      title={
        <span className="flex items-center gap-3">
          {res?.name ?? "…"}
          {res && <StatusBadge status={res.status} />}
        </span>
      }
      description={res?.description || `${noun.charAt(0).toUpperCase()}${noun.slice(1)}`}
      actions={res && <ResourceActions kind={kind} uuid={uuid} noun={noun} onDone={refresh} />}
    >
      {error && <ErrorAlert message={error} />}
      {!res && !error && <LoadingRows rows={3} />}
      {res && (
        <Tabs defaultValue="overview">
          <TabsList>
            <TabsTrigger value="overview">Overview</TabsTrigger>
            <TabsTrigger value="environment">Environment</TabsTrigger>
            <TabsTrigger value="logs">Logs</TabsTrigger>
          </TabsList>
          <TabsContent value="overview" className="mt-4">
            <Card>
              <CardContent>
                <dl className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
                  <Field label="Status">
                    <StatusBadge status={res.status} />
                  </Field>
                  <Field label="Image" mono>
                    {res.image}
                  </Field>
                  <Field label="Domain" mono>
                    {res.fqdn}
                  </Field>
                  <Field label="UUID" mono>
                    {res.uuid}
                  </Field>
                </dl>
              </CardContent>
            </Card>
          </TabsContent>
          <TabsContent value="environment" className="mt-4">
            <EnvEditor kind={kind} uuid={uuid} />
          </TabsContent>
          <TabsContent value="logs" className="mt-4">
            <LogsPanel kind={kind} uuid={uuid} />
          </TabsContent>
        </Tabs>
      )}
    </Page>
  );
}

interface ResourceActionsProps {
  kind: ActionKind;
  uuid: string;
  noun: string;
  startLabel?: string;
  onDone: () => void | Promise<void>;
}

/** Start/Deploy, Restart, Stop. All three are POST in the API (GET returns "post required"). */
export function ResourceActions({ kind, uuid, noun, startLabel = "Start", onDone }: ResourceActionsProps) {
  const [busy, setBusy] = useState<string | null>(null);

  const run = async (verb: "start" | "restart" | "stop", label: string) => {
    setBusy(verb);
    try {
      await api.post(`/${kind}/${uuid}/${verb}`);
      toast.success(`${label} requested`, { description: `The ${noun} status updates in a few seconds.` });
      await onDone();
    } catch (e: unknown) {
      toast.error(`${label} failed`, { description: errorMessage(e) });
    } finally {
      setBusy(null);
    }
  };

  const spin = (verb: string, Icon: typeof Play) => (busy === verb ? <Loader2 className="animate-spin" /> : <Icon />);

  return (
    <>
      <ConfirmButton title={`Stop this ${noun}?`} description="Its containers are stopped. You can start it again later." confirmLabel="Stop" onConfirm={() => run("stop", "Stop")}>
        <Button variant="outline" disabled={!!busy}>
          {spin("stop", Square)} Stop
        </Button>
      </ConfirmButton>
      <Button variant="outline" disabled={!!busy} onClick={() => void run("restart", "Restart")}>
        {spin("restart", RotateCw)} Restart
      </Button>
      <Button disabled={!!busy} onClick={() => void run("start", startLabel)}>
        {spin("start", Play)} {startLabel}
      </Button>
    </>
  );
}

/** Container logs on demand (the API reads them over SSH, so they are fetched only when asked). */
export function LogsPanel({ kind, uuid }: { kind: ActionKind; uuid: string }) {
  const [logs, setLogs] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const body = await api.get<{ logs?: string; message?: string }>(`/${kind}/${uuid}/logs?lines=200`);
      setLogs(body.logs ?? body.message ?? "");
    } catch (e: unknown) {
      setError(errorMessage(e, "Could not read logs. The server may be unreachable."));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <p className="text-sm text-muted-foreground">Last 200 lines from the running container.</p>
        <Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}>
          {loading ? <Loader2 className="animate-spin" /> : <RefreshCw />} {logs === null ? "Load logs" : "Refresh"}
        </Button>
      </div>
      {error && <ErrorAlert message={error} />}
      {logs === null && !error && !loading && <EmptyState icon={ScrollText} title="Logs are loaded on demand" description="Reading logs connects to the server over SSH." />}
      {logs !== null && (
        <pre className="max-h-[32rem] overflow-auto rounded-xl border bg-neutral-950 p-4 font-mono text-xs leading-relaxed text-neutral-200">
          {logs || "No output."}
        </pre>
      )}
    </div>
  );
}
