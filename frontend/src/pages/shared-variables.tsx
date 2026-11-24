import { useState } from "react";
import { toast } from "sonner";
import { Eye, EyeOff, Pencil, Plus, Trash2, Variable, Check, X } from "lucide-react";
import { Card } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Page, ErrorAlert, EmptyState, LoadingRows, ConfirmButton, errorMessage } from "@/components/page";
import { api } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";

interface SharedEnv {
  id: number;
  key: string;
  value: string | null;
}

export function SharedVariablesPage() {
  const { data, error, refresh } = usePolledData<SharedEnv[]>(() => api.get("/team/envs"), 15000);

  return (
    <Page
      title="Shared variables"
      description={
        <>
          Team-wide variables. Reference them in any resource as <code className="rounded bg-muted px-1 font-mono text-xs">{"{{team.KEY}}"}</code>.
        </>
      }
      actions={<AddVariableDialog onCreated={refresh} />}
    >
      {error && <ErrorAlert message={error} />}
      {!data && !error && <LoadingRows />}
      {data?.length === 0 && (
        <EmptyState icon={Variable} title="No shared variables" description="Add a variable once and reuse it across every project in this team." />
      )}
      {data && data.length > 0 && (
        <Card className="gap-0 overflow-hidden py-0">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="pl-6">Key</TableHead>
                <TableHead>Value</TableHead>
                <TableHead className="w-28 pr-6 text-right">
                  <span className="sr-only">Actions</span>
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.map((env) => (
                <EnvRow key={env.id} env={env} onChanged={refresh} />
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </Page>
  );
}

function EnvRow({ env, onChanged }: { env: SharedEnv; onChanged: () => Promise<void> }) {
  const [revealed, setRevealed] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(env.value ?? "");
  const [saving, setSaving] = useState(false);

  const save = async () => {
    setSaving(true);
    try {
      await api.patch(`/team/envs/${env.id}`, { value: draft });
      toast.success(`${env.key} updated`);
      setEditing(false);
      await onChanged();
    } catch (e: unknown) {
      toast.error(errorMessage(e, "Update failed"));
    } finally {
      setSaving(false);
    }
  };

  const remove = async () => {
    try {
      await api.delete(`/team/envs/${env.id}`);
      toast.success(`${env.key} deleted`);
      await onChanged();
    } catch (e: unknown) {
      toast.error(errorMessage(e, "Delete failed"));
    }
  };

  return (
    <TableRow>
      <TableCell className="pl-6 font-mono text-[13px] font-medium">{env.key}</TableCell>
      <TableCell className="max-w-md">
        {editing ? (
          <div className="flex items-center gap-2">
            <Input
              autoFocus
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void save();
                if (e.key === "Escape") setEditing(false);
              }}
              className="h-8 font-mono text-[13px]"
              aria-label={`Value for ${env.key}`}
            />
            <Button size="icon" variant="ghost" className="size-8" disabled={saving} onClick={save} aria-label="Save">
              <Check />
            </Button>
            <Button size="icon" variant="ghost" className="size-8" disabled={saving} onClick={() => setEditing(false)} aria-label="Cancel">
              <X />
            </Button>
          </div>
        ) : (
          <div className="flex items-center gap-2">
            <span className="truncate font-mono text-[13px] text-muted-foreground">
              {revealed ? env.value || <em className="not-italic opacity-60">empty</em> : "••••••••••••"}
            </span>
            <Button size="icon" variant="ghost" className="size-7 shrink-0" onClick={() => setRevealed((r) => !r)} aria-label={revealed ? "Hide value" : "Show value"}>
              {revealed ? <EyeOff /> : <Eye />}
            </Button>
          </div>
        )}
      </TableCell>
      <TableCell className="pr-6 text-right">
        {!editing && (
          <div className="flex justify-end gap-1">
            <Button
              size="icon"
              variant="ghost"
              className="size-8"
              onClick={() => {
                setDraft(env.value ?? "");
                setEditing(true);
              }}
              aria-label={`Edit ${env.key}`}
            >
              <Pencil />
            </Button>
            <ConfirmButton title={`Delete ${env.key}?`} description="Resources that reference this variable will lose its value on their next deploy." confirmLabel="Delete" onConfirm={remove}>
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

function AddVariableDialog({ onCreated }: { onCreated: () => Promise<void> }) {
  const [open, setOpen] = useState(false);
  const [key, setKey] = useState("");
  const [value, setValue] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setSaving(true);
    setError(null);
    try {
      await api.post("/team/envs", { key: key.trim(), value });
      toast.success(`${key.trim()} added`);
      setOpen(false);
      setKey("");
      setValue("");
      await onCreated();
    } catch (e: unknown) {
      setError(errorMessage(e, "Could not add variable"));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button className="rounded-full">
          <Plus /> Add variable
        </Button>
      </DialogTrigger>
      <DialogContent>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
          className="space-y-5"
        >
          <DialogHeader>
            <DialogTitle>Add shared variable</DialogTitle>
            <DialogDescription>Available to every resource in this team.</DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <Label htmlFor="sv-key">Key</Label>
            <Input id="sv-key" autoFocus value={key} onChange={(e) => setKey(e.target.value)} placeholder="DATABASE_URL" className="font-mono" />
          </div>
          <div className="space-y-2">
            <Label htmlFor="sv-value">Value</Label>
            <Input id="sv-value" value={value} onChange={(e) => setValue(e.target.value)} className="font-mono" />
          </div>
          {error && <p className="text-sm text-destructive">{error}</p>}
          <DialogFooter>
            <Button type="submit" disabled={!key.trim() || saving}>
              Add variable
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
