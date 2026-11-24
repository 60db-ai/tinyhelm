import { useState } from "react";
import { toast } from "sonner";
import { Users } from "lucide-react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Page, ErrorAlert, EmptyState, LoadingRows, ToneBadge, Field, timeAgo, errorMessage } from "@/components/page";
import { api } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";

interface Team {
  id: number;
  name: string;
  description: string | null;
  personal_team: boolean;
  is_build_server_fallback_enabled: boolean;
  scan_images_on_new_servers: boolean;
  created_at: string | null;
}

// Only safe display fields are typed; credential/2FA fields from the API are never read.
interface Member {
  id: number;
  name: string;
  email: string;
  email_verified_at: string | null;
  created_at: string | null;
}

// PATCH /team accepts only these two booleans (TeamController::update_current_team); name is read-only via API.
type TeamToggle = "is_build_server_fallback_enabled" | "scan_images_on_new_servers";

const TOGGLES: { field: TeamToggle; label: string; hint: string }[] = [
  { field: "is_build_server_fallback_enabled", label: "Build server fallback", hint: "Build on the deploy server when no build server is available." },
  { field: "scan_images_on_new_servers", label: "Scan images on new servers", hint: "Enable vulnerability scanning on servers added from now on." },
];

function initials(name: string): string {
  return name
    .split(/\s+/)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("");
}

export function TeamPage() {
  const team = usePolledData<Team>(() => api.get("/teams/current"), 30000);
  const members = usePolledData<Member[]>(() => api.get("/teams/current/members"), 30000);

  return (
    <Page title="Team" description="Your workspace and the people in it.">
      {team.error && <ErrorAlert message={team.error} />}
      {!team.data && !team.error && <LoadingRows rows={1} />}
      {team.data && <TeamCard team={team.data} onChanged={team.refresh} />}

      <Card className="gap-0 overflow-hidden py-0">
        <CardHeader className="border-b px-6 py-4 [.border-b]:pb-4">
          <CardTitle className="text-sm">Members</CardTitle>
          <CardDescription>{members.data ? `${members.data.length} ${members.data.length === 1 ? "person" : "people"}` : " "}</CardDescription>
        </CardHeader>
        {members.error && (
          <div className="p-4">
            <ErrorAlert message={members.error} />
          </div>
        )}
        {!members.data && !members.error && <LoadingRows className="p-4" rows={2} />}
        {members.data?.length === 0 && (
          <div className="p-6">
            <EmptyState icon={Users} title="No members" />
          </div>
        )}
        {members.data && members.data.length > 0 && (
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="pl-6">Name</TableHead>
                <TableHead>Email</TableHead>
                <TableHead className="pr-6 text-right">Joined</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {members.data.map((m) => (
                <TableRow key={m.id}>
                  <TableCell className="pl-6">
                    <div className="flex items-center gap-3">
                      <Avatar className="size-8">
                        <AvatarFallback className="bg-muted text-xs font-medium">{initials(m.name)}</AvatarFallback>
                      </Avatar>
                      <span className="font-medium">{m.name}</span>
                    </div>
                  </TableCell>
                  <TableCell>
                    <div className="flex items-center gap-2">
                      <span className="text-muted-foreground">{m.email}</span>
                      {!m.email_verified_at && (
                        <ToneBadge tone="warning" dot={false}>
                          unverified
                        </ToneBadge>
                      )}
                    </div>
                  </TableCell>
                  <TableCell className="pr-6 text-right text-muted-foreground">{timeAgo(m.created_at)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </Card>
    </Page>
  );
}

function TeamCard({ team, onChanged }: { team: Team; onChanged: () => Promise<void> }) {
  const [pending, setPending] = useState<TeamToggle | null>(null);

  const toggle = async (field: TeamToggle, value: boolean) => {
    setPending(field);
    try {
      await api.patch("/team", { [field]: value });
      toast.success("Team settings saved");
      await onChanged();
    } catch (e: unknown) {
      toast.error(errorMessage(e, "Could not save team settings"));
    } finally {
      setPending(null);
    }
  };

  return (
    <Card className="gap-0 py-0">
      <CardHeader className="border-b px-6 py-4 [.border-b]:pb-4">
        <CardTitle className="text-sm">Workspace</CardTitle>
      </CardHeader>
      <CardContent className="space-y-6 px-6 py-5">
        <dl className="grid gap-5 sm:grid-cols-3">
          <Field label="Name">{team.name}</Field>
          <Field label="Description">{team.description}</Field>
          <Field label="Type">{team.personal_team ? "Personal" : "Shared"}</Field>
        </dl>
        <div className="divide-y rounded-xl border">
          {TOGGLES.map(({ field, label, hint }) => (
            <div key={field} className="flex items-center justify-between gap-4 px-4 py-3">
              <div>
                <Label htmlFor={field} className="text-sm font-medium">
                  {label}
                </Label>
                <p className="mt-0.5 text-xs text-muted-foreground">{hint}</p>
              </div>
              <Switch id={field} checked={team[field]} disabled={pending !== null} onCheckedChange={(v) => void toggle(field, v)} />
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}
