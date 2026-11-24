import { useEffect, useMemo, useState } from "react";
import { Loader2, Mail, MessageSquare, Send, Bell, Webhook, Hash } from "lucide-react";
import { toast } from "sonner";
import { Page, ErrorAlert, LoadingRows, ToneBadge, errorMessage } from "@/components/page";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";

type Settings = Record<string, string | number | boolean | null>;

const CHANNELS = [
  { id: "email", label: "Email", icon: Mail },
  { id: "discord", label: "Discord", icon: MessageSquare },
  { id: "slack", label: "Slack", icon: Hash },
  { id: "telegram", label: "Telegram", icon: Send },
  { id: "pushover", label: "Pushover", icon: Bell },
  { id: "webhook", label: "Webhook", icon: Webhook },
] as const;
type Channel = (typeof CHANNELS)[number]["id"];

// Read-only / bookkeeping fields, plus columns the PATCH endpoint rejects as "not allowed".
const SKIP = new Set(["id", "team_id", "failure_count", "created_at", "updated_at", "smtp_servers", "smtp_server_name"]);
const SECRET = /(token|password|secret|key|webhook_url)/;
const NUMERIC = new Set(["smtp_port", "smtp_timeout"]);
const ACRONYMS: Record<string, string> = { smtp: "SMTP", url: "URL", api: "API", id: "ID", ehlo: "EHLO", tls: "TLS" };

