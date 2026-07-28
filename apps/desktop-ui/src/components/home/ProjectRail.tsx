import type { WorkspaceProjection } from "../../lib/hostClient";
import { OpenCodeIcon } from "../OpenCodeIcon";

export interface ProjectRailProps {
  readonly activeWorkspaceId: string | null;
  readonly canOpenFolder: boolean;
  readonly onActivateWorkspace: (workspaceId: string) => void;
  readonly onOpenWorkspace: () => void;
  readonly onOpenWorkspaceManager: () => void;
  /// Re-runs host verification. Supplied only when the host is unavailable, so
  /// the control is never a button that cannot do anything.
  readonly onRetryHost?: (() => void) | undefined;
  readonly workspaces: readonly WorkspaceProjection[];
  readonly workspaceStatusLabel: string;
}

/**
 * Every workspace the host has granted, and which one is live.
 *
 * Split out of `HomeView` because it answers a different question from the task
 * index beside it -- "where can I work" rather than "what was I doing" -- and
 * because its empty case is deliberately not its own. A rail with no rows says
 * nothing here; the reason lives once, in the index body where the eye lands.
 * Two places explaining the same absence is two wordings that drift apart, and
 * the one nobody re-reads is the one that goes stale.
 *
 * The active project carries `aria-current` rather than a label of its own.
 * That is the attribute's purpose, a screen reader already announces it, and an
 * "Active" suffix on the visible name would put the same bit on screen twice
 * next to a row that weight and colour have already brought forward.
 */
export function ProjectRail({
  activeWorkspaceId,
  canOpenFolder,
  onActivateWorkspace,
  onOpenWorkspace,
  onOpenWorkspaceManager,
  onRetryHost,
  workspaces,
  workspaceStatusLabel,
}: ProjectRailProps) {
  return (
    <section className="home-view__projects">
      <header className="home-view__section-header">
        <h2>Projects</h2>
        <button
          aria-label="Open workspace"
          disabled={!canOpenFolder}
          onClick={onOpenWorkspace}
          type="button"
        >
          <OpenCodeIcon name="folder-add-left" size={14} />
        </button>
      </header>
      {workspaces.length === 0 ? null : (
        <ul aria-label="Projects" className="home-view__project-list">
          {workspaces.map((workspace) => (
            <li key={workspace.workspaceId}>
              <button
                aria-current={workspace.workspaceId === activeWorkspaceId ? "page" : undefined}
                onClick={() => onActivateWorkspace(workspace.workspaceId)}
                type="button"
              >
                <span>{workspace.displayName}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className="home-view__project-actions">
        <button onClick={onOpenWorkspaceManager} type="button">
          <OpenCodeIcon name="settings-gear" size={14} />
          Manage workspaces
        </button>
        {onRetryHost ? (
          <button onClick={onRetryHost} type="button">
            <OpenCodeIcon name="reset" size={14} />
            Retry host
          </button>
        ) : null}
        <span>{workspaceStatusLabel}</span>
      </div>
    </section>
  );
}
