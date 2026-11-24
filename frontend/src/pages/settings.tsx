import { useEffect, useState } from "react";
import { useTheme } from "next-themes";
import { Monitor, Moon, Sun } from "lucide-react";
import { Page, Field } from "@/components/page";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { api, getToken } from "@/lib/api";
import { cn } from "@/lib/utils";

interface Team {
  id: number;
  name: string;
}

const THEMES = [
  { id: "light", label: "Light", icon: Sun },
  { id: "dark", label: "Dark", icon: Moon },
  { id: "system", label: "System", icon: Monitor },
] as const;

export function SettingsPage() {
  const { theme, setTheme } = useTheme();
  const [version, setVersion] = useState<string | null>(null);
  const [team, setTeam] = useState<Team | null>(null);

  useEffect(() => {
    // /version answers plain text, so the JSON api client cannot read it.
    const token = getToken();
    fetch("/api/v1/version", { headers: token ? { Authorization: `Bearer ${token}` } : {} })
      .then((res) => (res.ok ? res.text() : Promise.reject(new Error(res.statusText))))
      .then((v) => setVersion(v.trim()))
      .catch(() => setVersion("unavailable"));
    api.get<Team>("/team").then(setTeam).catch(() => setTeam(null));
  }, []);

  return (
    <Page title="Settings" description="Preferences for this browser and details about your TinyHelm instance.">
      <Card>
        <CardHeader>
          <CardTitle>Appearance</CardTitle>
          <CardDescription>Theme for this browser. System follows your OS setting.</CardDescription>
        </CardHeader>
        <CardContent>
          <div role="radiogroup" aria-label="Theme" className="grid max-w-md grid-cols-3 gap-3">
            {THEMES.map(({ id, label, icon: Icon }) => (
              <button
                key={id}
                role="radio"
                aria-checked={theme === id}
                onClick={() => setTheme(id)}
                className={cn(
                  "flex flex-col items-center gap-2 rounded-xl border p-4 text-sm transition-colors hover:bg-muted/60 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
                  theme === id && "border-foreground bg-muted/60 font-medium",
                )}
              >
                <Icon className="size-5" />
                {label}
              </button>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Team</CardTitle>
          <CardDescription>Manage members and roles on the Team page.</CardDescription>
        </CardHeader>
        <CardContent>
          <dl className="grid gap-6 sm:grid-cols-2">
            <Field label="Team name">{team ? team.name : <Skeleton className="h-4 w-32" />}</Field>
            <Field label="Team ID" mono>
              {team ? String(team.id) : <Skeleton className="h-4 w-10" />}
            </Field>
          </dl>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>About</CardTitle>
          <CardDescription>Instance details for support and API clients.</CardDescription>
        </CardHeader>
        <CardContent>
          <dl className="grid gap-6 sm:grid-cols-3">
            <Field label="Version" mono>
              {version ?? <Skeleton className="h-4 w-16" />}
            </Field>
            <Field label="API base URL" mono>
              {`${window.location.origin}/api/v1`}
            </Field>
            <Field label="Source">
              <a href="https://github.com/60db-ai/tinyhelm" target="_blank" rel="noreferrer" className="underline-offset-4 hover:underline">
                github.com/60db-ai/tinyhelm
              </a>
            </Field>
          </dl>
        </CardContent>
      </Card>
    </Page>
  );
}
