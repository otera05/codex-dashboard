import type { DashboardEvent, DashboardSnapshot } from "../types";

const demoSnapshot: DashboardSnapshot = {
  connected: false,
  account: { connected: true, email: "you@example.com", plan: "Plus", usedPercent: 42, resetsAt: Date.now() + 7_920_000 },
  sessions: [
    {
      id: "demo-1", title: "Realtime dashboard", cwd: "~/Projects/codex-dashboard", status: "working",
      updatedAt: Date.now() - 30_000, model: "gpt-5.6-codex", activeTurnId: "turn-1",
      tokenUsage: { input: 18_420, output: 5_280, cached: 12_700 },
      messages: [
        { id: "m1", role: "user", text: "Build the session monitoring dashboard and connect it to Codex App Server.", createdAt: Date.now() - 240_000 },
        { id: "m2", role: "assistant", text: "I’m wiring the live event stream into the session workspace. The sidebar and account usage are ready.", createdAt: Date.now() - 60_000 },
      ],
    },
    { id: "demo-2", title: "Authentication flow", cwd: "~/Projects/codex-dashboard", status: "waiting", updatedAt: Date.now() - 480_000, model: "gpt-5.6-codex", tokenUsage: { input: 8_130, output: 2_020, cached: 4_100 }, messages: [{ id: "m3", role: "assistant", text: "Waiting for approval to open the ChatGPT sign-in page.", createdAt: Date.now() - 480_000 }] },
    { id: "demo-3", title: "Update documentation", cwd: "~/Projects/sdk", status: "idle", updatedAt: Date.now() - 7_200_000, model: "gpt-5.5", tokenUsage: { input: 3_240, output: 920, cached: 0 }, messages: [] },
  ],
};

const isTauri = () => "__TAURI_INTERNALS__" in window;

export async function getSnapshot(): Promise<DashboardSnapshot> {
  if (!isTauri()) return demoSnapshot;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DashboardSnapshot>("get_dashboard_snapshot");
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
