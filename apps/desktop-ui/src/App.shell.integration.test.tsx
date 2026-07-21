// @vitest-environment jsdom
import "./test/setup";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";
import {
  createReadyShellRuntime,
  dispatchedCommands,
  primaryShellWorkspace,
  secondaryShellWorkspace,
} from "./test/shellFixtures";

function contextualSurface(name: string): HTMLElement | null {
  return screen.queryByRole("complementary", { name })
    ?? screen.queryByRole("dialog", { name });
}

describe("App task-shell integration", () => {
  it("keeps one Task route while Files is a contextual drawer and Settings is a modal", async () => {
    const { runtime } = await createReadyShellRuntime([primaryShellWorkspace]);
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await screen.findAllByText("Local host ready");

    const task = screen.getByRole("main");
    expect(within(task).getByRole("heading", { name: "New task" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "New task" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Attach files" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Changes" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Run details" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Skills and agents" })).toBeTruthy();
    expect(screen.queryByRole("complementary", { name: "Inspector" })).toBeNull();
    expect(screen.queryByText(/^Inspector$/)).toBeNull();

    await user.click(screen.getByRole("button", { name: "Attach files" }));
    expect(contextualSurface("Files")).toBeTruthy();
    expect(screen.getByRole("main")).toBe(task);
    expect(within(task).getByRole("heading", { name: "New task" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("dialog", { name: "Settings" })).toBeTruthy();
    expect(screen.getByRole("main", { hidden: true })).toBe(task);
  });

  it("offers exactly one ready-state Open workspace action and dispatches it once", async () => {
    const { runtime, invoke } = await createReadyShellRuntime();
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await screen.findByRole("region", { name: "No workspace open" });
    const openWorkspaceActions = screen.getAllByRole("button", { name: "Open workspace" });
    expect(openWorkspaceActions).toHaveLength(1);

    await user.click(openWorkspaceActions[0]!);
    await waitFor(() => {
      expect(dispatchedCommands(invoke)).toEqual(["workspace.select_folder"]);
    });
  });

  it("does not dispatch domain commands for transient Run details, Settings, or Account UI", async () => {
    const { runtime, invoke } = await createReadyShellRuntime([primaryShellWorkspace]);
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await screen.findAllByText("Local host ready");
    expect(dispatchedCommands(invoke)).toEqual([]);

    await user.click(screen.getByRole("button", { name: "Run details" }));
    expect(contextualSurface("Activity")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Close Activity" }));

    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("dialog", { name: "Settings" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Close settings" }));

    await user.click(screen.getByRole("button", { name: "Account" }));
    expect(screen.getByRole("dialog", { name: "Settings" })).toBeTruthy();
    expect(screen.getByText("Organization sign-in is not configured for this build.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Close settings" }));

    expect(dispatchedCommands(invoke)).toEqual([]);
  });

  it("closes stale contextual UI when the task or workspace changes", async () => {
    const { runtime } = await createReadyShellRuntime([
      primaryShellWorkspace,
      secondaryShellWorkspace,
    ]);
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await screen.findAllByText("primary-workspace");
    await user.click(screen.getByRole("button", { name: "Run details" }));
    expect(contextualSurface("Activity")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "New task" }));
    expect(contextualSurface("Activity")).toBeNull();

    await user.click(screen.getByRole("button", { name: "Attach files" }));
    expect(contextualSurface("Files")).toBeTruthy();
    await user.click(screen.getByRole("button", {
      name: "Manage workspace primary-workspace",
    }));
    const workspaceManager = screen.getByRole("dialog", { name: "Local workspaces" });
    await user.click(within(workspaceManager).getByRole("button", {
      name: "Switch to workspace secondary-workspace",
    }));
    await user.click(within(workspaceManager).getByRole("button", {
      name: "Close workspaces",
    }));
    expect(contextualSurface("Files")).toBeNull();
  });

  it("keeps named Settings and Account reachable from the title strip in narrow mode", async () => {
    const originalMatchMedia = window.matchMedia;
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: (query: string) => ({
        addEventListener: () => undefined,
        dispatchEvent: () => false,
        matches: query.includes("max-width"),
        media: query,
        onchange: null,
        removeEventListener: () => undefined,
      }),
    });
    const user = userEvent.setup();
    const view = render(<App />);

    try {
      await screen.findAllByText("Browser preview");
      await user.click(screen.getByRole("button", { name: "Settings" }));
      expect(screen.getByRole("dialog", { name: "Settings" })).toBeTruthy();
      await user.click(screen.getByRole("button", { name: "Close settings" }));

      await user.click(screen.getByRole("button", { name: "Account" }));
      expect(screen.getByRole("dialog", { name: "Settings" })).toBeTruthy();
    } finally {
      view.unmount();
      Object.defineProperty(window, "matchMedia", {
        configurable: true,
        value: originalMatchMedia,
      });
    }
  });
});
