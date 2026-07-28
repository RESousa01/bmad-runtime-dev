import { describe, expect, it } from "vitest";

import { hasRenderableModal } from "./AppModals";
import type { ShellModel } from "./AppProviders";

type Deletion = ShellModel["deletion"];

const pending: Deletion = {
  cancel: () => undefined,
  confirm: () => undefined,
  historyStatus: "loaded",
  mayHaveDurableCopy: false,
  messageCount: 0,
  taskTitle: "Fix login",
};

describe("hasRenderableModal", () => {
  it("is true for the two dialogs that always render", () => {
    expect(hasRenderableModal("workspace-manager", null)).toBe(true);
    expect(hasRenderableModal("settings", null)).toBe(true);
  });

  it("is false when nothing is open", () => {
    expect(hasRenderableModal(null, null)).toBe(false);
  });

  /// `AppShellLayout` decides focus containment and where Escape goes from
  /// whether it was *given* a modal. An element that renders nothing traps
  /// focus in an empty container and routes Escape to a dialog that is not
  /// there.
  it("is false for a modal state with no dialog behind it", () => {
    // `"account"` is reachable through `openUtilityPanel` and has no dialog.
    expect(hasRenderableModal("account", null)).toBe(false);
  });

  it("is false for a deletion whose task has already gone", () => {
    expect(hasRenderableModal("delete-task", null)).toBe(false);
  });

  it("is true for a deletion with a task pending", () => {
    expect(hasRenderableModal("delete-task", pending)).toBe(true);
  });
});
