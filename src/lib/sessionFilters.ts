import type { ApprovalRequest, Session, SessionStatus } from "../types";

export type SessionSort = "updated-desc" | "updated-asc" | "title";
export type SessionStatusFilter = "all" | SessionStatus;

export interface SessionFilters {
  query: string;
  status: SessionStatusFilter;
  approvalsOnly: boolean;
  sort: SessionSort;
}

function timelineText(session: Session) {
  return session.messages.map((item) => {
    if (item.type === "message") return item.text;
    if (item.type === "command") return `${item.command}\n${item.output ?? ""}`;
    return item.changes.map((change) => `${change.path}\n${change.movePath ?? ""}\n${change.diff}`).join("\n");
  }).join("\n");
}

export function filterSessions(sessions: Session[], approvals: ApprovalRequest[], filters: SessionFilters) {
  const query = filters.query.trim().toLocaleLowerCase();
  const approvalThreads = new Set(approvals.map((approval) => approval.threadId));
  return sessions.filter((session) => {
    if (filters.status !== "all" && session.status !== filters.status) return false;
    if (filters.approvalsOnly && !approvalThreads.has(session.id)) return false;
    if (!query) return true;
    return `${session.title}\n${session.cwd}\n${timelineText(session)}`.toLocaleLowerCase().includes(query);
  }).sort((left, right) => {
    if (filters.sort === "title") return left.title.localeCompare(right.title);
    return filters.sort === "updated-asc" ? left.updatedAt - right.updatedAt : right.updatedAt - left.updatedAt;
  });
}
