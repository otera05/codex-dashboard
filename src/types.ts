export type SessionStatus = "working" | "waiting" | "idle" | "error";

export interface Session {
  id: string;
  title: string;
  cwd: string;
  status: SessionStatus;
  updatedAt: number;
  model: string;
  activeTurnId?: string;
  messages: Message[];
  tokenUsage: { input: number; output: number; cached: number };
  historyLoaded?: boolean;
}

export interface Message {
  id: string;
  role: "user" | "assistant" | "system";
  text: string;
  createdAt: number;
  streaming?: boolean;
}

export interface Account {
  connected: boolean;
  email?: string;
  plan?: string;
  usedPercent?: number;
  resetsAt?: number;
}

export interface DashboardSnapshot {
  sessions: Session[];
  account: Account;
  connected: boolean;
}

export type DashboardEvent =
  | { type: "snapshot"; snapshot: DashboardSnapshot }
  | { type: "session.updated"; session: Session }
  | { type: "account.updated"; account: Account }
  | { type: "connection.changed"; connected: boolean };
