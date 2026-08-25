import { describe, expect, it } from "vitest";
import type { ApprovalRequest, Session } from "../types";
import { notificationChanges } from "./useDesktopNotifications";

const session = (status: Session["status"]): Session => ({ id: "thread-1", title: "Dashboard work", cwd: "/workspace", status, updatedAt: 2, model: "Codex", messages: [], tokenUsage: { input: 0, output: 0, cached: 0 } });

describe("desktop notification changes", () => {
  it("detects completion, errors, and new approvals without repeating existing approvals", () => {
    const approval: ApprovalRequest = { requestId: 1, kind: "command", threadId: "thread-1", turnId: "turn-1", itemId: "item-1", startedAt: 1, availableDecisions: ["accept", "decline"] };
    expect(notificationChanges([session("working")], [session("idle")], [], [approval]).map(({ title }) => title)).toEqual(["Codex finished", "Codex needs approval"]);
    expect(notificationChanges([session("idle")], [session("error")], [approval], [approval]).map(({ title }) => title)).toEqual(["Codex session error"]);
  });
});
