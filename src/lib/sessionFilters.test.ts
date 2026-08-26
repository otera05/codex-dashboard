import { describe, expect, it } from "vitest";
import type { ApprovalRequest, Session } from "../types";
import { filterSessions } from "./sessionFilters";

const session = (id: string, title: string, status: Session["status"], updatedAt: number, text = ""): Session => ({ id, title, status, updatedAt, cwd: `/projects/${id}`, model: "Codex", messages: text ? [{ type: "message", id: `${id}-message`, role: "assistant", text, createdAt: 1 }] : [], tokenUsage: { input: 0, output: 0, cached: 0 } });
const approval: ApprovalRequest = { requestId: 1, kind: "command", threadId: "two", turnId: "turn", itemId: "item", startedAt: 1, availableDecisions: ["accept"] };

describe("session filters", () => {
  const sessions = [session("one", "Alpha", "idle", 1, "markdown renderer"), session("two", "Beta", "working", 3), session("three", "Gamma", "error", 2)];

  it("searches titles, directories, and loaded message content", () => {
    expect(filterSessions(sessions, [], { query: "markdown", status: "all", approvalsOnly: false, sort: "updated-desc" }).map(({ id }) => id)).toEqual(["one"]);
    expect(filterSessions(sessions, [], { query: "projects/three", status: "all", approvalsOnly: false, sort: "updated-desc" }).map(({ id }) => id)).toEqual(["three"]);
  });

  it("combines status and approval filters and sorts results", () => {
    expect(filterSessions(sessions, [approval], { query: "", status: "working", approvalsOnly: true, sort: "title" }).map(({ id }) => id)).toEqual(["two"]);
    expect(filterSessions(sessions, [], { query: "", status: "all", approvalsOnly: false, sort: "updated-desc" }).map(({ id }) => id)).toEqual(["two", "three", "one"]);
  });
});
