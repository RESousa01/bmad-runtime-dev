/**
 * The three things that can own the page as a modal.
 *
 * FE-10 shell composition: the workspace manager, Settings, and the delete
 * confirmation. Like the drawer, this reads the shell model rather than taking
 * props — the three between them need forty-four of App's locals, and a
 * forty-four-parameter interface is the same coupling with a longer signature.
 *
 * Only one can be open at a time, which is what makes a single `appModal`
 * discriminant correct: they are alternatives, not layers.
 */

import { DeleteTaskDialog } from "../components/DeleteTaskDialog";
import { SettingsDialog } from "../components/settings/SettingsDialog";
import { WorkspacePanel } from "../components/WorkspacePanel";
import { useShell } from "./AppProviders";

function WorkspaceManager() {
  const { host, workspace } = useShell();
  const manager = workspace.manager;
  return (
    <WorkspacePanel
      activeWorkspaceId={workspace.active?.workspaceId ?? null}
      busyWorkspaceId={manager.removalBusyId}
      canActivate={manager.canActivate}
      canRemove={manager.canRemove}
      canSelect={manager.canSelect}
      isSelecting={manager.selectionBusy}
      mode={host.runtimeKind}
      onActivate={manager.activate}
      onClose={manager.close}
      onRemove={manager.remove}
      onSelect={manager.select}
      workspaceError={manager.error}
      workspaces={manager.workspaces}
    />
  );
}

function Settings() {
  const { settings } = useShell();
  return (
    <SettingsDialog
      {...settings.props}
      /*
       * Keyed on the section as well as the modal. Settings holds per-section
       * state, and reopening it on a different section has to start that
       * section fresh rather than inherit the last one's.
       */
      key={`settings:${settings.section}`}
    />
  );
}

function DeleteConfirmation() {
  const { deletion } = useShell();
  if (deletion === null) return null;
  return (
    <DeleteTaskDialog
      historyStatus={deletion.historyStatus}
      mayHaveDurableCopy={deletion.mayHaveDurableCopy}
      messageCount={deletion.messageCount}
      onCancel={deletion.cancel}
      onConfirm={deletion.confirm}
      taskTitle={deletion.taskTitle}
    />
  );
}

/** Renders nothing when no modal is open, so the caller can pass it straight through. */
export function AppModals() {
  const { overlays } = useShell();
  switch (overlays.modalKind) {
    case "workspace-manager": return <WorkspaceManager />;
    case "settings": return <Settings />;
    case "delete-task": return <DeleteConfirmation />;
    // `"account"` and `null` both render nothing: the account surface has no
    // dialog yet, and this preserves that exactly rather than inventing one.
    default: return null;
  }
}
