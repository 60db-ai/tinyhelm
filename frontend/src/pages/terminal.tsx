import { useState } from "react";
import { SquareTerminal, Unplug } from "lucide-react";
import { Card } from "@/components/ui/card";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { EmptyState, ErrorAlert, LoadingRows, Page } from "@/components/page";
import { api } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";

interface ServerRow {
  uuid: string;
  name: string;
  ip: string;
  user?: string;
}

/**
 * Server shell. The browser terminal needs the terminal WebSocket service (port 6002),
 * which is not ported to the Rust stack yet — so this page says so instead of faking a shell.
 */
export function TerminalPage() {
  const { data, error } = usePolledData<ServerRow[]>(() => api.get("/servers"), 30000);
  const [selected, setSelected] = useState<string | undefined>();
  const server = data?.find((s) => s.uuid === selected) ?? data?.[0];

  return (
    <Page
      title="Terminal"
      description="Open a shell on any of your servers."
      actions={
        data && data.length > 0 ? (
          <Select value={server?.uuid} onValueChange={setSelected}>
            <SelectTrigger className="w-56">
              <SelectValue placeholder="Choose a server" />
            </SelectTrigger>
            <SelectContent>
              {data.map((s) => (
                <SelectItem key={s.uuid} value={s.uuid}>
                  {s.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        ) : undefined
      }
    >
      {error && <ErrorAlert message={error} />}
      {!data && !error && <LoadingRows rows={1} />}
      {data?.length === 0 && <EmptyState icon={SquareTerminal} title="No servers yet" description="Add a server to open a terminal on it." />}
      {server && (
        <Card className="overflow-hidden border-neutral-800 bg-neutral-950 py-0 text-neutral-200">
          <div className="flex items-center gap-2 border-b border-neutral-800 px-4 py-2.5">
            <span className="size-2.5 rounded-full bg-neutral-700" />
            <span className="size-2.5 rounded-full bg-neutral-700" />
            <span className="size-2.5 rounded-full bg-neutral-700" />
            <span className="ml-3 font-mono text-xs text-neutral-400">
              {server.user ?? "root"}@{server.ip}
            </span>
          </div>
          <div className="flex min-h-80 flex-col items-center justify-center gap-3 p-8 text-center">
            <Unplug className="size-6 text-neutral-500" />
            <p className="text-sm font-medium">Terminal service not connected</p>
            <p className="max-w-md text-sm text-neutral-400">
              The browser terminal relies on the terminal WebSocket service, which is not running in this setup yet. Use SSH directly in the
              meantime:
            </p>
            <code className="rounded-md bg-neutral-900 px-3 py-1.5 font-mono text-xs text-neutral-300">
              ssh {server.user ?? "root"}@{server.ip}
            </code>
          </div>
        </Card>
      )}
    </Page>
  );
}
