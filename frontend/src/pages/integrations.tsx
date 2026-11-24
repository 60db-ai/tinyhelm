import type { ComponentType } from "react";
import { Github, Gitlab, HardDrive, Cloud, Globe, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Page, ErrorAlert, EmptyState, LoadingRows, ConfirmButton, timeAgo, errorMessage } from "@/components/page";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";

type Row = Record<string, unknown>;

interface Section {
  title: string;
  description: string;
  icon: ComponentType<{ className?: string }>;
  path: string;
  /** Non-secret column shown next to the name. Never read key/secret/token fields here. */
  detail: (r: Row) => string;
  deletable?: boolean;
}

function str(v: unknown): string {
  return typeof v === "string" ? v : "";
}

const SECTIONS: Section[] = [
  {
    title: "GitHub apps",
    description: "Private repositories and automatic deploys from GitHub.",
    icon: Github,
    path: "/github-apps",
    detail: (r) => str(r.organization) || str(r.html_url) || str(r.api_url),
  },
  {
    title: "GitLab apps",
    description: "Private repositories and webhooks from GitLab.",
    icon: Gitlab,
    path: "/gitlab-apps",
    detail: (r) => str(r.group_name) || str(r.html_url) || str(r.api_url),
  },
  {
    title: "S3 storages",
    description: "Destinations for database and volume backups.",
    icon: HardDrive,
    path: "/s3-storages",
    detail: (r) => [str(r.bucket), str(r.endpoint) || str(r.region)].filter(Boolean).join(" · "),
    deletable: true,
  },
  {
    title: "Cloud provider tokens",
    description: "Hetzner, DigitalOcean, Vultr and other providers used to create servers.",
    icon: Cloud,
    path: "/cloud-tokens",
    detail: (r) => str(r.provider),
    deletable: true,
  },
  {
    title: "DNS & integration tokens",
    description: "Tokens for DNS providers and other third-party integrations.",
    icon: Globe,
    path: "/security/integration-tokens",
    detail: (r) => str(r.provider) || str(r.type),
  },
];

export function IntegrationsPage() {
  return (
    <Page title="Integrations" description="Git sources, storage, and cloud providers connected to this team.">
      {SECTIONS.map((s) => (
        <IntegrationSection key={s.path} section={s} />
      ))}
      <p className="text-xs text-muted-foreground">
        Connecting a new integration needs a provider-specific setup flow. It comes to this page next; existing connections are listed above.
      </p>
    </Page>
  );
}

function IntegrationSection({ section }: { section: Section }) {
  const { data, error, refresh } = usePolledData<Row[]>(() => api.get<Row[]>(section.path), 30000);
  const Icon = section.icon;

  const remove = async (uuid: string, name: string) => {
    try {
      await api.delete(`${section.path}/${uuid}`);
      toast.success(`${name} removed`);
      await refresh();
    } catch (e: unknown) {
      toast.error(errorMessage(e, "Failed to remove"));
    }
  };

  return (
    <Card className="gap-0 py-0">
      <CardHeader className="border-b px-6 py-4 [.border-b]:pb-4">
        <CardTitle className="flex items-center gap-2 text-sm">
          <Icon className="size-4" /> {section.title}
          {data && <span className="font-normal text-muted-foreground">{data.length}</span>}
        </CardTitle>
        <CardDescription>{section.description}</CardDescription>
      </CardHeader>
      <CardContent className="p-0">
        {error && (
          <div className="p-4">
            <ErrorAlert message={error} />
          </div>
        )}
        {!data && !error && <LoadingRows rows={2} className="p-4" />}
        {data?.length === 0 && (
          <div className="p-4">
            <EmptyState icon={Icon} title={`No ${section.title.toLowerCase()} yet`} />
          </div>
        )}
        {data && data.length > 0 && (
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="pl-6">Name</TableHead>
                <TableHead>Details</TableHead>
                <TableHead>Added</TableHead>
                {section.deletable && <TableHead className="w-12 pr-6" />}
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.map((r, i) => {
                const uuid = str(r.uuid);
                const name = str(r.name) || str(r.app_name) || uuid || `#${i + 1}`;
                return (
                  <TableRow key={uuid || String(r.id ?? i)}>
                    <TableCell className="pl-6 font-medium">{name}</TableCell>
                    <TableCell className="text-muted-foreground">{section.detail(r) || "—"}</TableCell>
                    <TableCell className="text-muted-foreground">{timeAgo(str(r.created_at) || null)}</TableCell>
                    {section.deletable && (
                      <TableCell className="pr-6 text-right">
                        {uuid && (
                          <ConfirmButton
                            title={`Remove ${name}?`}
                            description="Resources that use it will lose access. This cannot be undone."
                            confirmLabel="Remove"
                            onConfirm={() => remove(uuid, name)}
                          >
                            <Button
                              variant="ghost"
                              size="icon"
                              className="size-8 text-muted-foreground hover:text-destructive"
                              aria-label={`Remove ${name}`}
                            >
                              <Trash2 />
                            </Button>
                          </ConfirmButton>
                        )}
                      </TableCell>
                    )}
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        )}
      </CardContent>
    </Card>
  );
}
