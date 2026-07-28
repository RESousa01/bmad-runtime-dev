// @vitest-environment jsdom
import "../test/setup";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { WorkspaceProjection } from "../lib/hostClient";
import { HomeView, type HomeViewProps } from "./HomeView";

function workspace(name: string): WorkspaceProjection {
  return {
    workspaceId: `workspace_${name}`,
    projectId: `project_${name}`,
    displayName: name,
    grantEpoch: 1,
    permissions: "read_only",
    contextReadEpoch: 1,
    governedEditEpoch: 1,
  };
}

const alpha = workspace("alpha");
const beta = workspace("beta");

function createProps(overrides: Partial<HomeViewProps> = {}): HomeViewProps {
  return {
    activeWorkspaceId: alpha.workspaceId,
    canOpenFolder: true,
    closedTasks: [],
    composerDisabled: false,
    hostKind: "ready",
    onActivateWorkspace: vi.fn(),
    onNewTask: vi.fn(),
    onOpenWorkspace: vi.fn(),
    onOpenWorkspaceManager: vi.fn(),
    onReopenTask: vi.fn(),
    onSelectTask: vi.fn(),
    onSubmitIntent: vi.fn(),
    recentTasks: [],
    statusHint: "Ready to send through the selected model.",
    workspaces: [alpha],
    workspaceStatusLabel: "Read only",
    ...overrides,
  };
}

/// The sentence Home shows in place of its rows.
///
/// Read from the DOM rather than through a role query because an empty state is
/// prose, not a control: giving it a role purely so a test could find it would
/// be a fixture leaking into the accessibility tree.
function emptyNotice(): string | null {
  return document.querySelector(".home-view__task-empty")?.textContent ?? null;
}

/// What a screen reader would call each control, in document order.
function buttonNames(): readonly string[] {
  return screen.getAllByRole("button").map(
    (button) => button.getAttribute("aria-label") ?? button.textContent?.trim() ?? "",
  );
}

