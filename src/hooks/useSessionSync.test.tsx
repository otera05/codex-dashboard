import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Session } from "../types";

const refreshSession = vi.hoisted(() => vi.fn());
vi.mock("../lib/bridge", () => ({ refreshSession }));

import { useSessionSync } from "./useSessionSync";

const session: Session = {
  id: "thread-1",
  title: "Session",
  cwd: "/workspace",
  status: "idle",
  updatedAt: 1_700_000_000_000,
  model: "Codex",
  messages: [],
  tokenUsage: { input: 0, output: 0, cached: 0 },
  historyLoaded: true,
};

function Harness({ onSession }: { onSession: (value: Session) => void }) {
  useSessionSync({ threadId: session.id, enabled: true, onSession });
  return null;
}

describe("useSessionSync", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    refreshSession.mockResolvedValue(session);
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("refreshes the selected session without overlapping requests", async () => {
    const onSession = vi.fn();
    render(<Harness onSession={onSession} />);

    await act(() => vi.advanceTimersByTimeAsync(2_000));

    expect(refreshSession).toHaveBeenCalledOnce();
    expect(onSession).toHaveBeenCalledWith(session);
  });

  it("pauses while the dashboard is hidden and resumes when visible", async () => {
    const hidden = vi.spyOn(document, "hidden", "get").mockReturnValue(true);
    const onSession = vi.fn();
    render(<Harness onSession={onSession} />);

    await act(() => vi.advanceTimersByTimeAsync(4_000));
    expect(refreshSession).not.toHaveBeenCalled();

    hidden.mockReturnValue(false);
    document.dispatchEvent(new Event("visibilitychange"));
    await act(() => vi.advanceTimersByTimeAsync(0));

    expect(refreshSession).toHaveBeenCalledOnce();
    hidden.mockRestore();
  });
});
