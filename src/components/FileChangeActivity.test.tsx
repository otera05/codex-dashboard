import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { FileChangeActivity } from "./FileChangeActivity";

describe("FileChangeActivity", () => {
  it("renders file metadata, totals, and highlighted diff lines", () => {
    const { container } = render(<FileChangeActivity item={{
      type: "fileChange",
      id: "change-1",
      status: "completed",
      createdAt: 1,
      changes: [{ path: "src/App.tsx", kind: "update", diff: "@@ -1 +1 @@\n-old\n+new", additions: 1, deletions: 1 }],
    }} />);

    expect(screen.getByText("File changes")).toBeInTheDocument();
    expect(screen.getByText("src/App.tsx")).toBeInTheDocument();
    expect(screen.getByText("Completed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Copy diff for src/App.tsx" })).toBeInTheDocument();
    expect(container.querySelector(".diff-line.added")).toHaveTextContent("+new");
    expect(container.querySelector(".diff-line.deleted")).toHaveTextContent("-old");
  });
});