describe("HomeView", () => {
  it("lists every project and marks exactly the live one", () => {
    render(<HomeView {...createProps({ activeWorkspaceId: beta.workspaceId, workspaces: [alpha, beta] })} />);

    const projects = screen.getByRole("list", { name: "Projects" });
    const rows = within(projects).getAllByRole("button");
    expect(rows.map((row) => row.textContent)).toEqual(["alpha", "beta"]);
    // Exact attribute values, not truthiness: a rail that marks every row, or
    // the wrong row, is the failure worth catching.
    expect(rows.map((row) => row.getAttribute("aria-current"))).toEqual([null, "page"]);
  });

  it("switches project on one press, to the project the press was on", async () => {
    const onActivateWorkspace = vi.fn();
    const user = userEvent.setup();
    render(<HomeView {...createProps({ onActivateWorkspace, workspaces: [alpha, beta] })} />);

    const projects = screen.getByRole("list", { name: "Projects" });
    await user.click(within(projects).getByRole("button", { name: "beta" }));

    expect(onActivateWorkspace.mock.calls).toEqual([[beta.workspaceId]]);
  });

  it("keeps the caller's order and never re-sorts the index", () => {
    // `updatedAt` is a display string the shell picks, not a timestamp, so a
    // component that sorted on it would order the index by the alphabet and
    // silently break "most recent first".
    render(<HomeView {...createProps({
      recentTasks: [
        { id: "task-3", title: "Zebra", updatedAt: "Now" },
        { id: "task-1", title: "Apple", updatedAt: "2h ago" },
        { id: "task-2", title: "Mango", updatedAt: "Yesterday" },
      ],
    })} />);

    const index = screen.getByRole("list", { name: "Recent tasks" });
    expect(within(index).getAllByRole("button").map((row) => row.getAttribute("aria-label")))
      .toEqual(["Resume task Zebra", "Resume task Apple", "Resume task Mango"]);
  });

  it("resumes a task in one press, and resuming is not reopening", async () => {
    const onSelectTask = vi.fn();
    const onReopenTask = vi.fn();
    const user = userEvent.setup();
    render(<HomeView {...createProps({
      onReopenTask,
      onSelectTask,
      recentTasks: [
        { id: "task-3", title: "Zebra", updatedAt: "Now" },
        { id: "task-1", title: "Apple", updatedAt: "2h ago" },
      ],
    })} />);

    await user.click(screen.getByRole("button", { name: "Resume task Apple" }));

    expect(onSelectTask.mock.calls).toEqual([["task-1"]]);
    expect(onReopenTask).not.toHaveBeenCalled();
  });

  it("gives its three empty states three different sentences", () => {
    // The bug this replaces: "No tasks yet. Describe what you want above" shown
    // to a user with no folder granted and a disabled composer to describe it
    // to, and to a user whose host is not answering at all.
    const notices: (string | null)[] = [];
    for (const props of [
      { hostKind: "unavailable" as const, workspaces: [] },
      { hostKind: "ready" as const, workspaces: [] },
      { hostKind: "ready" as const, workspaces: [alpha] },
    ]) {
      const view = render(<HomeView {...createProps({ ...props, recentTasks: [] })} />);
      notices.push(emptyNotice());
      view.unmount();
    }

    for (const notice of notices) expect(typeof notice).toBe("string");
    expect(new Set(notices).size).toBe(3);
    expect(notices[0]).toContain("unavailable");
    expect(notices[1]).toContain("folder");
    expect(notices[2]).toContain("No tasks yet");
  });

  it("assumes a ready host until the shell says otherwise", () => {
    // The prop is optional so the shell can be wired separately. The default
    // has to be the state in which the rest of Home's copy is already correct,
    // or an unwired Home is a confidently wrong Home.
    render(<HomeView {...createProps({ workspaces: [] })} hostKind={undefined} />);

    expect(emptyNotice()).toContain("folder");
    expect(screen.getByRole("button", { name: "Open a local workspace" })).toBeTruthy();
  });

  it("shows the index rather than an explanation whenever it has rows", () => {
    // Even with the host down: rows the user can still resume outrank a notice
    // about why there are no more of them.
    render(<HomeView {...createProps({
      hostKind: "unavailable",
      recentTasks: [{ id: "task-1", title: "Apple", updatedAt: "2h ago" }],
      workspaces: [],
    })} />);

    expect(emptyNotice()).toBeNull();
    expect(screen.getByRole("button", { name: "Resume task Apple" })).toBeTruthy();
  });

  it("offers the folder picker only where a folder is the thing that is missing", async () => {
    const onOpenWorkspace = vi.fn();
    const user = userEvent.setup();
    const view = render(
      <HomeView {...createProps({ hostKind: "ready", onOpenWorkspace, workspaces: [] })} />,
    );
    await user.click(screen.getByRole("button", { name: "Open a local workspace" }));
    expect(onOpenWorkspace).toHaveBeenCalledTimes(1);
    view.unmount();

    // A host that cannot list projects cannot open one either, so the same
    // control here would be a button that does nothing.
    render(<HomeView {...createProps({
      canOpenFolder: false,
      hostKind: "unavailable",
      workspaces: [],
    })} />);
    expect(screen.queryByRole("button", { name: "Open a local workspace" })).toBeNull();
    expect(screen.getByRole("button", { name: "Open workspace" }))
      .toHaveProperty("disabled", true);
  });

  it("offers the host retry only when the shell supplies one", async () => {
    const onRetryHost = vi.fn();
    const user = userEvent.setup();
    const view = render(<HomeView {...createProps({ onRetryHost })} />);
    await user.click(screen.getByRole("button", { name: "Retry host" }));
    expect(onRetryHost).toHaveBeenCalledTimes(1);
    view.unmount();

    render(<HomeView {...createProps()} />);
    expect(screen.queryByRole("button", { name: "Retry host" })).toBeNull();
  });

  it("reopens a closed task by name, and closing is not deleting", async () => {
    const onReopenTask = vi.fn();
    const onSelectTask = vi.fn();
    const user = userEvent.setup();
    render(<HomeView {...createProps({
      closedTasks: [{ id: "task-9", title: "Trace the boot sequence", updatedAt: "Now" }],
      onReopenTask,
      onSelectTask,
    })} />);

    const closed = screen.getByRole("list", { name: "Closed tasks" });
    expect(within(closed).getByText("Trace the boot sequence")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Reopen task Trace the boot sequence" }));

    expect(onReopenTask.mock.calls).toEqual([["task-9"]]);
    expect(onSelectTask).not.toHaveBeenCalled();
  });

  it("keeps the closed section off the surface when nothing is closed", () => {
    render(<HomeView {...createProps()} />);
    expect(screen.queryByRole("list", { name: "Closed tasks" })).toBeNull();
    expect(screen.queryByRole("heading", { name: "Closed tasks" })).toBeNull();
  });

  it("starts a task from the composer with the trimmed intent, once", async () => {
    const onSubmitIntent = vi.fn();
    const user = userEvent.setup();
    render(<HomeView {...createProps({ onSubmitIntent })} />);

    await user.type(screen.getByLabelText("Describe your intent"), "  Plan the rollout  ");
    await user.click(screen.getByRole("button", { name: "Start task" }));

    expect(onSubmitIntent.mock.calls).toEqual([["Plan the rollout"]]);
    expect(screen.getByLabelText("Describe your intent")).toHaveProperty("value", "");
  });

  it("keeps the section labels the rest of the suite navigates Home by", () => {
    // `App.test.tsx` and the shell integration suite find their way around Home
    // through these headings — the Recent tasks header is how they disambiguate
    // Home's New task from the title strip's. Renaming one here breaks a suite
    // that has no idea this component moved.
    render(<HomeView {...createProps({
      closedTasks: [{ id: "task-9", title: "Trace the boot sequence", updatedAt: "Now" }],
    })} />);

    expect(screen.getByRole("heading", { name: "Projects" })).toBeTruthy();
    const recent = screen.getByRole("heading", { name: "Recent tasks" }).closest("section");
    expect(recent).not.toBeNull();
    expect(within(recent as HTMLElement).getByRole("button", { name: "New task" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Closed tasks" })).toBeTruthy();
  });

  it("names every control it puts on the surface", () => {
    // The existing suites query Home by role and accessible name, so a control
    // the tests cannot name is a control a screen reader cannot name either.
    render(<HomeView {...createProps({
      closedTasks: [{ id: "task-9", title: "Trace the boot sequence", updatedAt: "Now" }],
      onRetryHost: vi.fn(),
      recentTasks: [{ id: "task-1", title: "Apple", updatedAt: "2h ago" }],
      workspaces: [alpha, beta],
    })} />);

    const names = buttonNames();
    expect(names.length).toBeGreaterThan(0);
    expect(names.filter((name) => name.length === 0)).toEqual([]);
    for (const expected of [
      "Open workspace",
      "Manage workspaces",
      "Retry host",
      "Start task",
      "New task",
      "Resume task Apple",
      "Reopen task Trace the boot sequence",
    ]) {
      expect(names, expected).toContain(expected);
    }
  });
});
