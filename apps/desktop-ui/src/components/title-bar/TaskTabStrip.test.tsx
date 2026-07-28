// @vitest-environment jsdom
import "../../test/setup";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TaskTabStrip, type TaskTabStripMeasurement } from "./TaskTabStrip";

afterEach(cleanup);

const tasks = [
  { id: "task-1", title: "First task" },
  { id: "task-2", title: "Second task" },
  { id: "task-3", title: "Third task" },
  { id: "task-4", title: "Fourth task" },
];

/// jsdom reports zero for every width, so the strip can only be pushed into
/// overflow by replacing the measurement outright. Room for two of these tabs
/// once the overflow control has been paid for.
function room(available: number): TaskTabStripMeasurement {
  return {
    available,
    overflowWidth: 60,
    tabWidths: Object.fromEntries(tasks.map((task) => [task.id, 100])),
  };
}

describe("a strip that has not been measured", () => {
  it("shows every task, which is what jsdom and the first paint both look like", () => {
    render(<TaskTabStrip activeTaskId="task-1" tasks={tasks} />);

    expect(screen.getAllByRole("tab")).toHaveLength(4);
    expect(screen.queryByRole("button", { name: /more open task/ })).toBeNull();
  });
});

describe("a strip narrower than its tabs", () => {
  it("puts the tabs that do not fit behind a control that says how many", () => {
    render(<TaskTabStrip activeTaskId="task-1" measurement={room(260)} tasks={tasks} />);

    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual([
      "First task",
      "Second task",
    ]);
    // The visible text is the prefix of the accessible name, so what a screen
    // reader says and what the user is looking at cannot drift apart.
    const trigger = screen.getByRole("button", { name: "2 more open tasks" });
    expect(trigger.textContent).toBe("2 more");
  });

  it("lists exactly the overflowed tasks in the menu", async () => {
    const user = userEvent.setup();
    render(<TaskTabStrip activeTaskId="task-1" measurement={room(260)} tasks={tasks} />);

    await user.click(screen.getByRole("button", { name: "2 more open tasks" }));
    const menu = screen.getByRole("menu", { name: "More open tasks" });
    expect(within(menu).getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "Third task",
      "Fourth task",
    ]);
  });

  it("never overflows the task the user is looking at", () => {
    // The whole point. Filling from the left would put the fourth task in the
    // menu and leave the user staring at a strip that does not contain the
    // thing on screen.
    render(<TaskTabStrip activeTaskId="task-4" measurement={room(260)} tasks={tasks} />);

    const active = screen.getByRole("tab", { name: "Fourth task" });
    expect(active.getAttribute("aria-selected")).toBe("true");
    expect(screen.queryByRole("tab", { name: "Second task" })).toBeNull();
  });

  it("counts the open tasks, not the ones that fit", () => {
    // A screen reader reading the rendered tabs would announce a different
    // number of open tasks at every window width.
    render(<TaskTabStrip activeTaskId="task-1" measurement={room(260)} tasks={tasks} />);

    const tab = screen.getByRole("tab", { name: "First task" });
    expect(tab.getAttribute("aria-setsize")).toBe("4");
    expect(
      screen.getByRole("tab", { name: "Second task" }).getAttribute("aria-posinset"),
    ).toBe("2");
  });
});

