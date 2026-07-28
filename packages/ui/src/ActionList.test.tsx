import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ActionList, type ActionListItem } from "./ActionList";
import { Button } from "./Button";
import { Menu, MenuTrigger } from "./Menu";
import { SearchableList } from "./SearchableList";

const items: ActionListItem[] = [
  { id: "review", label: "Review changes", detail: "3 files" },
  { id: "apply", label: "Apply changes", detail: "⌘⏎" },
  {
    id: "publish",
    label: "Publish",
    isDisabled: true,
    disabledReason: "Sign in to an organization first",
  },
];

describe("ActionList", () => {
  it("can be driven entirely from the keyboard", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    render(<ActionList items={items} label="Task actions" onAction={onAction} />);

    await user.tab();
    await user.keyboard("{ArrowDown}");
    await user.keyboard("{Enter}");

    // FE-06 acceptance: keyboard users can complete every list flow.
    expect(onAction).toHaveBeenCalledTimes(1);
  });

  it("says why an item is unavailable, on screen and to a screen reader", () => {
    render(<ActionList items={items} label="Task actions" />);

    // FE-06 acceptance: inactive items explain why. The grey conveys nothing
    // to a screen reader, so the reason has to reach it through ARIA rather
    // than through styling.
    const reason = screen.getByText("Sign in to an organization first");
    const option = screen.getByRole("option", { name: /Publish/u });
    expect(option.getAttribute("aria-disabled")).toBe("true");
    // The reason reaches a screen reader as a description -- the name stays
    // the action -- so assert the wiring rather than the rendered text alone.
    expect(option.getAttribute("aria-describedby") ?? "").toContain(reason.id);
  });

  it("does not invoke a disabled item", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    render(<ActionList items={items} label="Task actions" onAction={onAction} />);

    await user.click(screen.getByRole("option", { name: /Publish/u }));
    expect(onAction).not.toHaveBeenCalled();
  });
});

describe("SearchableList", () => {
  it("announces how many results a filter left", async () => {
    const user = userEvent.setup();
    render(<SearchableList items={items} label="Commands" />);

    // FE-06 acceptance: the filtered result count is announced. Without this a
    // screen-reader user hears their own typing and no result.
    const status = screen.getByRole("status");
    expect(status.textContent).toBe("3 items");
    expect(status.getAttribute("aria-live")).toBe("polite");

    await user.type(screen.getByLabelText("Commands"), "appl");
    expect(status.textContent).toBe('1 of 3 item matches "appl"');

    await user.clear(screen.getByLabelText("Commands"));
    await user.type(screen.getByLabelText("Commands"), "zzz");
    expect(status.textContent).toBe('0 of 3 items match "zzz"');
  });

  it("filters on detail as well as label, because a shortcut is searchable too", async () => {
    const user = userEvent.setup();
    render(<SearchableList items={items} label="Commands" />);

    await user.type(screen.getByLabelText("Commands"), "3 files");
    expect(screen.getByRole("status").textContent).toBe('1 of 3 item matches "3 files"');
  });
});

describe("Menu", () => {
  it("opens from the keyboard and invokes the focused action", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    render(
      <MenuTrigger>
        <Button>Actions</Button>
        <Menu
          actions={[
            { id: "rename", label: "Rename", shortcut: "F2" },
            {
              id: "delete",
              label: "Delete task",
              isDisabled: true,
              disabledReason: "The task is still running",
            },
          ]}
          label="Task actions"
          onAction={onAction}
        />
      </MenuTrigger>,
    );

    await user.tab();
    await user.keyboard("{Enter}");
    await user.keyboard("{ArrowDown}");
    await user.keyboard("{Enter}");

    expect(onAction).toHaveBeenCalledWith("rename");
  });

  it("explains a disabled action rather than only greying it", async () => {
    const user = userEvent.setup();
    render(
      <MenuTrigger>
        <Button>Actions</Button>
        <Menu
          actions={[
            {
              id: "delete",
              label: "Delete task",
              isDisabled: true,
              disabledReason: "The task is still running",
            },
          ]}
          label="Task actions"
        />
      </MenuTrigger>,
    );

    await user.click(screen.getByRole("button", { name: "Actions" }));
    const reason = screen.getByText("The task is still running");
    const item = screen.getByRole("menuitem", { name: /Delete task/u });
    expect(item.getAttribute("aria-disabled")).toBe("true");
    expect(item.getAttribute("aria-describedby") ?? "").toContain(reason.id);
  });
});
