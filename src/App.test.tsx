import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "./App";

describe("App", () => {
  it("renders the session dashboard", async () => {
    render(<App />);
    expect((await screen.findAllByText("Realtime dashboard")).length).toBeGreaterThan(0);
    expect(screen.getByText("Codex usage")).toBeInTheDocument();
    const search = screen.getByRole("textbox", { name: "Search sessions" });
    fireEvent.keyDown(window, { key: "k", metaKey: true });
    expect(search).toHaveFocus();
    fireEvent.change(search, { target: { value: "Authentication" } });
    expect(screen.getByText("1/3")).toBeInTheDocument();
  });
});