describe("reaching an overflowed task", () => {
  it("selects it from the menu and puts focus on the tab it becomes", async () => {
    // The task has no tab at the moment it is picked, so focusing "the tab"
    // has to wait for the render that creates it. Nothing armed on the trigger
    // would do: the count drops and the trigger may be gone.
    const user = userEvent.setup();
    const onSelectTask = vi.fn();
    function Host() {
      const [activeTaskId, setActiveTaskId] = useState("task-1");
      return (
        <TaskTabStrip
          activeTaskId={activeTaskId}
          measurement={room(260)}
          onSelectTask={(id) => {
            onSelectTask(id);
            setActiveTaskId(id);
          }}
          tasks={tasks}
        />
      );
    }
    render(<Host />);

    await user.click(screen.getByRole("button", { name: "2 more open tasks" }));
    await user.click(screen.getByRole("menuitem", { name: "Fourth task" }));

    expect(onSelectTask).toHaveBeenCalledWith("task-4");
    const active = screen.getByRole("tab", { name: "Fourth task" });
    expect(active.getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(active);
  });

  it("opens the menu with the keyboard and lands on its first item", async () => {
    const user = userEvent.setup();
    render(<TaskTabStrip activeTaskId="task-1" measurement={room(260)} tasks={tasks} />);

    screen.getByRole("button", { name: "2 more open tasks" }).focus();
    await user.keyboard("{Enter}");

    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Third task" }));
  });

  it("moves between menu items with the arrow, Home and End keys", async () => {
    const user = userEvent.setup();
    render(<TaskTabStrip activeTaskId="task-1" measurement={room(260)} tasks={tasks} />);

    await user.click(screen.getByRole("button", { name: "2 more open tasks" }));
    const item = (name: string) => screen.getByRole("menuitem", { name });

    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(item("Fourth task"));
    await user.keyboard("{ArrowUp}");
    expect(document.activeElement).toBe(item("Third task"));
    await user.keyboard("{End}");
    expect(document.activeElement).toBe(item("Fourth task"));
    await user.keyboard("{Home}");
    expect(document.activeElement).toBe(item("Third task"));
  });

  it("returns focus to the trigger when the menu is dismissed", async () => {
    // Escape that leaves focus on a removed menu item leaves it on <body>,
    // and the next key press goes nowhere.
    const user = userEvent.setup();
    render(<TaskTabStrip activeTaskId="task-1" measurement={room(260)} tasks={tasks} />);

    const trigger = screen.getByRole("button", { name: "2 more open tasks" });
    await user.click(trigger);
    await user.keyboard("{Escape}");

    expect(screen.queryByRole("menu")).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it("reaches an overflowed task with the arrow keys alone", async () => {
    // The tablist arrows walk every open task, not just the rendered ones —
    // otherwise the only keyboard route to an overflowed task is through the
    // menu, and a roving tabindex makes even that two extra stops away.
    const user = userEvent.setup();
    const onSelectTask = vi.fn();
    function Host() {
      const [activeTaskId, setActiveTaskId] = useState("task-1");
      return (
        <TaskTabStrip
          activeTaskId={activeTaskId}
          measurement={room(260)}
          onSelectTask={(id) => {
            onSelectTask(id);
            setActiveTaskId(id);
          }}
          tasks={tasks}
        />
      );
    }
    render(<Host />);

    screen.getByRole("tab", { name: "First task" }).focus();
    await user.keyboard("{ArrowLeft}");

    expect(onSelectTask).toHaveBeenLastCalledWith("task-4");
    expect(document.activeElement).toBe(screen.getByRole("tab", { name: "Fourth task" }));
  });
});

describe("the window drag handle", () => {
  it("keeps the tab region draggable", () => {
    // The title bar is how the window is moved. The region covers the stretch
    // of bar that `.title-strip` used to, so without the attribute a maximised
    // window has nothing left to grab.
    const { container } = render(<TaskTabStrip activeTaskId="task-1" tasks={tasks} />);

    expect(
      container.querySelector(".title-strip__tabs-region[data-tauri-drag-region]"),
    ).not.toBeNull();
  });
});

describe("closing a task", () => {
  it("closes the focused tab on Delete", () => {
    const onCloseTask = vi.fn();
    render(
      <TaskTabStrip activeTaskId="task-2" onCloseTask={onCloseTask} tasks={tasks} />,
    );

    fireEvent.keyDown(screen.getByRole("tab", { name: "Second task" }), { key: "Delete" });
    expect(onCloseTask).toHaveBeenCalledWith("task-2");
  });
});
