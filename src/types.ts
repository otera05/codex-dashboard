export type SessionStatus = "working" | "waiting" | "idle" | "error";

export interface Session {
  id: string;
  title: string;
  cwd: string;
  status: SessionStatus;
  updatedAt: number;
  model: string;
  activeTurnId?: string;
  messages: TimelineItem[];
  tokenUsage: { input: number; output: number; cached: number };
  historyLoaded?: boolean;
}

export interface Message {
  type: "message";
  id: string;
  role: "user" | "assistant" | "system";
  text: string;
  createdAt: number;
  streaming?: boolean;
}

export interface CommandActivity {
  type: "command";
  id: string;
  command: string;
  cwd: string;
  status: "inProgress" | "completed" | "failed" | "declined";
  output?: string;
  exitCode?: number;
  durationMs?: number;
  createdAt: number;
}

export interface FileChange {
  path: string;
  kind: "add" | "update" | "delete";
  movePath?: string;
  diff: string;
  additions: number;
  deletions: number;
}

export interface FileChangeActivity {
  type: "fileChange";
  id: string;
  status: "inProgress" | "completed" | "failed" | "declined";
  changes: FileChange[];
  createdAt: number;
}

export type TimelineItem = Message | CommandActivity | FileChangeActivity;

export interface Account {
  connected: boolean;
  email?: string;
  plan?: string;
  usedPercent?: number;
  resetsAt?: number;
}

export interface ApprovalRequest {
  requestId: string | number;
  kind: "command" | "fileChange";
  threadId: string;
  turnId: string;
  itemId: string;
  startedAt: number;
  command?: string;
  cwd?: string;
  reason?: string;
  grantRoot?: string;
  availableDecisions: string[];
}

export interface CodexModel {
  id: string;
  displayName: string;
  description: string;
  isDefault: boolean;
}

export interface DashboardSnapshot {
  sessions: Session[];
  archivedSessions?: Session[];
  approvals?: ApprovalRequest[];
  account: Account;
  connected: boolean;
}

export type DashboardEvent =
  | { type: "snapshot"; snapshot: DashboardSnapshot }
  | { type: "session.updated"; session: Session }
  | { type: "approval.requested"; approval: ApprovalRequest }
  | { type: "approval.resolved"; requestId: string | number }
  | { type: "account.updated"; account: Account }
  | { type: "connection.changed"; connected: boolean };
