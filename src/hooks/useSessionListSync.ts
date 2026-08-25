import { useEffect } from "react";
import { refreshSessionList } from "../lib/bridge";
import type { DashboardSnapshot } from "../types";

const SYNC_INTERVAL_MS = 5_000;
const MAX_RETRY_INTERVAL_MS = 30_000;

export function useSessionListSync({ enabled, onSnapshot }: {
  enabled: boolean;
  onSnapshot: (snapshot: DashboardSnapshot) => void;
}) {
  useEffect(() => {
    if (!enabled) return;

    let cancelled = false;
    let timer: number | undefined;
    let retryInterval = SYNC_INTERVAL_MS;

    const schedule = (delay: number) => {
      if (!cancelled) timer = window.setTimeout(() => void synchronize(), delay);
    };

    const synchronize = async () => {
      if (cancelled) return;
      if (document.hidden) {
        schedule(SYNC_INTERVAL_MS);
        return;
      }

      try {
        const snapshot = await refreshSessionList();
        if (cancelled) return;
        onSnapshot(snapshot);
        retryInterval = SYNC_INTERVAL_MS;
      } catch {
        retryInterval = Math.min(retryInterval * 2, MAX_RETRY_INTERVAL_MS);
      }
      schedule(retryInterval);
    };

    const handleVisibilityChange = () => {
      if (document.hidden) return;
      if (timer !== undefined) window.clearTimeout(timer);
      schedule(0);
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);
    schedule(SYNC_INTERVAL_MS);
    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [enabled, onSnapshot]);
}
