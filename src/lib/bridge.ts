import type { CodexModel, DashboardEvent, DashboardSnapshot, Session } from "../types";

const demoSnapshot: DashboardSnapshot = {
  connected: false,
  approvals: [],
  account: { connected: true, email: "you@example.com", plan: "Plus", usedPercent: 42, resetsAt: Date.now() + 7_920_000 },
  sessions: [
    {
      id: "demo-1", title: "Realtime dashboard", cwd: "~/Projects/codex-dashboard", status: "working",
      updatedAt: Date.now() - 30_000, model: "gpt-5.6-codex", activeTurnId: "turn-1",
      tokenUsage: { input: 18_420, output: 5_280, cached: 12_700 },
      historyLoaded: true,
      messages: [
        { type: "message", id: "m1", role: "user", text: "Build the session monitoring dashboard and connect it to Codex App Server.", createdAt: Date.now() - 240_000 },
        { type: "command", id: "c1", command: "npm test", cwd: "~/Projects/codex-dashboard", status: "completed", output: "Test Files  6 passed (6)\nTests  9 passed (9)", exitCode: 0, durationMs: 842, createdAt: Date.now() - 90_000 },
        { type: "fileChange", id: "f1", status: "completed", createdAt: Date.now() - 75_000, changes: [{ path: "src/App.tsx", kind: "update", diff: "--- a/src/App.tsx\n+++ b/src/App.tsx\n@@ -1,2 +1,2 @@\n-old dashboard\n+realtime dashboard", additions: 1, deletions: 1 }] },
        { type: "message", id: "m2", role: "assistant", text: "I’m wiring the live event stream into the session workspace. The sidebar and account usage are ready.", createdAt: Date.now() - 60_000 },
      ],
    },
    { id: "demo-2", title: "Authentication flow", cwd: "~/Projects/codex-dashboard", status: "waiting", updatedAt: Date.now() - 480_000, model: "gpt-5.6-codex", tokenUsage: { input: 8_130, output: 2_020, cached: 4_100 }, historyLoaded: true, messages: [{ type: "message", id: "m3", role: "assistant", text: "Waiting for approval to open the ChatGPT sign-in page.", createdAt: Date.now() - 480_000 }] },
    { id: "demo-3", title: "Update documentation", cwd: "~/Projects/sdk", status: "idle", updatedAt: Date.now() - 7_200_000, model: "gpt-5.5", tokenUsage: { input: 3_240, output: 920, cached: 0 }, historyLoaded: true, messages: [] },
  ],
};

const isTauri = () => "__TAURI_INTERNALS__" in window;

export async function getSnapshot(): Promise<DashboardSnapshot> {
  if (!isTauri()) return demoSnapshot;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DashboardSnapshot>("get_dashboard_snapshot");
}

export async function getSession(threadId: string): Promise<Session> {
  if (!isTauri()) {
    const session = demoSnapshot.sessions.find((item) => item.id === threadId);
    if (!session) throw new Error("Session not found");
    return session;
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<Session>("get_session", { threadId });
}

export async function refreshSession(threadId: string): Promise<Session> {
  if (!isTauri()) return getSession(threadId);
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<Session>("refresh_session", { threadId });
}

export async function refreshSessionList(): Promise<DashboardSnapshot> {
  if (!isTauri()) return getSnapshot();
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DashboardSnapshot>("refresh_session_list");
}

export async function listModels(): Promise<CodexModel[]> {
  if (!isTauri()) return [
    { id: "gpt-5.6-codex", displayName: "GPT-5.6 Codex", description: "Frontier coding model", isDefault: true },
    { id: "gpt-5.5", displayName: "GPT-5.5", description: "General-purpose model", isDefault: false },
  ];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<CodexModel[]>("list_models");
}

export async function createThread(cwd: string, model: string | undefined, prompt: string): Promise<Session> {
  if (!isTauri()) return {
    id: `demo-${Date.now()}`,
    title: prompt.trim() || "New session",
    cwd,
    status: prompt.trim() ? "working" : "idle",
    updatedAt: Date.now(),
    model: model || "Codex",
    messages: prompt.trim() ? [{ type: "message", id: `demo-message-${Date.now()}`, role: "user", text: prompt.trim(), createdAt: Date.now() }] : [],
    tokenUsage: { input: 0, output: 0, cached: 0 },
    historyLoaded: true,
  };
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<Session>("create_thread", { cwd, model, prompt });
}

export async function renameThread(threadId: string, name: string): Promise<Session> {
  if (!isTauri()) {
    const session = demoSnapshot.sessions.find((item) => item.id === threadId);
    if (!session) throw new Error("Session not found");
    session.title = name;
    return { ...session };
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<Session>("rename_thread", { threadId, name });
}

export async function archiveThread(threadId: string): Promise<void> {
  if (!isTauri()) {
    demoSnapshot.sessions = demoSnapshot.sessions.filter((item) => item.id !== threadId);
    return;
  }
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("archive_thread", { threadId });
}

export async function sendTurn(threadId: string, text: string): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("send_turn", { threadId, text });
}

export async function interruptTurn(threadId: string, turnId: string): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("interrupt_turn", { threadId, turnId });
}

export async function resolveApproval(requestId: string | number, decision: "accept" | "acceptForSession" | "decline"): Promise<void> {
  if (!isTauri()) return;
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("resolve_approval", { requestId, decision });
}

export async function startLogin(): Promise<string | null> {
  if (!isTauri()) return null;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string | null>("start_chatgpt_login");
}

export async function subscribe(handler: (event: DashboardEvent) => void): Promise<() => void> {
  if (!isTauri()) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  return listen<DashboardEvent>("dashboard-event", ({ payload }) => handler(payload));
}
