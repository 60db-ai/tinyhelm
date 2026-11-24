import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { SidebarInset, SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar";
import { Separator } from "@/components/ui/separator";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { AppSidebar, NAV_LABEL, type NavId } from "@/components/app-sidebar";
import { CommandMenu } from "@/components/command-menu";
import { BrandLogo } from "@/components/brand-logo";
import { api, ApiError, getToken, setToken, clearToken } from "@/lib/api";
import { connectRealtime } from "@/lib/realtime";
import { DashboardPage } from "@/pages/dashboard";
import { ServersPage } from "@/pages/servers";
import { ProjectsPage } from "@/pages/projects";

interface Team {
  id: number;
  name: string;
}

export default function App() {
  const [token, setTok] = useState<string | null>(getToken());

  // Any 401 from the API (expired / revoked / wrong token) sends the user back to sign-in.
  useEffect(() => {
    const onUnauthenticated = () => setTok(null);
    window.addEventListener("tinyhelm:unauthenticated", onUnauthenticated);
    return () => window.removeEventListener("tinyhelm:unauthenticated", onUnauthenticated);
  }, []);

  if (!token) return <SignIn onSignedIn={setTok} />;
  return (
    <Shell
      onSignOut={() => {
        clearToken();
        setTok(null);
      }}
    />
  );
}

function Shell({ onSignOut }: { onSignOut: () => void }) {
  const [active, setActive] = useState<NavId>("dashboard");
  const [searchOpen, setSearchOpen] = useState(false);
  const [team, setTeam] = useState<Team | null>(null);
  const [live, setLive] = useState(false);

  useEffect(() => {
    api.get<Team>("/team").then(setTeam).catch(() => setTeam(null));
    let disconnect: (() => void) | undefined;
    connectRealtime(() => window.dispatchEvent(new Event("tinyhelm:refresh")))
      .then((d) => {
        disconnect = d;
        setLive(true);
      })
      .catch(() => setLive(false));
    return () => disconnect?.();
  }, []);

  return (
    <SidebarProvider>
      <AppSidebar
        active={active}
        teamName={team?.name ?? null}
        onNavigate={setActive}
        onSearch={() => setSearchOpen(true)}
        onSignOut={onSignOut}
      />
      <SidebarInset>
        <header className="sticky top-0 z-10 flex h-14 shrink-0 items-center gap-2 border-b bg-background/80 px-4 backdrop-blur">
          <SidebarTrigger className="-ml-1 text-muted-foreground" />
          <Separator orientation="vertical" className="mr-1 data-[orientation=vertical]:h-4" />
          <span className="text-sm font-medium">{NAV_LABEL[active]}</span>
          <div className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
            <span className={"size-1.5 rounded-full " + (live ? "bg-success" : "bg-muted-foreground/40")} />
            {live ? "Realtime" : "Polling"}
          </div>
        </header>
        <main className="flex-1 px-6 py-8 lg:px-10">
          {active === "dashboard" ? (
            <DashboardPage teamName={team?.name ?? null} onNavigate={setActive} />
          ) : active === "servers" ? (
            <ServersPage />
          ) : active === "projects" ? (
            <ProjectsPage />
          ) : (
            <ComingSoon title={NAV_LABEL[active]} />
          )}
        </main>
      </SidebarInset>
      <CommandMenu open={searchOpen} onOpenChange={setSearchOpen} onNavigate={setActive} />
    </SidebarProvider>
  );
}

function ComingSoon({ title }: { title: string }) {
  return (
    <div className="mx-auto max-w-5xl">
      <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
      <div className="mt-8 flex flex-col items-center justify-center rounded-2xl border border-dashed bg-muted/30 py-24 text-center">
        <p className="text-sm font-medium">This page is being ported to the new UI.</p>
        <p className="mt-1 max-w-sm text-sm text-muted-foreground">Its API already works; the screen comes next.</p>
      </div>
    </div>
  );
}

function SignIn({ onSignedIn }: { onSignedIn: (token: string) => void }) {
  const [value, setValue] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);

  // Validate against the API before storing, so a wrong token never reaches the app shell.
  const submit = async () => {
    const candidate = value.trim();
    if (!candidate) return;
    setChecking(true);
    setError(null);
    setToken(candidate);
    try {
      await api.get<Team>("/team");
      onSignedIn(candidate);
    } catch (e: unknown) {
      clearToken();
      setError(e instanceof ApiError && e.status === 401 ? "That token is not valid. Check it and try again." : "Cannot reach the TinyHelm API.");
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="grid min-h-screen lg:grid-cols-2">
      <div className="flex flex-col p-8">
        <BrandLogo className="h-9" />
        <div className="flex flex-1 items-center justify-center">
          <form
            className="w-full max-w-sm"
            onSubmit={(e) => {
              e.preventDefault();
              void submit();
            }}
          >
            <h1 className="text-3xl font-semibold tracking-tight">Welcome back</h1>
            <p className="mt-2 text-sm text-muted-foreground">Sign in with an API token from Keys &amp; tokens.</p>
            <div className="mt-8 space-y-2">
              <Label htmlFor="token">API token</Label>
              <Input
                id="token"
                type="password"
                autoFocus
                value={value}
                onChange={(e) => setValue(e.target.value)}
                placeholder="1|xxxxxxxxxxxxxxxx"
                aria-invalid={!!error}
                className="h-11"
              />
              {error && <p className="text-sm text-destructive">{error}</p>}
            </div>
            <Button type="submit" className="mt-6 h-11 w-full rounded-full" disabled={!value.trim() || checking}>
              {checking && <Loader2 className="animate-spin" />}
              Sign in
            </Button>
          </form>
        </div>
      </div>
      <div className="relative hidden overflow-hidden bg-neutral-950 lg:block">
        <div className="absolute inset-0 bg-[radial-gradient(circle_at_30%_20%,oklch(0.7_0.19_48/0.55),transparent_45%),radial-gradient(circle_at_75%_70%,oklch(0.45_0.2_295/0.6),transparent_50%)]" />
        <div className="relative flex h-full flex-col justify-end p-12 text-white">
          <p className="max-w-md text-3xl font-semibold leading-tight tracking-tight">Deploy, manage and scale your apps on servers you own.</p>
          <p className="mt-3 text-sm text-white/60">Self-hosted. Open source. Powered by Rust.</p>
        </div>
      </div>
    </div>
  );
}
