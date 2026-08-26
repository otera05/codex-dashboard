import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "./App";

describe("App", () => {
  it("renders the session dashboard", async () => {
    localStorage.removeItem("codex-dashboard.sidebar-collapsed");
    render(<App />);
    expect((await screen.findAllByText("Realtime dashboard")).length).toBeGreaterThan(0);
    expect(screen.getByText("Codex usage")).toBeInTheDocument();
    const search = screen.getByRole("textbox", { name: "Search sessions" });
    fireEvent.keyDown(window, { key: "k", metaKey: true });
    expect(search).toHaveFocus();
    fireEvent.change(search, { target: { value: "Authentication" } });
    expect(screen.getByText("1/3")).toBeInTheDocument();

    expect(document.querySelector(".window-drag")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Collapse sidebar" }));
    expect(document.querySelector(".app-shell")).toHaveClass("sidebar-collapsed");
    expect(screen.getByRole("button", { name: "Expand sidebar" })).toBeInTheDocument();
    expect(localStorage.getItem("codex-dashboard.sidebar-collapsed")).toBe("true");
  });
});
