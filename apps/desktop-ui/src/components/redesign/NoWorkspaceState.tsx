import { Button } from "@sapphirus/ui";
import { FolderOpen, Play } from "lucide-react";

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
  onTryDemo?: () => void;
}

const defaultCopy: Record<NoWorkspaceMode, string> = {
  browser_demo:
    "The Windows desktop host is not connected. You can explore Demo mode without local workspace access.",
  loading: "Verifying the signed Windows desktop host before workspace access.",
  read_only_recovery:
    "Workspace changes are unavailable while local authority is in read-only recovery.",
  ready:
    "Choose a local workspace to start a task. Workspace access remains under desktop host authority.",
  unavailable:
    "The signed Windows desktop host is unavailable, so a local workspace cannot be opened.",
};

export function NoWorkspaceState({
  copy,
  mode,
  onOpenWorkspace,
  onTryDemo,
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
        {mode === "browser_demo" && onTryDemo ? (
          <Button
            onPress={onTryDemo}
            size="large"
            variant="secondary"
          >
            <Play aria-hidden="true" size={16} strokeWidth={1.8} />
            Try demo
          </Button>
        ) : null}
      </div>
    </section>
  );
}
