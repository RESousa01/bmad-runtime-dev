import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CommandPalette, type PaletteAction } from "./CommandPalette";

afterEach(cleanup);

function createActions(): { actions: PaletteAction[]; ran: string[] } {
  const ran: string[] = [];
  const actions: PaletteAction[] = [
    { id: "new-task", label: "New task", run: () => ran.push("new-task") },
    { id: "settings", label: "Open settings", run: () => ran.push("settings") },
  ];
  return { actions, ran };
}

describe("CommandPalette", () => {
  it("renders nothing while closed", () => {
    const { actions } = createActions();
    render(<CommandPalette actions={actions} onClose={vi.fn()} open={false} />);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("filters by substring and runs the selected action on Enter", () => {
    const { actions, ran } = createActions();
    const onClose = vi.fn();
    render(<CommandPalette actions={actions} onClose={onClose} open />);
    const input = screen.getByLabelText("Search commands");
    fireEvent.change(input, { target: { value: "settings" } });
    expect(screen.queryByText("New task")).toBeNull();
    fireEvent.keyDown(input, { key: "Enter" });
    expect(ran).toEqual(["settings"]);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("navigates with arrow keys and closes on Escape", () => {
    const { actions, ran } = createActions();
    const onClose = vi.fn();
    render(<CommandPalette actions={actions} onClose={onClose} open />);
    const input = screen.getByLabelText("Search commands");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(ran).toEqual(["settings"]);
    fireEvent.keyDown(input, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("shows an empty state for unmatched queries", () => {
    const { actions } = createActions();
    render(<CommandPalette actions={actions} onClose={vi.fn()} open />);
    fireEvent.change(screen.getByLabelText("Search commands"), {
      target: { value: "zzz" },
    });
    expect(screen.getByText("No matching commands.")).toBeTruthy();
  });
});
