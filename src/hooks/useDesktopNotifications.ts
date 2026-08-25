import { useEffect, useRef, useState } from "react";
import { enableNativeNotifications, sendNativeNotification } from "../lib/notifications";
import type { ApprovalRequest, Session } from "../types";

const storageKey = "codex-dashboard.desktop-notifications";

export interface DashboardNotification {
  key: string;
  title: string;
  body: string;
}

export function notificationChanges(previous: Session[], current: Session[], previousApprovals: ApprovalRequest[], currentApprovals: ApprovalRequest[]): DashboardNotification[] {
  const notifications: DashboardNotification[] = [];
  for (const session of current) {
    const old = previous.find((item) => item.id === session.id);
    if (!old) continue;
    if (old.status === "working" && session.status !== "working" && session.status !== "error") notifications.push({ key: `complete:${session.id}:${session.updatedAt}`, title: "Codex finished", body: session.title });
    if (old.status !== "error" && session.status === "error") notifications.push({ key: `error:${session.id}:${session.updatedAt}`, title: "Codex session error", body: session.title });
  }
  for (const approval of currentApprovals) {
    if (!previousApprovals.some((item) => item.requestId === approval.requestId)) {
      const session = current.find((item) => item.id === approval.threadId);
      notifications.push({ key: `approval:${String(approval.requestId)}`, title: "Codex needs approval", body: session?.title ?? approval.command ?? "Review the pending request" });
    }
  }
  return notifications;
}

export function useDesktopNotifications(sessions: Session[], approvals: ApprovalRequest[]) {
  const [enabled, setEnabledState] = useState(() => localStorage.getItem(storageKey) === "true");
  const [permissionError, setPermissionError] = useState<string>();
  const previousSessions = useRef<Session[] | undefined>(undefined);
  const previousApprovals = useRef<ApprovalRequest[] | undefined>(undefined);
  const sent = useRef(new Set<string>());

  useEffect(() => {
    const oldSessions = previousSessions.current;
    const oldApprovals = previousApprovals.current;
    previousSessions.current = sessions;
    previousApprovals.current = approvals;
    if (!enabled || !oldSessions || !oldApprovals || (document.visibilityState === "visible" && document.hasFocus())) return;
    for (const notification of notificationChanges(oldSessions, sessions, oldApprovals, approvals)) {
      if (sent.current.has(notification.key)) continue;
      sent.current.add(notification.key);
      void sendNativeNotification(notification.title, notification.body);
    }
  }, [sessions, approvals, enabled]);

  const setEnabled = async (next: boolean) => {
    setPermissionError(undefined);
    if (next) {
      try {
        if (!await enableNativeNotifications()) {
          setPermissionError("Notification permission was not granted in system settings.");
          return false;
        }
      } catch (cause) {
        setPermissionError(cause instanceof Error ? cause.message : String(cause));
        return false;
      }
    }
    localStorage.setItem(storageKey, String(next));
    setEnabledState(next);
    if (next) void sendNativeNotification("Codex Dashboard", "Desktop notifications are enabled.");
    return true;
  };

  return { enabled, setEnabled, permissionError };
}
