import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SessionHeaderMenu } from "./SessionHeaderMenu";
import type { Session } from "../types";

const session: Session = { id: "thread-1", title: "Dashboard", cwd: "/workspace/dashboard", status: "idle", updatedAt: 1, model: "gpt-5", messages: [], tokenUsage: { input: 0, output: 0, cached: 0 } };

describe("SessionHeaderMenu", () => {
  const writeText = vi.fn();
  beforeEach(() => {
    writeText.mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
  });

  it("offers session actions and copies session details", async () => {
    const onAction = vi.fn();
    render(<SessionHeaderMenu session={session} archived={false} onAction={onAction} />);
    fireEvent.click(screen.getByRole("button", { name: "Session options" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Rename" }));
    expect(onAction).toHaveBeenCalledWith("rename", session);

    fireEvent.click(screen.getByRole("button", { name: "Session options" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Copy directory" }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith("/workspace/dashboard"));
  });
});
