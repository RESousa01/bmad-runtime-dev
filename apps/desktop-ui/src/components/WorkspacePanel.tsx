import { Button } from "@sapphirus/ui";
import { useEffect, useRef, type KeyboardEvent } from "react";
import type { WorkspaceProjection } from "../lib/hostClient";
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
  workspaces: WorkspaceProjection[];
}

function unavailableCopy(mode: WorkspacePanelProps["mode"]): string {
  switch (mode) {
    case "browser_demo":
      return "Access changes require the Windows desktop host; this view uses sample data.";
    case "read_only_recovery":
      return "Workspace changes are blocked while the local authority store is in read-only recovery.";
    case "unavailable":
      return "The signed Windows host is unavailable.";
    case "loading":
      return "Verifying the signed Windows host…";
    case "ready":
      return "Local workspace access is controlled by the signed desktop host.";
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

  useEffect(() => {
    panelRef.current?.querySelector<HTMLElement>("button:not([disabled])")?.focus();
  }, []);

  useEffect(() => {
    const removalCompleted = previousBusyWorkspaceId.current !== null
      && busyWorkspaceId === null;
    previousBusyWorkspaceId.current = busyWorkspaceId;
    if (removalCompleted) {
      panelRef.current?.querySelector<HTMLElement>("button:not([disabled])")?.focus();
    }
  }, [busyWorkspaceId]);

  const isBusy = isSelecting || busyWorkspaceId !== null;

  function containFocus(event: KeyboardEvent<HTMLElement>) {
    if (event.key !== "Tab") {
      return;
    }
    const focusable = Array.from(panelRef.current?.querySelectorAll<HTMLElement>(
      "button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])",
    ) ?? []);
    if (focusable.length === 0) {
      event.preventDefault();
      return;
    }
    const first = focusable[0]!;
    const last = focusable.at(-1)!;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  return (
    <section
      aria-label="Local workspaces"
      aria-busy={isBusy}
      aria-modal="true"
      className="workspace-panel workspace-panel--opencode-v2"
      data-component="workspace-manager-v2"
      data-source="opencode"
      onKeyDown={containFocus}
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
