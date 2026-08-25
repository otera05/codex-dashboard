import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const bridge = vi.hoisted(() => ({
  listModels: vi.fn(),
  createThread: vi.fn(),
}));
vi.mock("../lib/bridge", () => bridge);

import { NewSessionDialog } from "./NewSessionDialog";

describe("NewSessionDialog", () => {
  it("creates a session with the selected directory, model, and prompt", async () => {
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
});
