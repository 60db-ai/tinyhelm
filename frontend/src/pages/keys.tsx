import { useState } from "react";
import { toast } from "sonner";
import { KeyRound, Plus, Trash2 } from "lucide-react";
import { Card } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Page, ErrorAlert, EmptyState, LoadingRows, ToneBadge, ConfirmButton, SectionTitle, timeAgo, errorMessage } from "@/components/page";
import { api } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";

/*
 * SECURITY: GET /security/keys returns the plaintext private_key (and public_key) for root tokens.
 * Only the display-safe fields below are mapped; the key material is dropped before it reaches state.
 */
interface SshKey {
  uuid: string;
  name: string;
  description: string | null;
  fingerprint: string | null;
  is_git_related: boolean;
  created_at: string | null;
}

async function fetchKeys(): Promise<SshKey[]> {
  const rows = await api.get<SshKey[]>("/security/keys");
  return rows.map(({ uuid, name, description, fingerprint, is_git_related, created_at }) => ({
    uuid,
    name,
    description,
    fingerprint,
    is_git_related,
    created_at,
  }));
}

export function KeysPage() {
  const { data, error, refresh } = usePolledData<SshKey[]>(fetchKeys, 30000);

  const remove = async (key: SshKey) => {
    try {
      await api.delete(`/security/keys/${key.uuid}`);
      toast.success(`Key "${key.name}" deleted`);
      await refresh();
    } catch (e: unknown) {
      toast.error(errorMessage(e, "Delete failed"));
    }
  };

  return (
    <Page title="Keys & tokens" description="SSH keys TinyHelm uses to reach your servers and Git repositories." actions={<AddKeyDialog onCreated={refresh} />}>
      <SectionTitle>Private keys</SectionTitle>
      {error && <ErrorAlert message={error} />}
      {!data && !error && <LoadingRows rows={2} />}
      {data?.length === 0 && <EmptyState icon={KeyRound} title="No SSH keys" description="Add a private key to connect servers or private Git repositories." />}
      {data && data.length > 0 && (
        <Card className="gap-0 overflow-hidden py-0">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="pl-6">Name</TableHead>
                <TableHead>Fingerprint</TableHead>
                <TableHead>Added</TableHead>
                <TableHead className="w-16 pr-6">
                  <span className="sr-only">Actions</span>
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.map((key) => (
                <TableRow key={key.uuid}>
                  <TableCell className="pl-6">
                    <div className="flex items-center gap-2">
                      <span className="font-medium">{key.name}</span>
                      {key.is_git_related && (
                        <ToneBadge tone="info" dot={false}>
                          git
                        </ToneBadge>
                      )}
                    </div>
                    {key.description && <div className="text-xs text-muted-foreground">{key.description}</div>}
                  </TableCell>
                  <TableCell className="font-mono text-xs text-muted-foreground">{key.fingerprint ?? "—"}</TableCell>
                  <TableCell className="text-muted-foreground">{timeAgo(key.created_at)}</TableCell>
                  <TableCell className="pr-6 text-right">
                    <ConfirmButton
                      title={`Delete key "${key.name}"?`}
                      description="Servers and Git sources that use this key will no longer connect."
                      confirmLabel="Delete"
                      onConfirm={() => remove(key)}
                    >
                      <Button size="icon" variant="ghost" className="size-8 text-muted-foreground hover:text-destructive" aria-label={`Delete key ${key.name}`}>
                        <Trash2 />
                      </Button>
                    </ConfirmButton>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
      <p className="text-xs text-muted-foreground">API tokens are not listed here: the REST API has no endpoint to list them yet.</p>
    </Page>
  );
}

function AddKeyDialog({ onCreated }: { onCreated: () => Promise<void> }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [privateKey, setPrivateKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reset = () => {
    setName("");
    setDescription("");
    setPrivateKey("");
    setError(null);
  };

  const submit = async () => {
    setSaving(true);
    setError(null);
    try {
      await api.post("/security/keys", { name: name.trim(), description: description.trim() || null, private_key: privateKey });
      toast.success(`Key "${name.trim()}" added`);
      setOpen(false);
      reset();
      await onCreated();
    } catch (e: unknown) {
      setError(errorMessage(e, "Could not add key"));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) reset();
      }}
    >
      <DialogTrigger asChild>
        <Button className="rounded-full">
          <Plus /> Add key
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-lg">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
          className="space-y-5"
        >
          <DialogHeader>
            <DialogTitle>Add private key</DialogTitle>
            <DialogDescription>The key is encrypted at rest and never shown again.</DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="key-name">Name</Label>
              <Input id="key-name" autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="deploy-key" />
            </div>
            <div className="space-y-2">
              <Label htmlFor="key-desc">Description</Label>
              <Input id="key-desc" value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Optional" />
            </div>
          </div>
          <div className="space-y-2">
            <Label htmlFor="key-pem">Private key</Label>
            <Textarea
              id="key-pem"
              value={privateKey}
              onChange={(e) => setPrivateKey(e.target.value)}
              placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
              rows={7}
              spellCheck={false}
              autoComplete="off"
              className="font-mono text-xs"
            />
          </div>
          {error && <p className="text-sm text-destructive">{error}</p>}
          <DialogFooter>
            <Button type="submit" disabled={!name.trim() || !privateKey.trim() || saving}>
              Add key
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
