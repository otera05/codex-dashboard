import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MessageContent, parseMessageContent } from "./MessageContent";

describe("MessageContent", () => {
  it("renders fenced code separately from surrounding text", () => {
    render(<MessageContent text={'Before\n```ts\nconst answer = 42;\n```\nAfter'} />);

    expect(screen.getByText("Before")).toBeInTheDocument();
    expect(screen.getByText("ts")).toBeInTheDocument();
    expect(screen.getByText("const answer = 42;")).toBeInstanceOf(HTMLElement);
    expect(screen.getByRole("button", { name: "Copy code" })).toBeInTheDocument();
    expect(screen.queryByText(/```/)).not.toBeInTheDocument();
  });

  it("keeps plain messages as text", () => {
    expect(parseMessageContent("No code here")).toEqual([{ type: "text", value: "No code here" }]);
  });
});
