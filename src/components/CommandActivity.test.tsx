import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { CommandActivity } from "./CommandActivity";

describe("CommandActivity", () => {
  it("renders command status and collapsible output", () => {
    render(<CommandActivity item={{
      type: "command",
      id: "command-1",
      command: "npm test",
      cwd: "/workspace",
      status: "completed",
      output: "2 tests passed",
      exitCode: 0,
      durationMs: 1_250,
      createdAt: 1,
    }} />);

    expect(screen.getByText("npm test")).toBeInTheDocument();
    expect(screen.getByText("Completed")).toBeInTheDocument();
    expect(screen.getByText("exit 0")).toBeInTheDocument();
    expect(screen.getByText("2 tests passed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Copy command output" })).toBeInTheDocument();
  });
});
