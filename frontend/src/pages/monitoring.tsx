import { useState } from "react";
import { ChevronLeft, ChevronRight, Rocket, ScrollText, Server as ServerIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import {
  DeploymentBadge,
  EmptyState,
  ErrorAlert,
  LoadingRows,
  Page,
  SectionTitle,
  ToneBadge,
  timeAgo,
} from "@/components/page";
import { api } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";

interface DashboardData {
  servers: { uuid: string; name: string; ip: string; is_reachable: boolean | null; is_usable: boolean | null }[];
  deployments: {
    deployment_uuid: string;
    status: string;
    application_name: string | null;
    application_uuid: string;
    project: string;
    environment: string;
    created_at: string | null;
  }[];
}

interface AuditEvent {
  id: number;
  event?: string | null;
  action?: string | null;
  actor_name?: string | null;
  actor_email?: string | null;
  resource_type?: string | null;
  resource_name?: string | null;
  description?: string | null;
  created_at?: string | null;
}

interface AuditPage {
  data: AuditEvent[];
  current_page: number;
  last_page: number;
}

export function MonitoringPage() {
  const dash = usePolledData<DashboardData>(() => api.get("/dashboard"), 10000);
  const [auditPage, setAuditPage] = useState(1);

  return (
    <Page title="Monitoring" description="Server health, deployment activity, and the audit trail.">
      {dash.error && <ErrorAlert message={dash.error} />}

      <SectionTitle>Server health</SectionTitle>
      {!dash.data && !dash.error && <LoadingRows rows={2} />}
      {dash.data?.servers.length === 0 && <EmptyState icon={ServerIcon} title="No servers yet" />}
      {dash.data && dash.data.servers.length > 0 && (
        <Card className="py-0">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="pl-6">Server</TableHead>
                <TableHead>Address</TableHead>
                <TableHead>Reachable</TableHead>
                <TableHead className="pr-6">Usable</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {dash.data.servers.map((s) => (
                <TableRow key={s.uuid}>
                  <TableCell className="pl-6 font-medium">{s.name}</TableCell>
                  <TableCell className="font-mono text-xs text-muted-foreground">{s.ip}</TableCell>
                  <TableCell>
                    <ToneBadge tone={s.is_reachable ? "success" : "danger"}>{s.is_reachable ? "Yes" : "No"}</ToneBadge>
                  </TableCell>
                  <TableCell className="pr-6">
                    <ToneBadge tone={s.is_usable ? "success" : "danger"}>{s.is_usable ? "Yes" : "No"}</ToneBadge>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}

      <SectionTitle>Recent deployments</SectionTitle>
      {!dash.data && !dash.error && <LoadingRows rows={2} />}
      {dash.data?.deployments.length === 0 && <EmptyState icon={Rocket} title="No deployments yet" />}
      {dash.data && dash.data.deployments.length > 0 && (
        <Card className="py-0">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="pl-6">Application</TableHead>
                <TableHead>Project / environment</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="pr-6 text-right">Started</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {dash.data.deployments.map((d) => (
                <TableRow key={d.deployment_uuid}>
                  <TableCell className="pl-6 font-medium">{d.application_name || d.application_uuid}</TableCell>
                  <TableCell className="text-muted-foreground">
                    {d.project} · {d.environment}
                  </TableCell>
                  <TableCell>
                    <DeploymentBadge status={d.status} />
                  </TableCell>
                  <TableCell className="pr-6 text-right text-muted-foreground">{timeAgo(d.created_at)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}

      {/* Keyed on page so the polled hook remounts and fetches the new page immediately. */}
      <AuditLog key={auditPage} page={auditPage} onPage={setAuditPage} />
    </Page>
  );
}

function AuditLog({ page, onPage }: { page: number; onPage: (page: number) => void }) {
  const { data, error } = usePolledData<AuditPage>(() => api.get(`/audit-events?page=${page}`), 15000);

  return (
    <>
      <SectionTitle
        actions={
          data && data.last_page > 1 ? (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <Button variant="outline" size="icon" className="size-7" disabled={page <= 1} onClick={() => onPage(page - 1)} aria-label="Previous page">
                <ChevronLeft />
              </Button>
              {page} / {data.last_page}
              <Button
                variant="outline"
                size="icon"
                className="size-7"
                disabled={page >= data.last_page}
                onClick={() => onPage(page + 1)}
                aria-label="Next page"
              >
                <ChevronRight />
              </Button>
            </div>
          ) : undefined
        }
      >
        Audit events
      </SectionTitle>
      {error && <ErrorAlert message={error} />}
      {!data && !error && <LoadingRows rows={2} />}
      {data?.data.length === 0 && (
        <EmptyState icon={ScrollText} title="No audit events yet" description="Changes made through the UI and API are recorded here." />
      )}
      {data && data.data.length > 0 && (
        <Card className="py-0">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="pl-6">Event</TableHead>
                <TableHead>Resource</TableHead>
                <TableHead>Actor</TableHead>
                <TableHead className="pr-6 text-right">When</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.data.map((e) => (
                <TableRow key={e.id}>
                  <TableCell className="pl-6">
                    <div className="font-medium">{e.event || e.action}</div>
                    {e.description && <div className="max-w-md truncate text-xs text-muted-foreground">{e.description}</div>}
                  </TableCell>
                  <TableCell className="text-muted-foreground">
                    {e.resource_name || "—"}
                    {e.resource_type && <span className="ml-1 text-xs">({e.resource_type.split("\\").pop()})</span>}
                  </TableCell>
                  <TableCell className="text-muted-foreground">{e.actor_name || e.actor_email || "System"}</TableCell>
                  <TableCell className="pr-6 text-right text-muted-foreground">{timeAgo(e.created_at)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </>
  );
}
