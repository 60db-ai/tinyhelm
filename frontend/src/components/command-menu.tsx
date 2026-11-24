import { useEffect, useState } from "react";
import { Boxes, Server } from "lucide-react";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command";
import { NAV_GROUPS, type NavId } from "@/components/app-sidebar";
import { api } from "@/lib/api";

interface SearchIndex {
  projects: { uuid: string; name: string }[];
  servers: { uuid: string; name: string; ip: string }[];
}

interface CommandMenuProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onNavigate: (id: NavId) => void;
}

/** ⌘K palette: jump to any page, project, or server. */
export function CommandMenu({ open, onOpenChange, onNavigate }: CommandMenuProps) {
  const [index, setIndex] = useState<SearchIndex | null>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        onOpenChange(!open);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onOpenChange]);

  useEffect(() => {
    if (open) api.get<SearchIndex>("/dashboard").then(setIndex).catch(() => setIndex(null));
  }, [open]);

  const go = (id: NavId) => {
    onNavigate(id);
    onOpenChange(false);
  };

  return (
    <CommandDialog open={open} onOpenChange={onOpenChange}>
      <CommandInput placeholder="Search pages, projects, servers…" />
      <CommandList>
        <CommandEmpty>No results.</CommandEmpty>
        {NAV_GROUPS.map((group, i) => (
          <CommandGroup key={group.label ?? i} heading={group.label ?? "Navigate"}>
            {group.items.map(({ id, label, icon: Icon }) => (
              <CommandItem key={id} value={`page ${label}`} onSelect={() => go(id)}>
                <Icon /> {label}
              </CommandItem>
            ))}
          </CommandGroup>
        ))}
        {index && (
          <>
            <CommandSeparator />
            <CommandGroup heading="Projects">
              {index.projects.map((p) => (
                <CommandItem key={p.uuid} value={`project ${p.name}`} onSelect={() => go("projects")}>
                  <Boxes /> {p.name}
                </CommandItem>
              ))}
            </CommandGroup>
            <CommandGroup heading="Servers">
              {index.servers.map((s) => (
                <CommandItem key={s.uuid} value={`server ${s.name} ${s.ip}`} onSelect={() => go("servers")}>
                  <Server /> {s.name}
                  <span className="ml-auto font-mono text-xs text-muted-foreground">{s.ip}</span>
                </CommandItem>
              ))}
            </CommandGroup>
          </>
        )}
      </CommandList>
    </CommandDialog>
  );
}