function humanize(words: string): string {
  const text = words
    .split("_")
    .filter(Boolean)
    .map((w) => ACRONYMS[w] ?? w)
    .join(" ");
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/** "deployment_failure_slack_notifications" -> "Deployment failure". */
function eventLabel(key: string, channel: Channel): string {
  return humanize(key.replace(`_${channel}_notifications`, ""));
}

/** "discord_webhook_url" -> "Webhook URL"; "telegram_notifications_backup_failure_thread_id" -> "Backup failure thread ID". */
function fieldLabel(key: string, channel: Channel): string {
  return humanize(key.replace(`${channel}_notifications_`, "").replace(new RegExp(`^${channel}_`), ""));
}

interface Grouped {
  master: string | null;
  toggles: string[];
  fields: string[];
  advanced: string[];
  events: string[];
}

function group(settings: Settings, channel: Channel): Grouped {
  const keys = Object.keys(settings)
    .filter((k) => !SKIP.has(k))
    .sort();
  const master = keys.includes(`${channel}_enabled`) ? `${channel}_enabled` : null;
  const events = keys.filter((k) => k.endsWith("_notifications") && typeof settings[k] === "boolean");
  const rest = keys.filter((k) => k !== master && !events.includes(k));
  return {
    master,
    events,
    toggles: rest.filter((k) => typeof settings[k] === "boolean"),
    fields: rest.filter((k) => typeof settings[k] !== "boolean" && !k.endsWith("_thread_id")),
    advanced: rest.filter((k) => k.endsWith("_thread_id")),
  };
}

export function NotificationsPage() {
  const [tab, setTab] = useState<Channel>("email");
  return (
    <Page title="Notifications" description="Choose where TinyHelm sends alerts and which events trigger them.">
      <Tabs value={tab} onValueChange={(v) => setTab(v as Channel)}>
        <TabsList>
          {CHANNELS.map(({ id, label, icon: Icon }) => (
            <TabsTrigger key={id} value={id}>
              <Icon /> {label}
            </TabsTrigger>
          ))}
        </TabsList>
        {CHANNELS.map(({ id }) => (
          <TabsContent key={id} value={id} className="mt-6">
            {tab === id && <ChannelSettings channel={id} />}
          </TabsContent>
        ))}
      </Tabs>
    </Page>
  );
}

function ChannelSettings({ channel }: { channel: Channel }) {
  const [saved, setSaved] = useState<Settings | null>(null);
  const [draft, setDraft] = useState<Settings>({});
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    api
      .get<Settings>(`/notifications/${channel}`)
      .then((s) => {
        setSaved(s);
        setDraft(s);
      })
      .catch((e: unknown) => setError(errorMessage(e, "Failed to load settings")));
  }, [channel]);

  const groups = useMemo(() => (saved ? group(saved, channel) : null), [saved, channel]);
  // The API rejects unknown fields, so only changed keys are sent.
  const dirty = saved ? Object.keys(draft).filter((k) => draft[k] !== saved[k]) : [];

  const set = (key: string, value: string | boolean) => setDraft((d) => ({ ...d, [key]: value }));

  const save = async () => {
    if (!saved) return;
    const body = Object.fromEntries(
      dirty.map((k) => {
        const v = draft[k];
        if (typeof v !== "string") return [k, v];
        const trimmed = v.trim();
        if (trimmed === "") return [k, null];
        return [k, NUMERIC.has(k) ? Number(trimmed) : trimmed];
      }),
    );
    setSaving(true);
    try {
      await api.patch(`/notifications/${channel}`, body);
      const fresh = await api.get<Settings>(`/notifications/${channel}`);
      setSaved(fresh);
      setDraft(fresh);
      toast.success("Notification settings saved");
    } catch (e: unknown) {
      toast.error(errorMessage(e, "Failed to save settings"));
    } finally {
      setSaving(false);
    }
  };

  if (error) return <ErrorAlert message={error} />;
  if (!saved || !groups) return <LoadingRows rows={4} />;

  const master = groups.master;
  const enabled = master ? Boolean(draft[master]) : null;
  const eventsOn = groups.events.filter((k) => draft[k]).length;

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            Connection
            {enabled !== null && <ToneBadge tone={enabled ? "success" : "neutral"}>{enabled ? "enabled" : "disabled"}</ToneBadge>}
          </CardTitle>
          <CardDescription>Credentials and delivery settings for this channel.</CardDescription>
        </CardHeader>
        <CardContent className="space-y-5">
          {master && <ToggleRow id={master} label="Enable channel" checked={Boolean(draft[master])} onChange={(v) => set(master, v)} />}
          {groups.toggles.map((k) => (
            <ToggleRow key={k} id={k} label={fieldLabel(k, channel)} checked={Boolean(draft[k])} onChange={(v) => set(k, v)} />
          ))}
          {groups.fields.length > 0 && (
            <div className="grid gap-4 sm:grid-cols-2">
              {groups.fields.map((k) => (
                <TextField
                  key={k}
                  id={k}
                  label={fieldLabel(k, channel)}
                  value={draft[k]}
                  secret={SECRET.test(k)}
                  numeric={NUMERIC.has(k)}
                  onChange={(v) => set(k, v)}
                />
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Events</CardTitle>
          <CardDescription>
            {eventsOn} of {groups.events.length} events send a notification.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-x-8 gap-y-4 sm:grid-cols-2">
          {groups.events.map((k) => (
            <ToggleRow key={k} id={k} label={eventLabel(k, channel)} checked={Boolean(draft[k])} onChange={(v) => set(k, v)} />
          ))}
        </CardContent>
      </Card>

      {groups.advanced.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>Topic threads</CardTitle>
            <CardDescription>Optional thread IDs to route each event into a specific topic.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4 sm:grid-cols-2">
            {groups.advanced.map((k) => (
              <TextField key={k} id={k} label={fieldLabel(k, channel)} value={draft[k]} onChange={(v) => set(k, v)} />
            ))}
          </CardContent>
        </Card>
      )}

      <div className="sticky bottom-4 flex items-center justify-end gap-3">
        {dirty.length > 0 && (
          <span className="text-xs text-muted-foreground">
            {dirty.length} unsaved change{dirty.length === 1 ? "" : "s"}
          </span>
        )}
        <Button variant="outline" disabled={dirty.length === 0 || saving} onClick={() => setDraft(saved)}>
          Discard
        </Button>
        <Button className="rounded-full" disabled={dirty.length === 0 || saving} onClick={() => void save()}>
          {saving && <Loader2 className="animate-spin" />}
          Save changes
        </Button>
      </div>
    </div>
  );
}

function ToggleRow({ id, label, checked, onChange }: { id: string; label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <Label htmlFor={id} className="font-normal">
        {label}
      </Label>
      <Switch id={id} checked={checked} onCheckedChange={onChange} />
    </div>
  );
}

interface TextFieldProps {
  id: string;
  label: string;
  value: Settings[string];
  secret?: boolean;
  numeric?: boolean;
  onChange: (v: string) => void;
}

function TextField({ id, label, value, secret, numeric, onChange }: TextFieldProps) {
  return (
    <div className="space-y-2">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        type={secret ? "password" : numeric ? "number" : "text"}
        autoComplete="off"
        value={value === null || value === undefined ? "" : String(value)}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}
