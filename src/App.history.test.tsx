import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DashboardSnapshot, Session } from "./types";

const bridge = vi.hoisted(() => ({
  getSnapshot: vi.fn(),
  getSession: vi.fn(),
  subscribe: vi.fn(),
  sendTurn: vi.fn(),
  interruptTurn: vi.fn(),
  startLogin: vi.fn(),
}));

vi.mock("./lib/bridge", () => bridge);

import { App } from "./App";
import { useDashboard } from "./store";

const unloadedSession: Session = {
  id: "thread-1",
  title: "Live session",
  cwd: "/workspace",
  status: "idle",
  updatedAt: 1_700_000_000_000,
  model: "Codex",
  messages: [],
  tokenUsage: { input: 0, output: 0, cached: 0 },
  historyLoaded: false,
};

const snapshot: DashboardSnapshot = {
  sessions: [unloadedSession],
  account: { connected: true },
  connected: true,
};

describe("session history loading", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useDashboard.setState({
      sessions: [],
      account: { connected: false },
      connected: false,
      selectedId: undefined,
      settingsOpen: false,
      loadingSessionId: undefined,
      sessionError: undefined,
    });
    bridge.getSnapshot.mockResolvedValue(snapshot);
    bridge.getSession.mockResolvedValue({
      ...unloadedSession,
      historyLoaded: true,
      messages: [{ id: "message-1", role: "assistant", text: "Loaded from App Server", createdAt: 1_700_000_000_000 }],
    });
    bridge.subscribe.mockResolvedValue(() => undefined);
  });

  it("applies history after entering the loading state", async () => {
    render(<App />);

    expect(await screen.findByText("Loaded from App Server")).toBeInTheDocument();
    expect(bridge.getSession).toHaveBeenCalledOnce();
    expect(screen.queryByText("Loading session history…")).not.toBeInTheDocument();
  });
});
