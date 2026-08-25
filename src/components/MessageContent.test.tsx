import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MessageContent } from "./MessageContent";

describe("MessageContent", () => {
  it("renders fenced code separately from surrounding text", () => {
    render(<MessageContent text={'Before\n```ts\nconst answer = 42;\n```\nAfter'} />);

    expect(screen.getByText("Before")).toBeInTheDocument();
    expect(screen.getByText("ts")).toBeInTheDocument();
    expect(screen.getByText("const answer = 42;")).toBeInstanceOf(HTMLElement);
    expect(screen.getByRole("button", { name: "Copy code" })).toBeInTheDocument();
    expect(screen.queryByText(/```/)).not.toBeInTheDocument();
  });

  it("formats GitHub-flavored Markdown", () => {
    render(<MessageContent text={'## Result\n\n- first\n- second\n\nUse `npm test`.\n\n> Ready'} />);

    expect(screen.getByRole("heading", { name: "Result", level: 2 })).toBeInTheDocument();
    expect(screen.getAllByRole("listitem")).toHaveLength(2);
    expect(screen.getByText("npm test")).toHaveClass("inline-code");
    expect(screen.getByText("Ready").closest("blockquote")).toBeInTheDocument();
  });
});
