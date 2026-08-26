import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SessionActivityPanel } from "./SessionActivityPanel";
import type { Session } from "../types";

const session: Session = {
  id: "thread-1", title: "Dashboard work", cwd: "/workspace/dashboard", status: "working", updatedAt: 1, model: "gpt-5",
  tokenUsage: { input: 1200, output: 345, cached: 500 },
  messages: [
    { type: "message", id: "m1", role: "user", text: "Hello", createdAt: 1 },
    { type: "command", id: "c1", command: "npm test", cwd: "/workspace/dashboard", status: "completed", createdAt: 2 },
    { type: "fileChange", id: "f1", status: "completed", createdAt: 3, changes: [{ path: "src/App.tsx", kind: "update", diff: "", additions: 2, deletions: 1 }] },
  ],
};

describe("SessionActivityPanel", () => {
  it("summarizes the selected session and closes with Escape", () => {
    const onClose = vi.fn();
    render(<SessionActivityPanel session={session} onClose={onClose} />);
    expect(screen.getByRole("dialog", { name: "Session activity" })).toBeInTheDocument();
    expect(screen.getByText("1,545")).toBeInTheDocument();
    expect(screen.getAllByText("1")).toHaveLength(3);
    expect(screen.getByText("/workspace/dashboard")).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });
});
