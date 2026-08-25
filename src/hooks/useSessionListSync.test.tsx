import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DashboardSnapshot } from "../types";

const refreshSessionList = vi.hoisted(() => vi.fn());
vi.mock("../lib/bridge", () => ({ refreshSessionList }));

import { useSessionListSync } from "./useSessionListSync";

const snapshot: DashboardSnapshot = {
  connected: true,
  account: { connected: true },
  sessions: [],
};

function Harness({ onSnapshot }: { onSnapshot: (value: DashboardSnapshot) => void }) {
  useSessionListSync({ enabled: true, onSnapshot });
  return null;
}

describe("useSessionListSync", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    refreshSessionList.mockResolvedValue(snapshot);
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("refreshes the local session list every five seconds", async () => {
    const onSnapshot = vi.fn();
    render(<Harness onSnapshot={onSnapshot} />);

    await act(() => vi.advanceTimersByTimeAsync(5_000));

    expect(refreshSessionList).toHaveBeenCalledOnce();
    expect(onSnapshot).toHaveBeenCalledWith(snapshot);
  });
});
