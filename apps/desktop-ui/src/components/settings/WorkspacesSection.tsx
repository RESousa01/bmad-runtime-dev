import { Button } from "@sapphirus/ui";
import { ChevronRight, LockKeyhole } from "lucide-react";

/**
 * The current folder grant, and the way out to the manager that changes it.
 *
 * The seam is ownership: this pane reports the active grant and hands off, so
 * it holds no list, no busy state and no error. Everything that can actually
 * change a grant lives in `WorkspacePanel`, which is a separate modal — putting
 * any of it here would mean two surfaces able to revoke the same authority.
 */
export interface WorkspacesSectionProps {
  readonly onManageWorkspaces: () => void;
  readonly workspaceDetail: string;
  readonly workspaceLabel: string;
}

export function WorkspacesSection({
  onManageWorkspaces,
  workspaceDetail,
  workspaceLabel,
}: WorkspacesSectionProps) {
  return (
    <section aria-labelledby="settings-workspace-title">
      <header className="settings-pane__header">
        <h3 id="settings-workspace-title">Workspaces</h3>
        <p>Manage the local folder grants used by this desktop.</p>
      </header>
      <div className="workspace-setting-summary">
        <span>
          <small>Current workspace</small>
          <strong>{workspaceLabel}</strong>
          <em>{workspaceDetail}</em>
        </span>
        <LockKeyhole aria-hidden="true" size={18} />
      </div>
      <p className="settings-pane__copy">
        Files stay local unless you add them to a message and confirm
        sending. Changes use the Review tab; rollback availability is
        verified by the desktop host.
      </p>
      <Button
        className="settings-pane__action"
        onPress={onManageWorkspaces}
        variant="secondary"
      >
        Manage workspaces <ChevronRight aria-hidden="true" size={15} />
      </Button>
    </section>
  );
}
