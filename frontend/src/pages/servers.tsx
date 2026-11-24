import { useState } from "react";
import { Loader2, PlugZap, RefreshCw, Server as ServerIcon } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import {
  EmptyState,
  ErrorAlert,
  Field,
  LoadingRows,
  Page,
  SectionTitle,
  StatusBadge,
  ToneBadge,
  errorMessage,
} from "@/components/page";
import { api } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";

interface ServerRow {
  uuid: string;
  name: string;
  ip: string;
  port?: number;
  user?: string;
  description?: string | null;
  is_reachable?: boolean | null;
  is_usable?: boolean | null;
  settings?: { is_reachable?: boolean; is_usable?: boolean } | null;
}

interface ServerResource {
  id?: number;
  uuid: string;
  name: string;
  type: string;
  status?: string | null;
}

/** Native gateway returns flat flags; Laravel nests them under `settings`. */
function health(s: ServerRow): { reachable: boolean; usable: boolean } {
  return {
    reachable: !!(s.is_reachable ?? s.settings?.is_reachable),
    usable: !!(s.is_usable ?? s.settings?.is_usable),
  };
}

function ServerHealth({ server }: { server: ServerRow }) {
  const { reachable, usable } = health(server);
  if (reachable && usable) return <ToneBadge tone="success">Healthy</ToneBadge>;
  if (reachable) return <ToneBadge tone="warning">Not usable</ToneBadge>;
  return <ToneBadge tone="danger">Unreachable</ToneBadge>;
}

export function ServersPage() {
  const [selected, setSelected] = useState<string | null>(null);
  if (selected) return <ServerDetail uuid={selected} onBack={() => setSelected(null)} />;
  return <ServerList onOpen={setSelected} />;
}

function ServerList({ onOpen }: { onOpen: (uuid: string) => void }) {
  const { data, error, refresh } = usePolledData<ServerRow[]>(() => api.get("/servers"), 8000);

  return (
    <Page
      title="Servers"
      description="Hosts TinyHelm manages over SSH."
      actions={
        <Button variant="outline" size="sm" onClick={() => void refresh()}>
          <RefreshCw /> Refresh
        </Button>
      }
    >
      {error && <ErrorAlert message={error} />}
      {!data && !error && <LoadingRows />}
      {data?.length === 0 && (
        <EmptyState icon={ServerIcon} title="No servers yet" description="Add a server to start deploying applications to it." />
      )}
      {data && data.length > 0 && (
        <Card className="py-0">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="pl-6">Name</TableHead>
                <TableHead>Address</TableHead>
                <TableHead>SSH</TableHead>
                <TableHead className="pr-6 text-right">Status</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.map((s) => (
                <TableRow key={s.uuid} className="cursor-pointer" onClick={() => onOpen(s.uuid)}>
                  <TableCell className="pl-6">
                    <div className="font-medium">{s.name}</div>
                    {s.description && <div className="text-xs text-muted-foreground">{s.description}</div>}
                  </TableCell>
                  <TableCell className="font-mono text-xs text-muted-foreground">{s.ip}</TableCell>
                  <TableCell className="font-mono text-xs text-muted-foreground">
                    {s.user ?? "root"}:{s.port ?? 22}
                  </TableCell>
                  <TableCell className="pr-6 text-right">
                    <ServerHealth server={s} />
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </Page>
  );
}

function ServerDetail({ uuid, onBack }: { uuid: string; onBack: () => void }) {
  const server = usePolledData<ServerRow>(() => api.get(`/servers/${uuid}`), 8000);
  const resources = usePolledData<ServerResource[]>(() => api.get(`/servers/${uuid}/resources`), 10000);
  const [validating, setValidating] = useState(false);

  const validate = async () => {
    setValidating(true);
    try {
      const res = await api.post<{ message?: string }>(`/servers/${uuid}/validate`);
      toast.success(res?.message ?? "Validation started");
      await server.refresh();
    } catch (e: unknown) {
      toast.error(errorMessage(e, "Validation failed"));
    } finally {
      setValidating(false);
    }
  };

  const s = server.data;
  return (
    <Page
      back={{ label: "Servers", onClick: onBack }}
      title={
        <span className="flex items-center gap-3">
          {s?.name ?? "Server"}
          {s && <ServerHealth server={s} />}
        </span>
      }
      description={s?.description || undefined}
      actions={
        <Button variant="outline" size="sm" disabled={validating} onClick={() => void validate()}>
          {validating ? <Loader2 className="animate-spin" /> : <PlugZap />} Validate connection
        </Button>
      }
    >
      {server.error && <ErrorAlert message={server.error} />}
      {!s && !server.error && <LoadingRows rows={2} />}
      {s && (
        <Card>
          <CardContent>
            <dl className="grid grid-cols-2 gap-6 md:grid-cols-5">
              <Field label="IP address" mono>{s.ip}</Field>
              <Field label="Port" mono>{String(s.port ?? 22)}</Field>
              <Field label="User" mono>{s.user ?? "root"}</Field>
              <Field label="Description">{s.description}</Field>
              <Field label="UUID" mono>{s.uuid}</Field>
            </dl>
          </CardContent>
        </Card>
      )}

      <SectionTitle>Resources</SectionTitle>
      {resources.error && <ErrorAlert message={resources.error} />}
      {!resources.data && !resources.error && <LoadingRows rows={2} />}
      {resources.data?.length === 0 && (
        <EmptyState icon={ServerIcon} title="No resources on this server" description="Applications, databases, and services deployed here appear in this list." />
      )}
      {resources.data && resources.data.length > 0 && (
        <Card className="py-0">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="pl-6">Name</TableHead>
                <TableHead>Type</TableHead>
                <TableHead className="pr-6 text-right">Status</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {resources.data.map((r) => (
                <TableRow key={r.uuid}>
                  <TableCell className="pl-6 font-medium">{r.name}</TableCell>
                  <TableCell className="text-muted-foreground capitalize">{r.type.replace(/^standalone-/, "")}</TableCell>
                  <TableCell className="pr-6 text-right">
                    <StatusBadge status={r.status} />
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </Page>
  );
}
