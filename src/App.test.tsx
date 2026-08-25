import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "./App";

describe("App", () => {
  it("renders the session dashboard", async () => {
    render(<App />);
    expect((await screen.findAllByText("Realtime dashboard")).length).toBeGreaterThan(0);
    expect(screen.getByText("Codex usage")).toBeInTheDocument();
  });
});
