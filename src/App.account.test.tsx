import "@testing-library/jest-dom/vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
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
  logoutAccount: vi.fn(),
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
    cleanup();
    vi.clearAllMocks();
    useDashboard.setState({
      sessions: [],
      archivedSessions: [],
      showingArchived: false,
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
    bridge.logoutAccount.mockResolvedValue({ connected: false });
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

  it("disconnects the signed-in account after confirmation", async () => {
    bridge.getSnapshot.mockResolvedValue({
      ...snapshot,
      account: { connected: true, email: "signed-in@example.test", plan: "Plus" },
    });

    render(<App />);

    await act(async () => undefined);

    fireEvent.click(screen.getByRole("button", { name: /signed-in@example.test/i }));
    fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));
    const dialog = screen.getByRole("dialog", { name: "Disconnect ChatGPT" });
    expect(dialog).toBeInTheDocument();

    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Disconnect" }));
    });

    expect(bridge.logoutAccount).toHaveBeenCalledOnce();
    expect(await screen.findByText("No account connected")).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Disconnect ChatGPT" })).not.toBeInTheDocument();
  });
});
