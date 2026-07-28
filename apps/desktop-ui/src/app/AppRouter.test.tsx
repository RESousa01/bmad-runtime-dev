import "../test/setup";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AppRouter, resolveAppView, type AppRouteInput } from "./AppRouter";

afterEach(cleanup);

function input(overrides: Partial<AppRouteInput> = {}): AppRouteInput {
  return {
    route: { kind: "task", taskId: "t1" },
    hasSelectedTask: true,
    hasWorkspace: true,
    ...overrides,
  };
}

describe("resolveAppView", () => {
  it("shows Home for the home route", () => {
    expect(resolveAppView(input({ route: { kind: "home" } }))).toBe("home");
  });

  it("shows Home for a task route with nothing selected", () => {
    // The rule this file exists to make visible. There is no phantom
    // "New task" record to fall back onto, so an empty list, a closed last tab
    // and a deleted last task all land on Home rather than on a tab that
    // cannot be acted on.
    expect(resolveAppView(input({ hasSelectedTask: false }))).toBe("home");
    expect(resolveAppView(input({
      route: { kind: "task", taskId: null },
      hasSelectedTask: false,
    }))).toBe("home");
  });

  it("shows Home rather than the empty state when nothing is selected and no workspace is bound", () => {
    // Order matters: "nothing selected" wins over "no workspace". Showing the
    // no-workspace state here would tell the user to pick a folder when what
    // they actually have is no task.
    expect(resolveAppView(input({ hasSelectedTask: false, hasWorkspace: false })))
      .toBe("home");
  });

  it("shows the empty state for a real task with no workspace behind it", () => {
    // Nothing in the task view works without a workspace, and showing the task
    // chrome over an unbound one invites edits that cannot be applied.
    expect(resolveAppView(input({ hasWorkspace: false }))).toBe("no-workspace");
  });

  it("shows the task when both a task and a workspace are present", () => {
    expect(resolveAppView(input())).toBe("task");
  });
});

describe("AppRouter", () => {
  function renderRouter(overrides: Partial<AppRouteInput> = {}) {
    const renderTask = vi.fn(() => <p>task view</p>);
    const renderHome = vi.fn(() => <p>home view</p>);
    const renderNoWorkspace = vi.fn(() => <p>no workspace</p>);
    render(
      <AppRouter
        {...input(overrides)}
        renderHome={renderHome}
        renderNoWorkspace={renderNoWorkspace}
        renderTask={renderTask}
      />,
    );
    return { renderHome, renderNoWorkspace, renderTask };
  }

  it("renders the view the rule selects", () => {
    renderRouter();
    expect(screen.getByText("task view")).toBeTruthy();
  });

  it("does not build the views it will not show", () => {
    // Thunks rather than elements: the task view cannot be constructed without
    // a selected task, so building it eagerly to hand to a router that will
    // not render it is how a null dereference gets introduced by a refactor.
    const { renderHome, renderNoWorkspace, renderTask } = renderRouter({
      hasSelectedTask: false,
    });
    expect(renderHome).toHaveBeenCalled();
    expect(renderTask).not.toHaveBeenCalled();
    expect(renderNoWorkspace).not.toHaveBeenCalled();
  });

  it("renders the empty state for a task with no workspace", () => {
    renderRouter({ hasWorkspace: false });
    expect(screen.getByText("no workspace")).toBeTruthy();
  });
});
