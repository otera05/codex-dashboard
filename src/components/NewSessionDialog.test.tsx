import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const bridge = vi.hoisted(() => ({
  listModels: vi.fn(),
  createThread: vi.fn(),
  pickDirectory: vi.fn(),
  validateDirectory: vi.fn(),
}));
vi.mock("../lib/bridge", () => bridge);

import { NewSessionDialog } from "./NewSessionDialog";

describe("NewSessionDialog", () => {
  afterEach(() => {
    cleanup();
    localStorage.removeItem("codex-dashboard.recent-directories");
  });

  it("creates a session with the selected directory, model, and prompt", async () => {
    bridge.validateDirectory.mockResolvedValue(true);
    bridge.listModels.mockResolvedValue([{ id: "model-default", displayName: "Default model", description: "", isDefault: true }]);
    const created = { id: "thread-new", title: "New", cwd: "/workspace", status: "idle", updatedAt: 1, model: "model-default", messages: [], tokenUsage: { input: 0, output: 0, cached: 0 } };
    bridge.createThread.mockResolvedValue(created);
    const onCreated = vi.fn();

    render(<NewSessionDialog defaultCwd="/workspace" onClose={vi.fn()} onCreated={onCreated} />);
    expect(await screen.findByRole("option", { name: "Default model" })).toBeInTheDocument();
    fireEvent.change(screen.getByPlaceholderText("What should Codex work on?"), { target: { value: "Implement the feature" } });
    fireEvent.click(screen.getByRole("button", { name: "Create session" }));

    await waitFor(() => expect(bridge.createThread).toHaveBeenCalledWith("/workspace", "model-default", "Implement the feature"));
    expect(onCreated).toHaveBeenCalledWith(created);
  });

  it("selects a working directory with the native picker", async () => {
    bridge.listModels.mockResolvedValue([]);
    bridge.pickDirectory.mockResolvedValue("C:\\Users\\codex\\project");
    render(<NewSessionDialog defaultCwd="" onClose={vi.fn()} onCreated={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Browse working directory" }));
    expect(await screen.findByDisplayValue("C:\\Users\\codex\\project")).toBeInTheDocument();
  });
});
