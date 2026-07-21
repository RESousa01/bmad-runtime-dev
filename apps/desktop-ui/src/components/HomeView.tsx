import { Button } from "@sapphirus/ui";
import { Send } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import type { WorkspaceProjection } from "../lib/hostClient";
import { BrandMark } from "./BrandMark";
import { WorkspaceSwitcher } from "./WorkspaceSwitcher";

export interface HomeViewProps {
  readonly activeWorkspaceId: string | null;
  readonly canOpenFolder: boolean;
  readonly composerDisabled: boolean;
  readonly hasWorkspace: boolean;
  readonly onActivateWorkspace: (workspaceId: string) => void;
  readonly onOpenWorkspaceManager: () => void;
  readonly onOpenWorkspace: () => void;
  readonly onSubmitIntent: (intent: string) => void;
  readonly statusHint: string;
  readonly workspaces: readonly WorkspaceProjection[];
  readonly workspaceStatusLabel: string;
}

/**
 * The landing view: a centered hero and composer with the bound workspace
 * as a quiet breadcrumb underneath. Submitting routes into a fresh task.
 */
export function HomeView({
  activeWorkspaceId,
  canOpenFolder,
  composerDisabled,
  hasWorkspace,
  onActivateWorkspace,
  onOpenWorkspaceManager,
  onOpenWorkspace,
  onSubmitIntent,
  statusHint,
  workspaces,
  workspaceStatusLabel,
}: HomeViewProps) {
  const [intent, setIntent] = useState("");
  const inputRef = useRef<HTMLInputElement | null>(null);
  const trimmed = intent.trim();

  useEffect(() => {
    // Land with the composer focused so typing starts a task immediately
    // (and stray activation keys from app launch are absorbed harmlessly).
    inputRef.current?.focus();
  }, []);

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!composerDisabled && trimmed.length > 0) {
      onSubmitIntent(trimmed);
      setIntent("");
    }
  };

  return (
    <main aria-label="Home" className="home-view">
      <div className="home-view__hero">
        <div className="empty-session__mark">
          <BrandMark size={44} />
        </div>
        <span aria-hidden="true" className="empty-session__wordmark">Sapphirus</span>
      </div>
      <form aria-label="Start a task" className="home-view__composer" onSubmit={submit}>
        <input
          aria-label="Describe your intent"
          autoComplete="off"
          className="home-view__input"
          disabled={composerDisabled}
          onChange={(event) => setIntent(event.target.value)}
          placeholder="Describe your intent — nothing is sent until you approve it…"
          ref={inputRef}
          spellCheck={false}
          type="text"
          value={intent}
        />
        <div className="home-view__composer-row">
          <span className="home-view__hint">{statusHint}</span>
          <Button
            aria-label="Start task"
            isDisabled={composerDisabled || trimmed.length === 0}
            size="small"
            type="submit"
            variant="primary"
          >
            <Send aria-hidden="true" size={14} />
            Start task
          </Button>
        </div>
      </form>
      {hasWorkspace ? (
        <WorkspaceSwitcher
          activeWorkspaceId={activeWorkspaceId}
          canOpenFolder={canOpenFolder}
          onActivate={onActivateWorkspace}
          onOpenFolder={onOpenWorkspace}
          onOpenManager={onOpenWorkspaceManager}
          statusLabel={workspaceStatusLabel}
          workspaces={workspaces}
        />
      ) : (
        <Button onPress={onOpenWorkspace} size="small" variant="secondary">
          Open workspace
        </Button>
      )}
    </main>
  );
}
