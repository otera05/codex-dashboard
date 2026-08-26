import { beforeEach, describe, expect, it } from "vitest";
import { useDashboard } from "./store";
import type { Session } from "./types";

const session = (id: string, historyLoaded = false): Session => ({
  id,
  title: id,
  cwd: "/workspace",
  status: "idle",
  updatedAt: 1_700_000_000_000,
  model: "Codex",
  messages: historyLoaded ? [{ type: "message", id: `${id}:message`, role: "assistant", text: "Cached", createdAt: 1 }] : [],
  tokenUsage: { input: 0, output: 0, cached: 0 },
  historyLoaded,
});

describe("dashboard session list updates", () => {
  beforeEach(() => {
    useDashboard.setState({
      sessions: [],
      archivedSessions: [],
      showingArchived: false,
      approvals: [],
      unreadCounts: {},
      account: { connected: false },
      connected: false,
      selectedId: undefined,
      settingsOpen: false,
      loadingSessionId: undefined,
      sessionError: undefined,
    });
  });

  it("adds discovered sessions while preserving loaded history", () => {
    useDashboard.setState({ sessions: [session("existing", true)], selectedId: "existing" });

    useDashboard.getState().applyEvent({
      type: "snapshot",
      snapshot: { connected: true, approvals: [], account: { connected: true }, sessions: [session("new"), session("existing")] },
    });

    expect(useDashboard.getState().sessions.map(({ id }) => id)).toEqual(["new", "existing"]);
    expect(useDashboard.getState().sessions[1].messages[0]).toMatchObject({ type: "message", text: "Cached" });
    expect(useDashboard.getState().selectedId).toBe("existing");
  });

  it("selects the next session when the current one disappears", () => {
    useDashboard.setState({ sessions: [session("removed")], selectedId: "removed" });

    useDashboard.getState().applyEvent({
      type: "snapshot",
      snapshot: { connected: true, approvals: [], account: { connected: true }, sessions: [session("remaining")] },
    });

    expect(useDashboard.getState().selectedId).toBe("remaining");
  });

  it("adds and removes approval requests from live events", () => {
    const approval = {
      requestId: 12,
      kind: "command" as const,
      threadId: "thread-1",
      turnId: "turn-1",
      itemId: "item-1",
      startedAt: 1,
      command: "npm install",
      availableDecisions: ["accept", "decline"],
    };

    useDashboard.getState().applyEvent({ type: "approval.requested", approval });
    expect(useDashboard.getState().approvals).toEqual([approval]);
    expect(useDashboard.getState().unreadCounts["thread-1"]).toBe(1);

    useDashboard.getState().applyEvent({ type: "approval.resolved", requestId: 12 });
    expect(useDashboard.getState().approvals).toEqual([]);
  });

  it("tracks activity on background sessions and clears it when selected", () => {
    useDashboard.setState({ sessions: [session("selected"), session("background")], selectedId: "selected" });
    useDashboard.getState().applyEvent({
      type: "snapshot",
      snapshot: { connected: true, approvals: [], account: { connected: true }, sessions: [session("selected"), { ...session("background"), updatedAt: 1_700_000_001_000 }] },
    });

    expect(useDashboard.getState().unreadCounts.background).toBe(1);
    useDashboard.getState().select("background");
    expect(useDashboard.getState().unreadCounts.background).toBe(0);
  });

  it("adds and selects a newly created session", () => {
    useDashboard.setState({ sessions: [session("existing")], selectedId: "existing" });

    useDashboard.getState().addSession(session("created", true));

    expect(useDashboard.getState().sessions.map(({ id }) => id)).toEqual(["created", "existing"]);
    expect(useDashboard.getState().selectedId).toBe("created");
  });

  it("moves sessions between active and archived lists", () => {
    useDashboard.setState({ sessions: [session("archived"), session("remaining")], selectedId: "archived" });

    useDashboard.getState().archiveSession("archived");

    expect(useDashboard.getState().sessions.map(({ id }) => id)).toEqual(["remaining"]);
    expect(useDashboard.getState().archivedSessions.map(({ id }) => id)).toEqual(["archived"]);
    expect(useDashboard.getState().selectedId).toBe("remaining");

    useDashboard.getState().restoreSession(session("archived", true));
    expect(useDashboard.getState().sessions.map(({ id }) => id)).toEqual(["archived", "remaining"]);
    expect(useDashboard.getState().archivedSessions).toEqual([]);
    expect(useDashboard.getState().selectedId).toBe("archived");
  });
});
