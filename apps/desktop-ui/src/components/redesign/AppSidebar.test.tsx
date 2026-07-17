// @vitest-environment jsdom
import "../../test/setup";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import {
  AppSidebar,
  type AppSidebarProps,
} from "./AppSidebar";
import {
  NoWorkspaceState,
  type NoWorkspaceStateProps,
} from "./NoWorkspaceState";

const tasks: AppSidebarProps["tasks"] = [
  {
    id: "task-architecture",
    title: "Review the architecture boundary",
    updatedAt: "Now",
    unread: true,
  },
  {
    id: "task-tests",
    title: "Tighten renderer tests",
    updatedAt: "Yesterday",
  },
];

function createSidebarProps(
  overrides: Partial<AppSidebarProps> = {},
): AppSidebarProps {
  return {
    canCreateTask: true,
    onNewTask: vi.fn(),
    onOpenAccount: vi.fn(),
    onOpenSettings: vi.fn(),
    onOpenWorkspaceManager: vi.fn(),
    onSelectTask: vi.fn(),
    selectedTaskId: "task-architecture",
    tasks,
    workspaceLabel: "Sapphirus runtime",
    workspaceStatus: "Local workspace · Governed edits",
    ...overrides,
  };
}

function createNoWorkspaceProps(
  overrides: Partial<NoWorkspaceStateProps> = {},
): NoWorkspaceStateProps {
  return {
    mode: "ready",
    onOpenWorkspace: vi.fn(),
    ...overrides,
  };
}

describe("AppSidebar", () => {
  it("keeps workspace scope, New task, and task history in primary navigation", () => {
    render(<AppSidebar {...createSidebarProps()} />);

    const sidebar = screen.getByRole("navigation", { name: "Sidebar" });
    expect(
      within(sidebar).getByRole("button", {
        name: "Manage workspace Sapphirus runtime",
      }),
    ).toBeTruthy();
    expect(within(sidebar).getByRole("button", { name: "New task" })).toBeTruthy();
    expect(within(sidebar).getByRole("heading", { name: "Tasks" })).toBeTruthy();
    expect(within(sidebar).getByText("Review the architecture boundary")).toBeTruthy();
    expect(within(sidebar).getByText("Tighten renderer tests")).toBeTruthy();

    for (const contextualDestination of [
      "Files",
      "Changes",
      "Run details",
      "Skills and agents",
    ]) {
      expect(
        within(sidebar).queryByRole("button", { name: contextualDestination }),
      ).toBeNull();
    }
  });

  it("marks the selected task and delegates task selection", async () => {
    const user = userEvent.setup();
    const onSelectTask = vi.fn();
    render(<AppSidebar {...createSidebarProps({ onSelectTask })} />);

    expect(
      screen
        .getByRole("button", { name: /Review the architecture boundary/ })
        .getAttribute("aria-current"),
    ).toBe("page");

    await user.click(
      screen.getByRole("button", { name: /Tighten renderer tests/ }),
    );

    expect(onSelectTask).toHaveBeenCalledOnce();
    expect(onSelectTask).toHaveBeenCalledWith("task-tests");
  });

  it("exposes unread task state as assistive text", () => {
    render(<AppSidebar {...createSidebarProps()} />);

    const selectedTask = screen.getByRole("button", {
      name: /Review the architecture boundary/,
    });
    expect(within(selectedTask).getByText("Unread")).toHaveProperty(
      "className",
      expect.stringContaining("sr-only"),
    );
  });

  it("delegates workspace and New task actions exactly once", async () => {
    const user = userEvent.setup();
    const onNewTask = vi.fn();
    const onOpenWorkspaceManager = vi.fn();
    render(
      <AppSidebar
        {...createSidebarProps({ onNewTask, onOpenWorkspaceManager })}
      />,
    );

    await user.click(screen.getByRole("button", { name: "New task" }));
    await user.click(
      screen.getByRole("button", {
        name: "Manage workspace Sapphirus runtime",
      }),
    );

    expect(onNewTask).toHaveBeenCalledOnce();
    expect(onOpenWorkspaceManager).toHaveBeenCalledOnce();
  });

  it("disables New task when the current workspace mode cannot authorize creation", async () => {
    const user = userEvent.setup();
    const onNewTask = vi.fn();
    render(
      <AppSidebar
        {...createSidebarProps({ canCreateTask: false, onNewTask })}
      />,
    );

    const newTask = screen.getByRole("button", { name: "New task" });
    expect(newTask).toHaveProperty("disabled", true);
    await user.click(newTask);
    expect(onNewTask).not.toHaveBeenCalled();
  });

  it("keeps named Settings and Account actions reachable at narrow width", async () => {
    const originalWidth = window.innerWidth;
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: 520,
    });
    const user = userEvent.setup();
    const onOpenSettings = vi.fn();
    const onOpenAccount = vi.fn();
    render(
      <AppSidebar
        {...createSidebarProps({ onOpenAccount, onOpenSettings })}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.click(screen.getByRole("button", { name: "Account" }));

    expect(onOpenSettings).toHaveBeenCalledOnce();
    expect(onOpenAccount).toHaveBeenCalledOnce();
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: originalWidth,
    });
  });

  it("uses a neutral Account affordance instead of hard-coded identity", () => {
    render(<AppSidebar {...createSidebarProps()} />);

    expect(screen.getByRole("button", { name: "Account" })).toBeTruthy();
    expect(screen.queryByText("RA")).toBeNull();
  });
});

describe("NoWorkspaceState", () => {
  it("offers one Open workspace action and invokes it once per activation", async () => {
    const user = userEvent.setup();
    const onOpenWorkspace = vi.fn();
    render(
      <NoWorkspaceState
        {...createNoWorkspaceProps({ onOpenWorkspace })}
      />,
    );

    const openWorkspaceActions = screen.getAllByRole("button", {
      name: "Open workspace",
    });
    expect(openWorkspaceActions).toHaveLength(1);

    await user.click(openWorkspaceActions[0]!);
    expect(onOpenWorkspace).toHaveBeenCalledOnce();
  });

  it("shows Try demo only when the optional action is supplied", async () => {
    const user = userEvent.setup();
    const onTryDemo = vi.fn();
    const { rerender } = render(
      <NoWorkspaceState
        {...createNoWorkspaceProps({ mode: "browser_demo" })}
      />,
    );
    expect(screen.queryByRole("button", { name: "Try demo" })).toBeNull();

    rerender(
      <NoWorkspaceState
        {...createNoWorkspaceProps({ mode: "browser_demo", onTryDemo })}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Try demo" }));
    expect(onTryDemo).toHaveBeenCalledOnce();
  });

  it("never exposes Try demo outside browser-demo mode", () => {
    render(
      <NoWorkspaceState
        {...createNoWorkspaceProps({ onTryDemo: vi.fn() })}
      />,
    );

    expect(screen.queryByRole("button", { name: "Try demo" })).toBeNull();
  });

  it("keeps unavailable workspace access honest and disabled", async () => {
    const user = userEvent.setup();
    const onOpenWorkspace = vi.fn();
    render(
      <NoWorkspaceState
        {...createNoWorkspaceProps({
          copy: "The signed Windows host is unavailable.",
          mode: "unavailable",
          onOpenWorkspace,
        })}
      />,
    );

    expect(screen.getByText("The signed Windows host is unavailable.")).toBeTruthy();
    const openWorkspace = screen.getByRole("button", {
      name: "Open workspace",
    });
    expect(openWorkspace).toHaveProperty("disabled", true);
    await user.click(openWorkspace);
    expect(onOpenWorkspace).not.toHaveBeenCalled();
  });
});
