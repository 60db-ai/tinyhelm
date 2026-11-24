import { useState } from "react";
import { Eye, EyeOff, Plus, Trash2, Check, X, Pencil, KeyRound } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { ConfirmButton, EmptyState, ErrorAlert, LoadingRows, ToneBadge, errorMessage } from "@/components/page";
import { api } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";

/** Environment variables of an application, database, or service (create / edit / delete). */
export type EnvKind = "applications" | "databases" | "services";

interface EnvVar {
  uuid: string;
  key: string;
  value?: string | null;
  is_build_time?: boolean;
}

const MASK = "••••••••••••";

export function EnvEditor({ kind, uuid }: { kind: EnvKind; uuid: string }) {
  const { data, error, refresh } = usePolledData<EnvVar[]>(() => api.get(`/${kind}/${uuid}/envs`), 30000);
  const [revealAll, setRevealAll] = useState(false);

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-sm text-muted-foreground">Values are encrypted at rest and injected when the resource starts.</p>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => setRevealAll((v) => !v)} disabled={!data?.length}>
            {revealAll ? <EyeOff /> : <Eye />} {revealAll ? "Hide values" : "Reveal values"}
          </Button>
          <AddVariableDialog kind={kind} uuid={uuid} onAdded={refresh} />
        </div>
      </div>

      {error && <ErrorAlert message={error} />}
      {!data && !error && <LoadingRows rows={3} />}
      {data?.length === 0 && (
        <EmptyState icon={KeyRound} title="No environment variables" description="Add secrets and configuration this resource needs at runtime." />
      )}
      {!!data?.length && (
        <Card className="gap-0 overflow-hidden py-0">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="w-[32%] pl-5">Key</TableHead>
                <TableHead>Value</TableHead>
                <TableHead className="w-28 pr-5 text-right" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.map((env) => (
                <EnvRow key={env.uuid} kind={kind} resourceUuid={uuid} env={env} reveal={revealAll} onChanged={refresh} />
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </div>
  );
}

interface EnvRowProps {
  kind: EnvKind;
  resourceUuid: string;
  env: EnvVar;
  reveal: boolean;
  onChanged: () => void | Promise<void>;
}

function EnvRow({ kind, resourceUuid, env, reveal, onChanged }: EnvRowProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(env.value ?? "");
  const [shown, setShown] = useState(false);
  const [busy, setBusy] = useState(false);
  const visible = reveal || shown;

  const save = async () => {
    setBusy(true);
    try {
      await api.patch(`/${kind}/${resourceUuid}/envs`, { key: env.key, value: draft });
      toast.success(`Saved ${env.key}`);
      setEditing(false);
      await onChanged();
    } catch (e: unknown) {
      toast.error(errorMessage(e, "Save failed"));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    try {
      await api.delete(`/${kind}/${resourceUuid}/envs/${env.uuid}`);
      toast.success(`Deleted ${env.key}`);
      await onChanged();
    } catch (e: unknown) {
      toast.error(errorMessage(e, "Delete failed"));
    }
  };

  return (
    <TableRow className="group">
      <TableCell className="pl-5 align-middle">
        <div className="flex items-center gap-2">
          <span className="font-mono text-[13px] font-medium">{env.key}</span>
          {env.is_build_time && (
            <ToneBadge tone="info" dot={false}>
              build
            </ToneBadge>
          )}
        </div>
      </TableCell>
      <TableCell className="max-w-0">
        {editing ? (
          <Input
            autoFocus
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void save();
              if (e.key === "Escape") setEditing(false);
            }}
            className="h-8 font-mono text-[13px]"
          />
        ) : (
          <button
            onClick={() => setShown((s) => !s)}
            className="block max-w-full truncate text-left font-mono text-[13px] text-muted-foreground hover:text-foreground"
            title={visible ? "Click to hide" : "Click to reveal"}
          >
            {visible ? env.value || <span className="italic">empty</span> : MASK}
          </button>
        )}
      </TableCell>
      <TableCell className="pr-5 text-right">
        {editing ? (
          <div className="flex justify-end gap-1">
            <Button size="icon" variant="ghost" className="size-8" onClick={() => void save()} disabled={busy} aria-label="Save">
              <Check />
            </Button>
            <Button size="icon" variant="ghost" className="size-8" onClick={() => setEditing(false)} disabled={busy} aria-label="Cancel">
              <X />
            </Button>
          </div>
        ) : (
          <div className="flex justify-end gap-1 opacity-60 transition-opacity group-hover:opacity-100">
            <Button
              size="icon"
              variant="ghost"
              className="size-8"
              aria-label={`Edit ${env.key}`}
              onClick={() => {
                setDraft(env.value ?? "");
                setEditing(true);
              }}
            >
              <Pencil />
            </Button>
            <ConfirmButton title={`Delete ${env.key}?`} description="The resource keeps the old value until it restarts." confirmLabel="Delete" onConfirm={remove}>
              <Button size="icon" variant="ghost" className="size-8 text-muted-foreground hover:text-destructive" aria-label={`Delete ${env.key}`}>
                <Trash2 />
              </Button>
            </ConfirmButton>
          </div>
        )}
      </TableCell>
    </TableRow>
  );
}

function AddVariableDialog({ kind, uuid, onAdded }: { kind: EnvKind; uuid: string; onAdded: () => void | Promise<void> }) {
  const [open, setOpen] = useState(false);
  const [key, setKey] = useState("");
  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const validKey = /^[A-Za-z_][A-Za-z0-9_.-]*$/.test(key.trim());

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.post(`/${kind}/${uuid}/envs`, { key: key.trim(), value });
      toast.success(`Added ${key.trim()}`);
      setKey("");
      setValue("");
      setOpen(false);
      await onAdded();
    } catch (e: unknown) {
      setError(errorMessage(e, "Could not add variable"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button size="sm">
          <Plus /> Add variable
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Add environment variable</DialogTitle>
          <DialogDescription>Available to the resource after its next restart or deploy.</DialogDescription>
        </DialogHeader>
        <form
          className="space-y-4"
          onSubmit={(e) => {
            e.preventDefault();
            if (validKey) void submit();
          }}
        >
          <div className="space-y-2">
            <Label htmlFor="env-key">Key</Label>
            <Input id="env-key" autoFocus value={key} onChange={(e) => setKey(e.target.value)} placeholder="DATABASE_URL" className="font-mono" />
            {key && !validKey && <p className="text-xs text-destructive">Use letters, digits, and underscores; do not start with a digit.</p>}
          </div>
          <div className="space-y-2">
            <Label htmlFor="env-value">Value</Label>
            <Input id="env-value" value={value} onChange={(e) => setValue(e.target.value)} placeholder="postgres://…" className="font-mono" />
          </div>
          {error && <ErrorAlert message={error} />}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={!validKey || busy}>
              Add variable
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
