import { Button } from "@sapphirus/ui";
import { FolderOpen, RotateCcw } from "lucide-react";

export type NoWorkspaceMode =
  | "browser_demo"
  | "loading"
  | "read_only_recovery"
  | "ready"
  | "unavailable";

export interface NoWorkspaceStateProps {
  copy?: string;
  mode: NoWorkspaceMode;
  onOpenWorkspace: () => void;
  /// Re-runs host verification. Supplied only when the host is unavailable —
  /// that state is otherwise terminal for the life of the window.
  onRetryHost?: (() => void) | undefined;
}

const defaultCopy: Record<NoWorkspaceMode, string> = {
  browser_demo:
    "The Windows desktop host is not connected. You can explore Demo mode without local workspace access.",
  loading: "Connecting to the Windows desktop host before workspace access.",
  read_only_recovery:
    "Workspace changes are unavailable while local authority is in read-only recovery.",
  ready:
    "Choose a local workspace to start a task. Workspace access remains under desktop host authority.",
  unavailable:
    "The Windows desktop host is unavailable, so a local workspace cannot be opened.",
};

export function NoWorkspaceState({
  copy,
  mode,
  onOpenWorkspace,
  onRetryHost,
}: NoWorkspaceStateProps) {
  return (
    <section aria-label="No workspace open" className="no-workspace-state">
      <div aria-hidden="true" className="no-workspace-state__icon">
        <FolderOpen size={25} strokeWidth={1.7} />
      </div>
      <p className="no-workspace-state__eyebrow">Workspace required</p>
      <h1>Open a workspace to begin</h1>
      <p className="no-workspace-state__copy">{copy ?? defaultCopy[mode]}</p>
      <div className="no-workspace-state__actions">
        <Button
          isDisabled={mode !== "ready"}
          onPress={onOpenWorkspace}
          size="large"
          variant="primary"
        >
          <FolderOpen aria-hidden="true" size={17} strokeWidth={1.8} />
          Open workspace
        </Button>
        {onRetryHost ? (
          <Button onPress={onRetryHost} size="large" variant="secondary">
            <RotateCcw aria-hidden="true" size={17} strokeWidth={1.8} />
            Try again
          </Button>
        ) : null}
      </div>
    </section>
  );
}
