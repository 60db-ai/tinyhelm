import { useCallback, useEffect, useRef, useState } from "react";

// Live data via interval polling. Refreshes immediately, on an interval, and when the tab
// becomes visible again (pauses while hidden). A stopgap until token-auth WebSockets land —
// see rust-migration/MIGRATION.md (real-time via WebSocket).
export function usePolledData<T>(fetcher: () => Promise<T>, intervalMs = 8000) {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const fetcherRef = useRef(fetcher);
  fetcherRef.current = fetcher;

  const refresh = useCallback(async () => {
    try {
      const next = await fetcherRef.current();
      setData(next);
      setError(null);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : "Failed to load");
    }
  }, []);

  useEffect(() => {
    void refresh();
    const id = setInterval(() => {
      if (!document.hidden) void refresh();
    }, intervalMs);
    const onVisible = () => {
      if (!document.hidden) void refresh();
    };
    // Pushed by the realtime WebSocket on any status event, for instant refresh.
    const onSignal = () => {
      if (!document.hidden) void refresh();
    };
    document.addEventListener("visibilitychange", onVisible);
    window.addEventListener("tinyhelm:refresh", onSignal);
    return () => {
      clearInterval(id);
      document.removeEventListener("visibilitychange", onVisible);
      window.removeEventListener("tinyhelm:refresh", onSignal);
    };
  }, [refresh, intervalMs]);

  return { data, error, refresh };
}
