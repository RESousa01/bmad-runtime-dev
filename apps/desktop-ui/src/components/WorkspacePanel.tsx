import { Button } from "@sapphirus/ui";
import { useEffect, useRef } from "react";
import type { WorkspaceProjection } from "../lib/hostClient";
import { focusableElements } from "../lib/panelFocus";
import { OpenCodeIcon } from "./OpenCodeIcon";

export interface WorkspacePanelProps {
  activeWorkspaceId: string | null;
  busyWorkspaceId: string | null;
  canActivate: boolean;
  canRemove: boolean;
  canSelect: boolean;
  isSelecting: boolean;
  mode: "browser_demo" | "loading" | "ready" | "read_only_recovery" | "unavailable";
  onActivate: (workspaceId: string) => void;
  onClose: () => void;
  onRemove: (workspaceId: string) => void;
  onSelect: () => void;
  workspaceError: string | null;
  /// `readonly`: the panel only lists these. A mutable array here let a view
  /// reorder the workspace list the controller owns.
  workspaces: readonly WorkspaceProjection[];
}

function unavailableCopy(mode: WorkspacePanelProps["mode"]): string {
  switch (mode) {
    case "browser_demo":
      return "Access changes require the Windows desktop host; this view uses sample data.";
    case "read_only_recovery":
      return "Workspace changes are blocked while the local authority store is in read-only recovery.";
    case "unavailable":
      return "The Windows host is unavailable.";
    case "loading":
      return "Connecting to the Windows host…";
    case "ready":
      return "Local workspace access is controlled by the desktop host.";
  }
}

export function WorkspacePanel({
  activeWorkspaceId,
  busyWorkspaceId,
  canActivate,
  canRemove,
  canSelect,
  isSelecting,
  mode,
  onActivate,
  onClose,
  onRemove,
  onSelect,
  workspaceError,
  workspaces,
}: WorkspacePanelProps) {
  const panelRef = useRef<HTMLElement>(null);
  const previousBusyWorkspaceId = useRef<string | null>(null);

  // Entry focus and Tab containment are *not* handled here. This panel is
  // rendered into `AppShellLayout`'s modal slot, whose wrapper already runs
  // `useModalPanelFocus` + `containModalPanelFocus`. This component used to
  // duplicate both against its own inline selector — one that lacked `summary`,
  // the `[hidden]/[aria-hidden]/[inert]` filter, the collapsed-disclosure
  // filter, and the document-order sort that `focusableElements` applies. Two
  // handlers ran on every Tab, and the inner one could move focus somewhere the
  // outer one did not agree was in the ring. Deleting them changed no test.
  //
  // What is left is the one behaviour the shared trap does not cover: a removal
  // unmounts the row that held focus, so focus has to be put back somewhere.
  useEffect(() => {
    const removalCompleted = previousBusyWorkspaceId.current !== null
      && busyWorkspaceId === null;
    previousBusyWorkspaceId.current = busyWorkspaceId;
    if (removalCompleted && panelRef.current) {
      // The shared definition of "focusable", so this cannot drift from what
      // the trap wraps around.
      focusableElements(panelRef.current).at(0)?.focus();
    }
  }, [busyWorkspaceId]);

  const isBusy = isSelecting || busyWorkspaceId !== null;

  return (
    <section
      aria-label="Local workspaces"
      aria-busy={isBusy}
      aria-modal="true"
      className="workspace-panel workspace-panel--opencode-v2"
      data-component="workspace-manager-v2"
      data-source="opencode"
      ref={panelRef}
      role="dialog"
    >
      <header className="workspace-panel__header">
        <div className="workspace-panel__title">
          <OpenCodeIcon name="workspace" size={16} />
          <div>
            <h2>Workspaces</h2>
            <span>{mode === "ready" ? "Local access" : mode === "browser_demo" ? "Browser demo" : "Host status"}</span>
          </div>
        </div>
        <Button aria-label="Close workspaces" onPress={onClose} size="icon" variant="quiet">
          <OpenCodeIcon name="close" size={20} />
        </Button>
      </header>

      <div className="workspace-panel__body">
        {mode === "ready" ? null : (
          <p className="workspace-panel__mode">
            <OpenCodeIcon name="status" size={16} />
            <span>{unavailableCopy(mode)}</span>
          </p>
        )}

        {workspaces.length > 0 ? (
          <div aria-label="Available local workspaces" className="workspace-panel__list" role="list">
            {workspaces.map((workspace) => (
              <div
                className={`workspace-panel__row${activeWorkspaceId === workspace.workspaceId ? " workspace-panel__row--current" : ""}`}
                key={workspace.workspaceId}
                role="listitem"
              >
                <span className="workspace-panel__icon"><OpenCodeIcon name="folder" size={16} /></span>
                <div className="workspace-panel__details">
                  <strong>{workspace.displayName}</strong>
                  <span>
                    {mode === "browser_demo"
                      ? "Sample workspace · no local access"
                      : workspace.permissions === "governed_edits"
                        ? "Governed edits"
                        : "Read only"}
                  </span>
                </div>
                <div className="workspace-panel__actions">
                  {activeWorkspaceId === workspace.workspaceId ? (
                    <span aria-label="Current workspace" className="workspace-panel__current">
                      <OpenCodeIcon name="check" size={14} />
                      Current
                    </span>
                  ) : (
                    <Button
                      aria-label={`Switch to workspace ${workspace.displayName}`}
                      isDisabled={!canActivate || isBusy}
                      onPress={() => onActivate(workspace.workspaceId)}
                      size="small"
                      variant="quiet"
                    >
                      Switch
                    </Button>
                  )}
                  <Button
                    aria-label={busyWorkspaceId === workspace.workspaceId
                      ? `Removing workspace ${workspace.displayName}`
                      : `Remove workspace ${workspace.displayName}`}
                    isDisabled={!canRemove || isBusy}
                    onPress={() => onRemove(workspace.workspaceId)}
                    size="icon"
                    title="Remove Forge access"
                    variant="quiet"
                  >
                    <OpenCodeIcon name="trash" size={16} />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="workspace-panel__empty">
            <OpenCodeIcon name="workspace" size={20} />
            <strong>No local workspace selected</strong>
            <span>Choose a folder for read-only access.</span>
          </div>
        )}

        {workspaceError ? (
          <p className="workspace-panel__error" role="alert">
            <OpenCodeIcon name="status" size={16} />
            {workspaceError}
          </p>
        ) : null}
      </div>

      <footer className="workspace-panel__footer">
        <Button
          className="workspace-panel__select"
          isDisabled={!canSelect || isBusy}
          onPress={onSelect}
          size="small"
          variant="secondary"
        >
          <OpenCodeIcon name="folder-add-left" size={16} />
          {isSelecting ? "Opening Windows picker…" : "Choose local workspace"}
        </Button>
        <p className="workspace-panel__privacy">
          Removing access never changes files.
          <span className="sr-only"> Absolute paths never enter renderer state.</span>
        </p>
      </footer>
    </section>
  );
}
