import { useState } from "react";
import { toast } from "sonner";
import { Plus, Tag, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Page, ErrorAlert, EmptyState, LoadingRows, ConfirmButton, errorMessage } from "@/components/page";
import { api } from "@/lib/api";
import { usePolledData } from "@/hooks/use-polled-data";

interface TagRow {
  uuid: string;
  name: string;
}

export function TagsPage() {
  const { data, error, refresh } = usePolledData<TagRow[]>(() => api.get("/tags"), 15000);

  const remove = async (tag: TagRow) => {
    try {
      await api.delete(`/tags/${tag.uuid}`);
      toast.success(`Tag "${tag.name}" deleted`);
      await refresh();
    } catch (e: unknown) {
      toast.error(errorMessage(e, "Delete failed"));
    }
  };

  return (
    <Page title="Tags" description="Group resources across projects and deploy them together." actions={<CreateTagDialog onCreated={refresh} />}>
      {error && <ErrorAlert message={error} />}
      {!data && !error && <LoadingRows rows={2} />}
      {data?.length === 0 && <EmptyState icon={Tag} title="No tags yet" description="Create a tag, then attach it to applications, services, or databases." />}
      {data && data.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {data.map((tag) => (
            <div key={tag.uuid} className="group flex items-center gap-2 rounded-full border bg-card py-1.5 pr-1.5 pl-3.5 text-sm shadow-xs transition-colors hover:bg-muted/50">
              <Tag className="size-3.5 text-muted-foreground" />
              <span className="font-medium">{tag.name}</span>
              <ConfirmButton title={`Delete tag "${tag.name}"?`} description="The tag is removed from every resource that uses it." confirmLabel="Delete" onConfirm={() => remove(tag)}>
                <button className="flex size-6 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive" aria-label={`Delete tag ${tag.name}`}>
                  <X className="size-3.5" />
                </button>
              </ConfirmButton>
            </div>
          ))}
        </div>
      )}
    </Page>
  );
}

function CreateTagDialog({ onCreated }: { onCreated: () => Promise<void> }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setSaving(true);
    setError(null);
    try {
      await api.post("/tags", { name: name.trim() });
      toast.success(`Tag "${name.trim()}" created`);
      setOpen(false);
      setName("");
      await onCreated();
    } catch (e: unknown) {
      setError(errorMessage(e, "Could not create tag"));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button className="rounded-full">
          <Plus /> New tag
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-sm">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
          className="space-y-5"
        >
          <DialogHeader>
            <DialogTitle>New tag</DialogTitle>
            <DialogDescription>Lowercase, short names work best.</DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <Label htmlFor="tag-name">Name</Label>
            <Input id="tag-name" autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="production" />
          </div>
          {error && <p className="text-sm text-destructive">{error}</p>}
          <DialogFooter>
            <Button type="submit" disabled={!name.trim() || saving}>
              Create tag
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
