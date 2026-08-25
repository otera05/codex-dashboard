import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ApprovalCard } from "./ApprovalCard";

describe("ApprovalCard", () => {
  it("shows a command approval and the decisions supported by the server", () => {
    render(<ApprovalCard approval={{
      requestId: 7,
      kind: "command",
      threadId: "thread-1",
      turnId: "turn-1",
      itemId: "item-1",
      startedAt: 1,
      command: "npm install",
      cwd: "/workspace",
      reason: "Requires network access",
      availableDecisions: ["accept", "acceptForSession", "decline"],
    }} />);

    expect(screen.getByText("npm install")).toBeInTheDocument();
    expect(screen.getByText("Requires network access")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Approve once" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Allow for session" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Decline" })).toBeInTheDocument();
  });
});
