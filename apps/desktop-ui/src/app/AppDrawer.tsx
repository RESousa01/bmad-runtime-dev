/**
 * The context drawer and its five panels.
 *
 * FE-10 shell composition. This is the map of what the side drawer can show:
 * files, review, model context, skills, activity. It reads the shell model
 * rather than taking props, because the five panels between them need
 * fifty-one of App's locals and a fifty-one-parameter interface is not a
 * boundary — it is the same coupling with a longer signature.
 *
 * There is no logic here beyond choosing a panel. Every decision the panels
 * act on was made in a controller and arrives on the model already resolved.
 */

import { ActivityPanel } from "../components/panels/ActivityPanel";
import { BmadCapabilityPanel } from "../components/BmadCapabilityPanel";
import { BmadLibraryPanel } from "../components/BmadLibraryPanel";
import { BmadWorkflowPanel } from "../components/BmadWorkflowPanel";
import { ContextDrawer } from "../components/redesign/ContextDrawer";
import { GovernedChangesPanel } from "../components/GovernedChangesPanel";
import { ReviewPanel } from "../components/GitReviewPanel";
import { SessionContextPanel } from "../components/SessionContextPanel";
import { WorkspaceExplorer } from "../components/WorkspaceExplorer";
import { useShell } from "./AppProviders";

function FilesPanel() {
  const { context, overlays, workspace } = useShell();
  return (
    <WorkspaceExplorer
      asPanel
      availabilityMessage={workspace.explorerAvailabilityMessage}
      /*
       * Keyed on the source *and* the grant. A workspace switch or a re-grant
       * gives a different tree with different permissions, and reusing the
       * component across that boundary would show one workspace's expansion
       * state over another's files.
       */
      key={[
        workspace.source?.provenance ?? "unavailable",
        workspace.active?.workspaceId ?? "none",
        workspace.active?.grantEpoch ?? 0,
      ].join(":")}
      onContextReview={(projection) => {
        context.adoptReviewed(projection);
        overlays.closeDrawer();
      }}
      source={workspace.source}
      workspaceName={workspace.name}
    />
  );
}

function ChangesReviewPanel() {
  const { changes, host, recovery } = useShell();
  return (
    <ReviewPanel
      availabilityMessage={changes.gitReviewAvailabilityMessage}
      governedReview={(
        <GovernedChangesPanel
          canEnableEdits={changes.canEnableEdits}
          enableEditsBusy={changes.enableEditsBusy}
          errorMessage={changes.error}
          history={changes.history}
          historyBusy={changes.historyBusy}
          onDecide={changes.decide}
          onDecideRecovery={recovery.decide}
          onEnableEdits={changes.enableEdits}
          onPrepareRecovery={recovery.prepare}
          onRefreshHistory={changes.refreshHistory}
          onStartNewProposal={changes.startNewProposal}
          onUndo={changes.undo}
          recoveryBusy={recovery.busy}
          recoveryReturnFocusTarget={recovery.returnFocusTarget}
          recoveryReview={recovery.review}
          state={changes.state}
        />
      )}
      isSampleData={host.runtimeKind === "browser_demo"}
      // A pending review or a prepared recovery are both things the user has
      // to answer, and the tab says so rather than waiting to be opened.
      proposalAttention={changes.state.kind === "review" || recovery.review !== null}
      source={changes.gitReviewSource}
    />
  );
}

function ModelContextPanel() {
  const { context, model, selectedTaskTitle } = useShell();
  return (
    <SessionContextPanel
      createdAt={context.session.createdAt}
      inputTokens={context.session.inputTokens}
      messages={context.session.messages}
      model={model.modelId}
      provider={model.providerLabel}
      sessionTitle={selectedTaskTitle ?? "No task selected"}
    />
  );
}

function SkillsPanel() {
  const { bmad, model } = useShell();
  return (
    <div className="method-library-panel">
      {bmad.run === null ? null : (
        <BmadCapabilityPanel
          capabilityId={bmad.run.capabilityId}
          capabilityLabel={bmad.run.label}
          destinationLabel={model.destinationLabel}
          onApprove={bmad.approveRun}
          onCancel={bmad.cancelRun}
          onClose={bmad.closeRun}
          onPrepare={bmad.prepareRun}
          onReviewChangeSet={() => bmad.proposeChangeSet(bmad.run!.capabilityId)}
          onSubmit={bmad.submitRun}
          phase={bmad.run.phase}
        />
      )}
      <BmadWorkflowPanel
        error={bmad.workflow.error}
        loading={bmad.workflow.loading}
        onRefresh={bmad.refreshWorkflow}
        state={bmad.workflow.state}
      />
      <BmadLibraryPanel
        // Method and Builder capabilities open the same reviewed run; the
        // panel's separate props keep the caller-side distinction, because
        // Builder output stays inert.
        onReload={bmad.reloadLibrary}
        onRunBuilderCapability={bmad.openRun}
        onRunCapability={bmad.openRun}
        onViewPersona={bmad.viewPersona}
        personaPerspectives={bmad.personaPerspectives}
        state={bmad.library}
      />
    </div>
  );
}

function RunActivityPanel() {
  const { bmad, changes, recovery } = useShell();
  return (
    <ActivityPanel
      helpState={bmad.helpState}
      history={changes.history}
      historyAvailable={changes.historyAvailable}
      historyBusy={changes.historyBusy}
      historyError={changes.error}
      historyStatus={changes.historyStatus}
      onDecideRecovery={recovery.decide}
      onPrepareRecovery={recovery.prepare}
      onRefreshHistory={changes.refreshHistory}
      onUndo={changes.undo}
      recoveryBusy={recovery.busy}
      recoveryReturnFocusTarget={recovery.returnFocusTarget}
      recoveryReview={recovery.review}
    />
  );
}

/** Renders nothing when the drawer is closed, so the caller can pass it straight through. */
export function AppDrawer() {
  const { overlays } = useShell();
  const kind = overlays.drawerKind;
  if (kind === null) return null;

  return (
    <ContextDrawer
      kind={kind}
      onClose={overlays.closeDrawer}
      onSelectTab={overlays.selectDrawerTab}
      presentation={overlays.drawerIsOverlay ? "overlay" : "pane"}
    >
      {kind === "files" ? <FilesPanel />
        : kind === "changes" ? <ChangesReviewPanel />
          : kind === "context" ? <ModelContextPanel />
            : kind === "skills" ? <SkillsPanel />
              : <RunActivityPanel />}
    </ContextDrawer>
  );
}
