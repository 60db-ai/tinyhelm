import { useState, type ComponentType, type ReactNode } from "react";
import { ChevronLeft, CircleAlert, Loader2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { ApiError } from "@/lib/api";
import { cn } from "@/lib/utils";

/*
 * Shared page building blocks for the TinyHelm SPA. Every page composes these so
 * headers, states, status badges, and confirmations look and behave the same.
 */

export function errorMessage(e: unknown, fallback = "Something went wrong"): string {
  if (e instanceof ApiError) return e.message;
  if (e instanceof Error) return e.message;
  return fallback;
}

interface PageProps {
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  back?: { label: string; onClick: () => void };
  children?: ReactNode;
  className?: string;
}

/** Page frame: optional back link, title row with actions, then content. */
export function Page({ title, description, actions, back, children, className }: PageProps) {
  return (
    <div className={cn("mx-auto w-full max-w-6xl", className)}>
      {back && (
        <button
          onClick={back.onClick}
          className="mb-4 -ml-1 inline-flex items-center gap-1 rounded-md px-1 text-sm text-muted-foreground transition-colors hover:text-foreground"
        >
          <ChevronLeft className="size-4" /> {back.label}
        </button>
      )}
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
          {description && <p className="mt-1 text-sm text-muted-foreground">{description}</p>}
        </div>
        {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
      </div>
      <div className="mt-8 space-y-6">{children}</div>
    </div>
  );
}

export function ErrorAlert({ message }: { message: string }) {
  return (
    <div role="alert" className="flex items-center gap-2 rounded-xl border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive">
      <CircleAlert className="size-4 shrink-0" /> {message}
    </div>
  );
}

interface EmptyStateProps {
  icon: ComponentType<{ className?: string }>;
  title: string;
  description?: string;
  action?: ReactNode;
}

export function EmptyState({ icon: Icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center rounded-2xl border border-dashed px-6 py-16 text-center">
      <span className="flex size-11 items-center justify-center rounded-xl border bg-muted/50">
        <Icon className="size-5 text-muted-foreground" />
      </span>
      <p className="mt-4 text-sm font-medium">{title}</p>
      {description && <p className="mt-1 max-w-sm text-sm text-muted-foreground">{description}</p>}
      {action && <div className="mt-5">{action}</div>}
    </div>
  );
}

export function LoadingRows({ rows = 3, className }: { rows?: number; className?: string }) {
  return (
    <div className={cn("space-y-2", className)}>
      {Array.from({ length: rows }, (_, i) => (
        <Skeleton key={i} className="h-12 rounded-xl" />
      ))}
    </div>
  );
}

export type Tone = "success" | "warning" | "danger" | "info" | "neutral";

const TONE_CLASS: Record<Tone, string> = {
  success: "border-success/25 bg-success/10 text-success",
  warning: "border-warning/30 bg-warning/10 text-warning",
  danger: "border-destructive/25 bg-destructive/10 text-destructive",
  info: "border-brand/25 bg-brand/10 text-brand",
  neutral: "bg-muted text-muted-foreground",
};

export function ToneBadge({ tone, children, dot = true }: { tone: Tone; children: ReactNode; dot?: boolean }) {
  return (
    <Badge variant="outline" className={cn("gap-1.5 font-medium capitalize", TONE_CLASS[tone])}>
      {dot && <span className="size-1.5 rounded-full bg-current" />}
      {children}
    </Badge>
  );
}

/** Container/resource status ("running:healthy", "exited:unhealthy", "restarting", ...). */
export function StatusBadge({ status }: { status?: string | null }) {
  const raw = (status ?? "unknown").toLowerCase();
  const [state, health] = raw.split(":");
  const tone: Tone = state.startsWith("running")
    ? health === "unhealthy"
      ? "warning"
      : "success"
    : /^(restarting|starting|degraded)/.test(state)
      ? "warning"
      : /^(exited|stopped|dead)/.test(state)
        ? "danger"
        : "neutral";
  return <ToneBadge tone={tone}>{state.replace(/_/g, " ") || "unknown"}</ToneBadge>;
}

/** Deployment queue status (queued / in_progress / finished / failed / cancelled-by-user). */
export function DeploymentBadge({ status }: { status: string }) {
  const tone: Tone =
    status === "finished" ? "success" : status === "failed" ? "danger" : status === "in_progress" ? "info" : "neutral";
  const label = status === "in_progress" ? "deploying" : status.replace(/[_-]/g, " ");
  return <ToneBadge tone={tone}>{label}</ToneBadge>;
}

export function timeAgo(iso?: string | null): string {
  if (!iso) return "—";
  const seconds = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000);
  if (seconds < 60) return "just now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
  return `${Math.floor(seconds / 86400)}d ago`;
}

interface ConfirmButtonProps {
  title: string;
  description: string;
  confirmLabel?: string;
  destructive?: boolean;
  onConfirm: () => Promise<void> | void;
  children: ReactNode;
}

/** Wraps a trigger in an AlertDialog; keeps it open with a spinner while `onConfirm` runs. */
export function ConfirmButton({ title, description, confirmLabel = "Confirm", destructive = true, onConfirm, children }: ConfirmButtonProps) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  return (
    <AlertDialog open={open} onOpenChange={(next) => !busy && setOpen(next)}>
      <AlertDialogTrigger asChild>{children}</AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>{description}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
          <AlertDialogAction
            disabled={busy}
            className={destructive ? "bg-destructive text-white hover:bg-destructive/90" : undefined}
            onClick={async (e) => {
              e.preventDefault();
              setBusy(true);
              try {
                await onConfirm();
                setOpen(false);
              } finally {
                setBusy(false);
              }
            }}
          >
            {busy && <Loader2 className="animate-spin" />}
            {confirmLabel}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

/** Label/value pair for detail grids (wrap several in a <dl className="grid ...">). */
export function Field({ label, children, mono }: { label: string; children: ReactNode; mono?: boolean }) {
  return (
    <div className="min-w-0">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className={cn("mt-1 truncate text-sm", mono && "font-mono text-[13px]")}>{children || "—"}</dd>
    </div>
  );
}

export function SectionTitle({ children, actions }: { children: ReactNode; actions?: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <h2 className="text-sm font-medium">{children}</h2>
      {actions}
    </div>
  );
}
