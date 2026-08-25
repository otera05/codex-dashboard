import { useEffect } from "react";
import { refreshSession } from "../lib/bridge";
import type { Session } from "../types";

const SYNC_INTERVAL_MS = 2_000;
const MAX_RETRY_INTERVAL_MS = 30_000;

interface SessionSyncOptions {
  threadId?: string;
  enabled: boolean;
  onSession: (session: Session) => void;
}

export function useSessionSync({ threadId, enabled, onSession }: SessionSyncOptions) {
  useEffect(() => {
    if (!threadId || !enabled) return;

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
        const session = await refreshSession(threadId);
        if (cancelled) return;
        onSession(session);
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
  }, [threadId, enabled, onSession]);
}
