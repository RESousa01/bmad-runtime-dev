// @vitest-environment jsdom
import "../../test/setup";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TaskIndex, type TaskIndexProps } from "./TaskIndex";

function createProps(overrides: Partial<TaskIndexProps> = {}): TaskIndexProps {
  return {
    closedTasks: [],
    emptyState: null,
    onNewTask: vi.fn(),
    onOpenWorkspace: vi.fn(),
    onReopenTask: vi.fn(),
    onSelectTask: vi.fn(),
    recentTasks: [],
    ...overrides,
  };
}

describe("TaskIndex", () => {
  /// `HomeView` derives the reason from the same list it passes here, so the
  /// two can only disagree through this component's own props. They are a
  /// public contract, and the failure they invite is the worst one available:
  /// an explanation rendered in place of the tasks it claims are missing.
  it("shows the rows even when it is also handed a reason there are none", () => {
    render(<TaskIndex {...createProps({
      emptyState: { message: "Nothing to see here.", reason: "no-tasks" },
      recentTasks: [{ id: "task-1", title: "Apple", updatedAt: "2h ago" }],
    })} />);

    expect(screen.getByRole("button", { name: "Resume task Apple" })).toBeTruthy();
    expect(screen.queryByText("Nothing to see here.")).toBeNull();
  });

  it("shows the reason once there is genuinely nothing to list", () => {
    render(<TaskIndex {...createProps({
      emptyState: { message: "Nothing to see here.", reason: "no-tasks" },
    })} />);

    expect(screen.getByText("Nothing to see here.")).toBeTruthy();
    expect(screen.queryByRole("list", { name: "Recent tasks" })).toBeNull();
  });
});
