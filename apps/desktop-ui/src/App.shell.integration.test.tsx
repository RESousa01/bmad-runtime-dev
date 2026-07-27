// @vitest-environment jsdom
import "./test/setup";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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

async function enterFirstTask(): Promise<void> {
  // Home offers "New task" in both the title strip and the Recent tasks
  // header, so scope to the title strip to keep the query unambiguous.
  const titlebar = await screen.findByRole("banner", { name: "Forge application" });
  fireEvent.click(within(titlebar).getByRole("button", { name: "New task" }));
}

function openWorkspaceFiles(): void {
  fireEvent.click(screen.getByRole("button", { name: "Open app menu" }));
  fireEvent.click(screen.getByRole("button", { name: "Show workspace files" }));
}

describe("App task-shell integration", () => {
  it("renders the Forge entry as an OpenCode-grounded shell", async () => {
    render(<App />);

    const shell = document.querySelector(".app-shell--forge-v2");
    expect(shell).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Projects" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Recent tasks" })).toBeTruthy();
    expect(screen.getByRole("form", { name: "Start a task" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "What do you want to build?" })).toBeNull();
    expect(screen.queryByText("Explore Forge safely")).toBeNull();

    const titlebar = screen.getByRole("banner", { name: "Forge application" });
    expect(titlebar.querySelector('[data-forge-icon="grid-plus"]')).toBeTruthy();
    expect(within(titlebar).queryByText("Forge")).toBeNull();
  });

  it("keeps one Task route while Files is a contextual drawer and Settings is a modal", async () => {
    const { runtime } = await createReadyShellRuntime([primaryShellWorkspace]);
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await enterFirstTask();
    await screen.findAllByText("Local host ready");

    const task = screen.getByRole("main");
    expect(within(task).getByRole("heading", { name: "New task" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "New task" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Add files or folder" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Review" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Run details" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Skills and agents" })).toBeTruthy();
    expect(screen.queryByRole("complementary", { name: "Inspector" })).toBeNull();
    expect(screen.queryByText(/^Inspector$/)).toBeNull();

    openWorkspaceFiles();
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
    await enterFirstTask();

    const openWorkspaceActions = await screen.findAllByRole("button", { name: "Open workspace" });
    expect(openWorkspaceActions).toHaveLength(1);

    await user.click(openWorkspaceActions[0]!);
    await waitFor(() => {
      expect(dispatchedCommands(invoke)).toEqual([
        "bmad.library.snapshot",
        "workspace.select_folder",
      ]);
    });
  });

  it("does not add domain commands for transient Run details or Settings UI", async () => {
    const { runtime, invoke } = await createReadyShellRuntime([primaryShellWorkspace]);
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await enterFirstTask();

    await screen.findAllByText("Local host ready");
    expect(dispatchedCommands(invoke)).toEqual(["bmad.library.snapshot"]);

    await user.click(screen.getByRole("button", { name: "Run details" }));
    expect(contextualSurface("Activity")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Close Activity" }));

    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("dialog", { name: "Settings" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Close settings" }));

    expect(dispatchedCommands(invoke)).toEqual(["bmad.library.snapshot"]);
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
    await enterFirstTask();

    await screen.findAllByText("primary-workspace");
    await user.click(screen.getByRole("button", { name: "Run details" }));
    expect(contextualSurface("Activity")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "New task" }));
    expect(contextualSurface("Activity")).toBeNull();

    openWorkspaceFiles();
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

  it("keeps Settings reachable and omits account UI in the local prototype", async () => {
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
    await enterFirstTask();

    try {
      await screen.findAllByText("Browser preview");
      await user.click(screen.getByRole("button", { name: "Settings" }));
      expect(screen.getByRole("dialog", { name: "Settings" })).toBeTruthy();
      await user.click(screen.getByRole("button", { name: "Close settings" }));

      expect(screen.queryByRole("button", { name: "Account" })).toBeNull();
    } finally {
      view.unmount();
      Object.defineProperty(window, "matchMedia", {
        configurable: true,
        value: originalMatchMedia,
      });
    }
  });
/// The palette declares `aria-modal="true"`. Nothing behind it may be
  /// reachable while it is open, and until the shell was told about it the
  /// title bar and the whole workbench stayed focusable and stayed exposed.
  it("takes the shell out of reach while the command palette is open", async () => {
    const { runtime } = await createReadyShellRuntime([primaryShellWorkspace]);
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await enterFirstTask();
    await screen.findAllByText("Local host ready");

    const titlebar = screen.getByRole("banner", { name: "Forge application" });
    const workbench = document.querySelector(".workbench");
    expect(workbench).toBeTruthy();
    expect(titlebar.hasAttribute("inert")).toBe(false);
    expect(workbench?.hasAttribute("inert")).toBe(false);

    await user.click(within(titlebar).getByRole("button", { name: "Open app menu" }));
    expect(screen.getByRole("dialog", { name: "Command palette" })).toBeTruthy();

    expect(titlebar.hasAttribute("inert")).toBe(true);
    expect(workbench?.hasAttribute("inert")).toBe(true);
    expect(workbench?.getAttribute("aria-hidden")).toBe("true");

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog", { name: "Command palette" })).toBeNull();
    expect(titlebar.hasAttribute("inert")).toBe(false);
    expect(workbench?.hasAttribute("inert")).toBe(false);
  });
});
