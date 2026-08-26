import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DashboardSnapshot } from "./types";

const bridge = vi.hoisted(() => ({
  getSnapshot: vi.fn(),
  getSession: vi.fn(),
  refreshSession: vi.fn(),
  refreshSessionList: vi.fn(),
  subscribe: vi.fn(),
  sendTurn: vi.fn(),
  interruptTurn: vi.fn(),
  refreshAccount: vi.fn(),
  startLogin: vi.fn(),
}));

vi.mock("./lib/bridge", () => bridge);

import { App } from "./App";
import { useDashboard } from "./store";

const snapshot: DashboardSnapshot = {
  sessions: [],
  approvals: [],
  account: { connected: false },
  connected: true,
};

describe("account connection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useDashboard.setState({
      sessions: [],
      approvals: [],
      account: { connected: false },
      connected: false,
      selectedId: undefined,
      settingsOpen: false,
      loadingSessionId: undefined,
      sessionError: undefined,
    });
    bridge.getSnapshot.mockResolvedValue(snapshot);
    bridge.refreshSessionList.mockResolvedValue(snapshot);
    bridge.subscribe.mockResolvedValue(() => undefined);
    bridge.startLogin.mockResolvedValue("https://auth.example.test/login");
    bridge.refreshAccount.mockResolvedValue({
      connected: true,
      email: "signed-in@example.test",
      plan: "Plus",
    });
  });

  it("starts browser sign-in and refreshes the displayed account", async () => {
    render(<App />);

    await act(async () => undefined);

    fireEvent.click(screen.getByRole("button", { name: /Connect ChatGPT/i }));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    });

    expect(bridge.startLogin).toHaveBeenCalledOnce();
    expect(bridge.refreshAccount).toHaveBeenCalledOnce();

    expect((await screen.findAllByText("signed-in@example.test")).length).toBeGreaterThan(0);
    expect(screen.getByText("ChatGPT account connected.")).toBeInTheDocument();
  });
});
