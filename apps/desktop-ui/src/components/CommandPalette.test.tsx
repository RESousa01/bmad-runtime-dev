import "../test/setup";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

  /// The palette declares `aria-modal="true"`. That is a promise that nothing
  /// behind it can be reached, and Tab containment is half of keeping it.
  it("contains Tab inside the dialog", async () => {
    const { actions } = createActions();
    const user = userEvent.setup();
    render(
      <>
        <button type="button">Behind the palette</button>
        <CommandPalette actions={actions} onClose={vi.fn()} open />
      </>,
    );

    const input = screen.getByLabelText("Search commands");
    const last = screen.getByRole("button", { name: "Open settings" });
    const outside = screen.getByRole("button", { name: "Behind the palette" });

    last.focus();
    await user.tab();
    expect(document.activeElement).toBe(input);
    expect(document.activeElement).not.toBe(outside);

    await user.tab({ shift: true });
    expect(document.activeElement).toBe(last);
  });

  /// Focus outside the dialog is pulled back rather than left where it is; the
  /// document-level listener is what makes that reachable.
  it("pulls focus back into the dialog when Tab is pressed from outside", async () => {
    const { actions } = createActions();
    const user = userEvent.setup();
    render(
      // The outside control is *after* the palette, so a Tab that is not
      // contained leaves the document rather than falling back into the dialog
      // by DOM order and passing for the wrong reason.
      <>
        <CommandPalette actions={actions} onClose={vi.fn()} open />
        <button type="button">Behind the palette</button>
      </>,
    );

    screen.getByRole("button", { name: "Behind the palette" }).focus();
    await user.tab();

    expect(document.activeElement).toBe(screen.getByLabelText("Search commands"));
  });

  it("dismisses from a real control rather than a click handler on a div", async () => {
    const { actions } = createActions();
    const onClose = vi.fn();
    const user = userEvent.setup();
    const { container } = render(
      <CommandPalette actions={actions} onClose={onClose} open />,
    );

    const scrim = container.querySelector(".command-palette__scrim");
    expect(scrim).not.toBeNull();
    expect(scrim?.tagName).toBe("BUTTON");
    await user.click(scrim as HTMLElement);

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  /// The reset used to be an effect that wrote the query back to "" after the
  /// first paint of each opening. Mounting the dialog only while open makes the
  /// fresh query a property of the component's lifetime instead.
  it("opens with an empty query after a previous search", () => {
    const { actions } = createActions();
    const { rerender } = render(
      <CommandPalette actions={actions} onClose={vi.fn()} open />,
    );
    fireEvent.change(screen.getByLabelText("Search commands"), {
      target: { value: "settings" },
    });
    expect(screen.queryByText("New task")).toBeNull();

    rerender(<CommandPalette actions={actions} onClose={vi.fn()} open={false} />);
    rerender(<CommandPalette actions={actions} onClose={vi.fn()} open />);

    expect((screen.getByLabelText("Search commands") as HTMLInputElement).value).toBe("");
    expect(screen.getByText("New task")).toBeTruthy();
  });

  /// FE-09 acceptance: unavailability always has a user-readable reason. The
  /// palette used to omit commands it could not run, which reads as the app
  /// not having the feature at all.
  it("lists an unavailable command with its reason instead of hiding it", () => {
    render(
      <CommandPalette
        actions={[
          {
            id: "close-task",
            label: "Close task",
            unavailableReason: "Open a task first.",
            run: vi.fn(),
          },
        ]}
        onClose={vi.fn()}
        open
      />,
    );

    expect(screen.getByRole("button", { name: "Close task" })).toBeTruthy();
    expect(screen.getByText("Open a task first.")).toBeTruthy();
  });

  it("refuses to run an unavailable command and stays open", () => {
    const run = vi.fn();
    const onClose = vi.fn();
    render(
      <CommandPalette
        actions={[
          { id: "close-task", label: "Close task", unavailableReason: "Open a task first.", run },
        ]}
        onClose={onClose}
        open
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Close task" }));
    expect(run).not.toHaveBeenCalled();
    // Closing on a refusal is worse than nothing happening: it looks exactly
    // like the command having run.
    expect(onClose).not.toHaveBeenCalled();

    fireEvent.keyDown(screen.getByLabelText("Search commands"), { key: "Enter" });
    expect(run).not.toHaveBeenCalled();
  });

  /// `aria-disabled`, not `disabled`: a disabled button leaves the tab order and
  /// most screen-reader element lists, hiding the reason from the users who
  /// most need it read out.
  it("keeps an unavailable command reachable and describes why", () => {
    render(
      <CommandPalette
        actions={[
          {
            id: "close-task",
            label: "Close task",
            unavailableReason: "Open a task first.",
            run: vi.fn(),
          },
        ]}
        onClose={vi.fn()}
        open
      />,
    );

    const button = screen.getByRole("button", { name: "Close task" });
    expect(button.hasAttribute("disabled")).toBe(false);
    expect(button.getAttribute("aria-disabled")).toBe("true");

    const describedBy = button.getAttribute("aria-describedby");
    expect(describedBy).not.toBeNull();
    expect(document.getElementById(describedBy as string)?.textContent)
      .toBe("Open a task first.");
  });

  /// `kbd` means keyboard input. It used to carry category words like "drawer",
  /// which announces a filing category as something the user typed.
  it("puts only a real chord in kbd, and the category in plain text", () => {
    const { container } = render(
      <CommandPalette
        actions={[
          { id: "settings", label: "Open settings", category: "Application", keybinding: "Ctrl+,", run: vi.fn() },
        ]}
        onClose={vi.fn()}
        open
      />,
    );

    expect(container.querySelector("kbd")?.textContent).toBe("Ctrl+,");
    expect(screen.getByText("Application").tagName).not.toBe("KBD");
  });

  it("finds a command by a keyword its label does not contain", () => {
    render(
      <CommandPalette
        actions={[
          { id: "settings", label: "Open settings", keywords: ["preferences"], run: vi.fn() },
          { id: "new-task", label: "New task", run: vi.fn() },
        ]}
        onClose={vi.fn()}
        open
      />,
    );

    fireEvent.change(screen.getByLabelText("Search commands"), {
      target: { value: "preferences" },
    });
    expect(screen.getByRole("button", { name: "Open settings" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "New task" })).toBeNull();
  });

  it("focuses the search field on open and returns focus on close", async () => {
    const { actions } = createActions();
    const shell = (open: boolean) => (
      <>
        <button type="button">Opener</button>
        <CommandPalette actions={actions} onClose={vi.fn()} open={open} />
      </>
    );
    const { rerender } = render(shell(false));

    const opener = screen.getByRole("button", { name: "Opener" });
    opener.focus();
    rerender(shell(true));
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByLabelText("Search commands"));
    });

    rerender(shell(false));

    expect(document.activeElement).toBe(opener);
  });
});
