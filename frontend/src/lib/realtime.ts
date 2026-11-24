import Echo from "laravel-echo";
import Pusher from "pusher-js";
import { api, getToken } from "@/lib/api";

// Opens a Reverb WebSocket and subscribes to the team's private channel. Any status event
// (ApplicationStatusChanged, ServiceStatusChanged, ProxyStatusChanged, ...) invokes onEvent.
// Channel auth goes through POST /api/broadcasting/auth with the Sanctum bearer token.

interface RealtimeConfig {
  key: string;
  host: string | null;
  port: number;
  scheme: string;
}

export async function connectRealtime(onEvent: () => void): Promise<() => void> {
  const [cfg, team] = await Promise.all([
    api.get<RealtimeConfig>("/realtime/config"),
    api.get<{ id: number }>("/team"),
  ]);
  if (!cfg.host) throw new Error("Realtime host not configured on this instance");

  const token = getToken() ?? "";
  // laravel-echo's pusher broadcaster expects a global Pusher.
  (window as unknown as { Pusher: typeof Pusher }).Pusher = Pusher;

  const echo = new Echo({
    broadcaster: "pusher",
    key: cfg.key,
    wsHost: cfg.host,
    wsPort: cfg.port,
    wssPort: cfg.port,
    forceTLS: cfg.scheme === "https",
    enabledTransports: ["ws", "wss"],
    disableStats: true,
    authEndpoint: "/api/broadcasting/auth",
    auth: { headers: { Authorization: `Bearer ${token}`, Accept: "application/json" } },
  } as unknown as ConstructorParameters<typeof Echo>[0]);

  const channelName = `team.${team.id}`;
  // listenToAll avoids coupling to each event's class name.
  echo.private(channelName).listenToAll(() => onEvent());

  return () => {
    try {
      echo.leave(channelName);
      echo.disconnect();
    } catch {
      /* ignore */
    }
  };
}
