import { Button } from "@sapphirus/ui";
import { Check, FolderPlus, HardDrive, ShieldAlert, Trash2, X } from "lucide-react";
import { useEffect, useRef, type KeyboardEvent } from "react";
import type { WorkspaceProjection } from "../lib/hostClient";

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
      return "Workspace management is available in the Windows desktop host. This browser view is for visual QA only.";
    case "read_only_recovery":
      return "Workspace changes are blocked while the local authority store is in read-only recovery.";
    case "unavailable":
      return "The signed Windows host is unavailable, so no folder can be selected.";
    case "loading":
      return "Verifying the signed Windows host…";
    case "ready":
      return "Switch between existing workspaces or choose a fixed local NTFS folder with the native Windows picker.";
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
      className="workspace-panel"
      onKeyDown={containFocus}
      ref={panelRef}
      role="dialog"
    >
      <header>
        <div>
          <span className="workspace-panel__eyebrow">Workspace access</span>
          <h2>Workspaces</h2>
        </div>
        <Button aria-label="Close workspaces" onPress={onClose} size="icon" variant="quiet">
          <X aria-hidden="true" size={18} />
        </Button>
      </header>

      <p className="workspace-panel__intro">{unavailableCopy(mode)}</p>

      {workspaces.length > 0 ? (
        <div aria-label="Available local workspaces" className="workspace-panel__list" role="list">
          {workspaces.map((workspace) => (
            <div className="workspace-panel__row" key={workspace.workspaceId} role="listitem">
              <span className="workspace-panel__icon"><HardDrive aria-hidden="true" size={18} /></span>
              <div className="workspace-panel__details">
                <strong>{workspace.displayName}</strong>
                <span>
                  {mode === "browser_demo" ? "Preview workspace · no local access" : "Local workspace · Read only"}
                </span>
              </div>
              <div className="workspace-panel__actions">
                {activeWorkspaceId === workspace.workspaceId ? (
                  <span aria-label="Current workspace" className="workspace-panel__current">
                    <Check aria-hidden="true" size={14} />
                    Current
                  </span>
                ) : (
                  <Button
                    aria-label={`Switch to workspace ${workspace.displayName}`}
                    isDisabled={!canActivate || isBusy}
                    onPress={() => onActivate(workspace.workspaceId)}
                    size="small"
                    variant="secondary"
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
                  size="small"
                  variant="quiet"
                >
                  <Trash2 aria-hidden="true" size={14} />
                  {busyWorkspaceId === workspace.workspaceId ? "Removing…" : "Remove workspace"}
                </Button>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="workspace-panel__empty">
          <HardDrive aria-hidden="true" size={23} />
          <strong>No local workspace selected</strong>
          <span>Choose a folder for read-only access. Its absolute path stays in the Windows host.</span>
        </div>
      )}

      {workspaceError ? (
        <p className="workspace-panel__error" role="alert">
          <ShieldAlert aria-hidden="true" size={16} />
          {workspaceError}
        </p>
      ) : null}

      <Button
        className="workspace-panel__select"
        isDisabled={!canSelect || isBusy}
        onPress={onSelect}
        size="large"
        variant="primary"
      >
        <FolderPlus aria-hidden="true" size={17} />
        {isSelecting ? "Opening Windows picker…" : "Choose local workspace"}
      </Button>
      <p className="workspace-panel__privacy">
        Remove workspace ends Sapphirus access only; files stay unchanged. Absolute paths never enter renderer state.
      </p>
    </section>
  );
}
