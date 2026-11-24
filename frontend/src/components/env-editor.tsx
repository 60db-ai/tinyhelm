import { useEffect, useState } from "react";
import { Trash2, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { api, ApiError } from "@/lib/api";

// Create/read/update/delete environment variables for an application, database, or service.
export type EnvKind = "applications" | "databases" | "services";

interface EnvVar {
  uuid?: string;
  key: string;
  value?: string | null;
  is_build_time?: boolean;
}

const input =
  "w-full rounded-md border bg-background px-2.5 py-1.5 text-sm outline-none focus:ring-2 focus:ring-ring";

export function EnvEditor({ kind, uuid }: { kind: EnvKind; uuid: string }) {
  const [rows, setRows] = useState<EnvVar[] | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const load = () => {
    setErr(null);
    return api
      .get<EnvVar[]>(`/${kind}/${uuid}/envs`)
      .then(setRows)
      .catch((e: unknown) => setErr(e instanceof ApiError ? e.message : "Failed to load variables"));
  };

  useEffect(() => {
    setRows(null);
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [kind, uuid]);

  return (
    <div className="space-y-3">
      {err && (
        <div className="rounded-xl border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm">
          {err}
        </div>
      )}
      {rows === null && !err && (
        <div className="h-32 animate-pulse rounded-xl border bg-card" />
      )}
      {rows && (
        <div className="overflow-hidden rounded-xl border bg-card">
          {rows.length === 0 && (
            <div className="px-4 py-6 text-center text-sm text-muted-foreground">
              No environment variables yet.
            </div>
          )}
          {rows.map((e) => (
            <EnvRow key={e.uuid ?? e.key} kind={kind} resourceUuid={uuid} env={e} onChanged={load} onError={setErr} />
          ))}
          <AddRow kind={kind} resourceUuid={uuid} onChanged={load} onError={setErr} />
        </div>
      )}
    </div>
  );
}

function EnvRow({
  kind,
  resourceUuid,
  env,
  onChanged,
  onError,
}: {
  kind: EnvKind;
  resourceUuid: string;
  env: EnvVar;
  onChanged: () => void | Promise<void>;
  onError: (m: string) => void;
}) {
  const [value, setValue] = useState(env.value ?? "");
  const [busy, setBusy] = useState(false);
  const dirty = value !== (env.value ?? "");

  const save = async () => {
    setBusy(true);
    try {
      await api.patch(`/${kind}/${resourceUuid}/envs`, { key: env.key, value });
      await onChanged();
    } catch (e: unknown) {
      onError(e instanceof ApiError ? e.message : "Update failed");
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!env.uuid) return;
    if (!window.confirm(`Delete ${env.key}?`)) return;
    setBusy(true);
    try {
      await api.delete(`/${kind}/${resourceUuid}/envs/${env.uuid}`);
      await onChanged();
    } catch (e: unknown) {
      onError(e instanceof ApiError ? e.message : "Delete failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex items-center gap-2 border-b px-3 py-2 font-mono text-sm last:border-b-0">
      <span className="w-48 shrink-0 truncate font-medium text-accent" title={env.key}>
        {env.key}
      </span>
      <input className={input} value={value} onChange={(ev) => setValue(ev.target.value)} disabled={busy} />
      {dirty && (
        <Button size="sm" disabled={busy} onClick={save}>
          Save
        </Button>
      )}
      <Button size="icon" variant="ghost" disabled={busy || !env.uuid} onClick={remove} aria-label="Delete">
        <Trash2 size={15} />
      </Button>
    </div>
  );
}

function AddRow({
  kind,
  resourceUuid,
  onChanged,
  onError,
}: {
  kind: EnvKind;
  resourceUuid: string;
  onChanged: () => void | Promise<void>;
  onError: (m: string) => void;
}) {
  const [key, setKey] = useState("");
  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);

  const add = async () => {
    if (!key.trim()) return;
    setBusy(true);
    try {
      await api.post(`/${kind}/${resourceUuid}/envs`, { key: key.trim(), value });
      setKey("");
      setValue("");
      await onChanged();
    } catch (e: unknown) {
      onError(e instanceof ApiError ? e.message : "Create failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex items-center gap-2 bg-elevated px-3 py-2 font-mono text-sm">
      <input
        className={input + " w-48 shrink-0"}
        placeholder="NEW_KEY"
        value={key}
        onChange={(e) => setKey(e.target.value)}
        disabled={busy}
      />
      <input
        className={input}
        placeholder="value"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && void add()}
        disabled={busy}
      />
      <Button size="sm" variant="default" disabled={busy || !key.trim()} onClick={add}>
        <Plus size={15} /> Add
      </Button>
    </div>
  );
}
