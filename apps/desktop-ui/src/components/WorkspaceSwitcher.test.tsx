import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceProjection } from "../lib/hostClient";
import { WorkspaceSwitcher, type WorkspaceSwitcherProps } from "./WorkspaceSwitcher";

afterEach(cleanup);

function workspace(id: string, name: string): WorkspaceProjection {
  return {
    workspaceId: id,
    projectId: `project_${id}`,
    displayName: name,
    grantEpoch: 1,
    permissions: "read_only",
    contextReadEpoch: 1,
    governedEditEpoch: 1,
  };
}

function createProps(
  overrides: Partial<WorkspaceSwitcherProps> = {},
): WorkspaceSwitcherProps {
  return {
    activeWorkspaceId: "workspace_a",
    canOpenFolder: true,
    onActivate: vi.fn(),
    onOpenFolder: vi.fn(),
    onOpenManager: vi.fn(),
    statusLabel: "Read only",
    workspaces: [
      workspace("workspace_a", "alpha-project"),
      workspace("workspace_b", "beta-project"),
    ],
    ...overrides,
  };
}

describe("WorkspaceSwitcher", () => {
  it("stays closed until the breadcrumb is pressed", () => {
    render(<WorkspaceSwitcher {...createProps()} />);
    expect(screen.queryByLabelText("Switch workspace")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /Manage workspace alpha-project/ }));
    expect(screen.getByLabelText("Switch workspace")).toBeTruthy();
  });

  it("filters by search and activates only a different workspace", () => {
    const props = createProps();
    render(<WorkspaceSwitcher {...props} />);
    fireEvent.click(screen.getByRole("button", { name: /Manage workspace/ }));
    fireEvent.change(screen.getByLabelText("Search workspaces"), {
      target: { value: "beta" },
    });
    const popover = screen.getByLabelText("Switch workspace");
    expect(within(popover).queryByText("alpha-project")).toBeNull();
    fireEvent.click(within(popover).getByText("beta-project"));
    expect(props.onActivate).toHaveBeenCalledWith("workspace_b");
    expect(screen.queryByLabelText("Switch workspace")).toBeNull();
  });

  it("does not re-activate the current workspace", () => {
    const props = createProps();
    render(<WorkspaceSwitcher {...props} />);
    fireEvent.click(screen.getByRole("button", { name: /Manage workspace/ }));
    const popover = screen.getByLabelText("Switch workspace");
    fireEvent.click(within(popover).getByText("alpha-project"));
    expect(props.onActivate).not.toHaveBeenCalled();
  });

  it("routes folder opening and manager rows and hides Open folder when unavailable", () => {
    const props = createProps();
    const { unmount } = render(<WorkspaceSwitcher {...props} />);
    fireEvent.click(screen.getByRole("button", { name: /Manage workspace/ }));
    fireEvent.click(screen.getByText("Open folder…"));
    expect(props.onOpenFolder).toHaveBeenCalledOnce();

    fireEvent.click(screen.getByRole("button", { name: /Manage workspace/ }));
    fireEvent.click(screen.getByText("Manage workspaces…"));
    expect(props.onOpenManager).toHaveBeenCalledOnce();
    unmount();

    render(<WorkspaceSwitcher {...createProps({ canOpenFolder: false })} />);
    fireEvent.click(screen.getByRole("button", { name: /Manage workspace/ }));
    expect(screen.queryByText("Open folder…")).toBeNull();
  });

  it("closes on Escape without activating anything", () => {
    const props = createProps();
    render(<WorkspaceSwitcher {...props} />);
    fireEvent.click(screen.getByRole("button", { name: /Manage workspace/ }));
    fireEvent.keyDown(screen.getByLabelText("Search workspaces"), { key: "Escape" });
    expect(screen.queryByLabelText("Switch workspace")).toBeNull();
    expect(props.onActivate).not.toHaveBeenCalled();
  });
});
