// @vitest-environment jsdom
import "../test/setup";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TitleBar } from "./TitleBar";

afterEach(cleanup);

describe("TitleBar Forge identity", () => {
  it("exposes Forge as the application name", () => {
    render(<TitleBar taskTitle="Review workspace" />);

    // The title strip carries no visible wordmark by design; the product
    // identity is exposed through the banner's accessible name.
    expect(screen.getByRole("banner", { name: "Forge application" })).toBeTruthy();
    expect(screen.queryByText("Sapphirus")).toBeNull();
  });

  it("renders open tasks as tabs and activates the selected task", () => {
    const onSelectTask = vi.fn();
    render(
      <TitleBar
        activeTaskId="task-2"
        onSelectTask={onSelectTask}
        taskTabs={[
          { id: "task-1", title: "First task" },
          { id: "task-2", title: "Second task" },
        ]}
      />,
    );

    const tabs = screen.getAllByRole("tab");
    expect(tabs).toHaveLength(2);
    expect(
      screen.getByRole("tab", { name: "First task" }).getAttribute("aria-selected"),
    ).toBe("false");
    expect(
      screen.getByRole("tab", { name: "Second task" }).getAttribute("aria-selected"),
    ).toBe("true");

    fireEvent.click(screen.getByRole("tab", { name: "First task" }));
    expect(onSelectTask).toHaveBeenCalledWith("task-1");
  });

  /// Regression guard: roving tabindex makes every inactive tab unreachable by
  /// Tab, so without arrow handling the tablist could not be operated from the
  /// keyboard at all.
  it("moves between open tasks with the arrow, Home and End keys", async () => {
    const onSelectTask = vi.fn();
    const user = userEvent.setup();
    render(
      <TitleBar
        activeTaskId="task-2"
        onSelectTask={onSelectTask}
        taskTabs={[
          { id: "task-1", title: "First task" },
          { id: "task-2", title: "Second task" },
          { id: "task-3", title: "Third task" },
        ]}
      />,
    );

    screen.getByRole("tab", { name: "Second task" }).focus();
    await user.keyboard("{ArrowRight}");
    expect(onSelectTask).toHaveBeenLastCalledWith("task-3");

    await user.keyboard("{ArrowLeft}");
    expect(onSelectTask).toHaveBeenLastCalledWith("task-1");

    await user.keyboard("{Home}");
    expect(onSelectTask).toHaveBeenLastCalledWith("task-1");

    await user.keyboard("{End}");
    expect(onSelectTask).toHaveBeenLastCalledWith("task-3");
  });

  it("uses the plus action only to create another task", () => {
    const onNewTask = vi.fn();
    render(
      <TitleBar
        activeTaskId="task-1"
        onNewTask={onNewTask}
        taskTabs={[{ id: "task-1", title: "First task" }]}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "New task" }));
    expect(onNewTask).toHaveBeenCalledOnce();
  });
});
