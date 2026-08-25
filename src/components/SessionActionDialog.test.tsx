import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Session } from "../types";

const bridge = vi.hoisted(() => ({ renameThread: vi.fn(), archiveThread: vi.fn() }));
vi.mock("../lib/bridge", () => bridge);

import { SessionActionDialog } from "./SessionActionDialog";

const session: Session = { id: "thread-1", title: "Old name", cwd: "/workspace", status: "idle", updatedAt: 1, model: "Codex", messages: [], tokenUsage: { input: 0, output: 0, cached: 0 } };

describe("SessionActionDialog", () => {
  it("renames a session", async () => {
    const renamed = { ...session, title: "New name" };
    bridge.renameThread.mockResolvedValue(renamed);
    const onRenamed = vi.fn();
    render(<SessionActionDialog action="rename" session={session} onClose={vi.fn()} onRenamed={onRenamed} onArchived={vi.fn()} />);

    fireEvent.change(screen.getByLabelText("Session name"), { target: { value: "New name" } });
    fireEvent.click(screen.getByRole("button", { name: "Save name" }));

    await waitFor(() => expect(bridge.renameThread).toHaveBeenCalledWith("thread-1", "New name"));
    expect(onRenamed).toHaveBeenCalledWith(renamed);
  });

  it("archives a session after confirmation", async () => {
    bridge.archiveThread.mockResolvedValue(undefined);
    const onArchived = vi.fn();
    render(<SessionActionDialog action="archive" session={session} onClose={vi.fn()} onRenamed={vi.fn()} onArchived={onArchived} />);

    fireEvent.click(screen.getByRole("button", { name: "Archive" }));

    await waitFor(() => expect(bridge.archiveThread).toHaveBeenCalledWith("thread-1"));
    expect(onArchived).toHaveBeenCalledWith("thread-1");
  });
});
