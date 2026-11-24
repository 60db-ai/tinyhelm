import type { ComponentType } from "react";
import { useTheme } from "next-themes";
import {
  LayoutDashboard,
  Boxes,
  Activity,
  SquareTerminal,
  Server,
  Plug,
  Variable,
  Users,
  Bell,
  KeyRound,
  Tag,
  Settings,
  Search,
  ChevronsUpDown,
  LogOut,
  Moon,
  Sun,
} from "lucide-react";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { BrandLogo } from "@/components/brand-logo";

export type NavId =
  | "dashboard"
  | "projects"
  | "monitoring"
  | "terminal"
  | "servers"
  | "integrations"
  | "shared-variables"
  | "team"
  | "notifications"
  | "keys"
  | "tags"
  | "settings";

interface NavItem {
  id: NavId;
  label: string;
  icon: ComponentType<{ className?: string }>;
}

export const NAV_GROUPS: { label: string | null; items: NavItem[] }[] = [
  {
    label: null,
    items: [
      { id: "dashboard", label: "Home", icon: LayoutDashboard },
      { id: "projects", label: "Projects", icon: Boxes },
      { id: "monitoring", label: "Monitoring", icon: Activity },
      { id: "terminal", label: "Terminal", icon: SquareTerminal },
    ],
  },
  {
    label: "Infrastructure",
    items: [
      { id: "servers", label: "Servers", icon: Server },
      { id: "integrations", label: "Integrations", icon: Plug },
      { id: "shared-variables", label: "Shared variables", icon: Variable },
    ],
  },
  {
    label: "Workspace",
    items: [
      { id: "team", label: "Team", icon: Users },
      { id: "notifications", label: "Notifications", icon: Bell },
      { id: "keys", label: "Keys & tokens", icon: KeyRound },
      { id: "tags", label: "Tags", icon: Tag },
      { id: "settings", label: "Settings", icon: Settings },
    ],
  },
];

export const NAV_LABEL: Record<NavId, string> = Object.fromEntries(
  NAV_GROUPS.flatMap((g) => g.items.map((i) => [i.id, i.label])),
) as Record<NavId, string>;

interface AppSidebarProps {
  active: NavId;
  teamName: string | null;
  onNavigate: (id: NavId) => void;
  onSearch: () => void;
  onSignOut: () => void;
}

export function AppSidebar({ active, teamName, onNavigate, onSearch, onSignOut }: AppSidebarProps) {
  const { resolvedTheme, setTheme } = useTheme();
  const initials = (teamName ?? "T").slice(0, 2).toUpperCase();

  return (
    <Sidebar variant="sidebar" collapsible="icon">
      <SidebarHeader className="gap-3 px-3 pt-4">
        <div className="flex h-9 items-center px-1 group-data-[collapsible=icon]:hidden">
          <BrandLogo />
        </div>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              onClick={onSearch}
              tooltip="Search"
              className="border bg-background text-muted-foreground shadow-xs hover:bg-background hover:text-foreground"
            >
              <Search />
              <span>Search</span>
              <kbd className="ml-auto rounded border bg-muted px-1.5 font-mono text-[10px] text-muted-foreground">⌘K</kbd>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        {NAV_GROUPS.map((group, index) => (
          <SidebarGroup key={group.label ?? index} className="py-1">
            {group.label && <SidebarGroupLabel>{group.label}</SidebarGroupLabel>}
            <SidebarGroupContent>
              <SidebarMenu>
                {group.items.map(({ id, label, icon: Icon }) => (
                  <SidebarMenuItem key={id}>
                    <SidebarMenuButton isActive={active === id} tooltip={label} onClick={() => onNavigate(id)} className="font-medium">
                      <Icon />
                      <span>{label}</span>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                ))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        ))}
      </SidebarContent>

      <SidebarFooter className="p-3">
        <SidebarMenu>
          <SidebarMenuItem>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <SidebarMenuButton size="lg" className="data-[state=open]:bg-sidebar-accent">
                  <Avatar className="size-8 rounded-lg">
                    <AvatarFallback className="rounded-lg bg-gradient-to-br from-orange-400 to-violet-600 text-xs font-semibold text-white">
                      {initials}
                    </AvatarFallback>
                  </Avatar>
                  <div className="grid flex-1 text-left leading-tight">
                    <span className="truncate text-sm font-medium">{teamName ?? "…"}</span>
                    <span className="truncate text-xs text-muted-foreground">Team workspace</span>
                  </div>
                  <ChevronsUpDown className="ml-auto size-4 text-muted-foreground" />
                </SidebarMenuButton>
              </DropdownMenuTrigger>
              <DropdownMenuContent side="top" align="start" className="w-(--radix-dropdown-menu-trigger-width) min-w-56">
                <DropdownMenuLabel className="text-xs text-muted-foreground">{teamName}</DropdownMenuLabel>
                <DropdownMenuItem onClick={() => onNavigate("team")}>
                  <Users /> Team members
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => setTheme(resolvedTheme === "dark" ? "light" : "dark")}>
                  {resolvedTheme === "dark" ? <Sun /> : <Moon />}
                  {resolvedTheme === "dark" ? "Light mode" : "Dark mode"}
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem variant="destructive" onClick={onSignOut}>
                  <LogOut /> Sign out
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}
