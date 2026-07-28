import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { BmadPersonaPerspectiveView } from "./components/BmadLibraryPanel";
import type { GitReviewSource } from "./components/GitReviewPanel";
import type { SessionContextMessage } from "./components/SessionContextPanel";
import "./components/review-panel.css";
import {
  MAX_LANE_SESSIONS,
  useCapabilitySessions,
} from "./state/taskSessions";
import {
  appendDispatchExchange,
  applyRestoredHistory,
  applySentTurn,
  beginSend,
  failHistoryRestore,
  failSend,
  useChatSessions,
} from "./state/chatSessions";
import {
  closedTasks as closedTaskList,
  createTaskId,
  findTask,
  openTasks as openTaskList,
  type TaskRecord,
} from "./state/taskList";
import { useTaskLifecycle } from "./app/task/taskLifecycle";
import { createModelActions } from "./app/model/modelController";
import { useChangesHistoryState } from "./app/changes/changesHistoryState";
import { createChangesRunner } from "./app/changes/changesRunner";
import { isRecoveryFailure } from "./app/hostFailure";
import { isAvailable, resolveCapabilities } from "./app/capabilityMatrix";
import { createGuidanceRequest } from "./app/guidance/guidanceRequest";
import {
  createRecoveryActions,
  useRecoveryReview,
} from "./app/recovery/recoveryReview";
import {
  explorerAvailabilityMessage as describeExplorerAvailability,
  hostStatusLabel as describeHostStatus,
  methodStatusLabel as describeMethodStatus,
  modelAccess as describeModelAccess,
  providerLabel as describeProvider,
  workspaceDescription as describeWorkspace,
} from "./app/statusLabels";
import { useHostProjection } from "./app/projection/hostProjection";
import { eraseLocalData } from "./app/offboarding/eraseLocalData";
import { resolveMenuDispatch } from "./app/chat/menuDispatch";
import {
  adoptPreview,
  attachmentPaths,
  // Aliased: `removeAttachment` is the shell handler that wires this to the
  // active task, and two names for the same idea in one file is how the wrong
  // one gets called.
  removeAttachment as dropAttachment,
  clearAttachments,
  type TaskAttachments,
} from "./features/attachments/attachmentTypes";
import { useTaskSessions } from "./features/tasks/taskSessionController";
import { useDeferredFocusReturn } from "./app/focus/deferredFocusReturn";
import { startProjectionSubscription } from "./app/projection/projectionPoller";
import {
  mergeContextPaths,
  pickRejections,
} from "./app/context/contextSelection";
import type { ImportOutcome } from "./features/composer/useComposer";
import {
  anyPhase,
  approved,
  createCapabilityRunner,
  underReview,
} from "./app/capability/capabilityController";
import type {
  OffboardingStatus,
  SettingsSection,
} from "./components/settings/SettingsDialog";
import "./components/settings/settings.css";
import { CommandPalette, type PaletteAction } from "./components/CommandPalette";
import { createCommandRegistry, type CommandContext } from "./app/commands/commands";
import { commandForEvent, formatKeybinding, isApplePlatform } from "./app/commands/keybindings";
import { DRAWER_OVERLAY_QUERY } from "./components/redesign/AppShellLayout";
import { AppDrawer } from "./app/AppDrawer";
import { AppModals, hasRenderableModal } from "./app/AppModals";
import { AppMainView } from "./app/AppMainView";
import { ShellProvider, type ShellModel } from "./app/AppProviders";
import type { ContextDrawerKind } from "./components/redesign/ContextDrawer";
import "./components/panels/activity-panel.css";
import "./components/redesign/app-shell.css";
import "./components/redesign/context-drawer.css";
import "./components/redesign/no-workspace-state.css";
import "./components/redesign/task-surface.css";
import "./components/redesign/utility-panel.css";
import "./components/redesign/design-polish.css";
import "./components/redesign/design-refinement.css";
import {
  getDefaultHostRuntime,
  getSafeHostMessage,
  latestResultForRun,
  type AboutProjection,
  type BmadWorkflowStateProjection,
  type RetentionManifestProjection,
  type ApprovalChoice,
  type DensityPreference,
  type ThemePreference,
  HostCapabilityError,
  HostCommandError,
  type ChangesRecoveryPrepared,
  type HostRuntime,
  type WorkspaceProjection,
  type WorkspaceFilePick,
  type WorkspaceSearchMatch,
} from "./lib/hostClient";
import { useAppBootstrap, type BoundHostClient } from "./app/useAppBootstrap";
import type {
  AppClient,
  BmadCapabilityClient,
  ChatClient,
  WorkspaceClient,
} from "./lib/hostClient/clients";
/// The full client, needed only where a runtime is published.
import type { BootstrapReply, DesktopHostClient } from "./lib/hostClient";
import {
  createWorkspaceActions,
  useWorkspaceState,
} from "./app/workspace/workspaceController";
import { AppWorkbench } from "./app/AppWorkbench";
import type { GovernedChangesUiState } from "./components/GovernedChangesPanel";
import type {
  BmadLibrarySnapshot,
  BmadLibraryUiState,
} from "./lib/bmadProjection";
import {
  initialBmadRequestState,
  transitionBmadRequest,
  type BmadAuthoritySnapshot,
  type BmadRequestEvent,
  type BmadRequestState,
  type ModelAuthStatusProjection,
  type ModelSelection,
} from "./lib/bmadModelProjection";
import {
  browserDemoWorkspaceSource,
  createHostWorkspaceSource,
} from "./lib/workspaceReadSource";
import { useMediaQuery } from "./lib/useMediaQuery";
import {
  contextSearchTerms,
  rankContextPaths,
} from "./lib/contextSuggestions";

/// The id the per-task stores key on while no task is selected (Home).
///
/// The shell used to keep a phantom `new-0` "New task" tab that was never a
/// real record: it could not be renamed, closed, or deleted, and it is the
/// reason every task looked like it was called "New task". The task list is now
/// the only source of truth, so "no task" is a real state and this constant is
/// only an inert key for the per-task lane stores — it is never sent to the
/// host as a `client_session_ref`.
const NO_TASK_SESSION_ID = "task-none";

const browserDemoWorkspace: WorkspaceProjection = {
  workspaceId: "workspace_browser_demo",
  projectId: "project_browser_demo",
  displayName: "bmad-runtime-dev",
  grantEpoch: 0,
  permissions: "read_only",
  contextReadEpoch: 1,
  governedEditEpoch: 1,
};

/// Module scope, not an inline literal: the bootstrap hook takes it as a
/// dependency, and a fresh array every render would restart the loader effect
/// on every render.
const BROWSER_DEMO_WORKSPACES: readonly WorkspaceProjection[] = [browserDemoWorkspace];

const browserDemoGitReviewSource: GitReviewSource = {
  async readStatus() {
    return {
      branch: "codex/forge-v2",
      entries: [
        {
          relativePath: "src/components/TaskWorkspace.tsx",
          indexState: "modified",
          worktreeState: null,
        },
        {
          relativePath: "src/components/GitReviewPanel.tsx",
          indexState: "added",
          worktreeState: "modified",
        },
        {
          relativePath: "src/components/review-panel.css",
          indexState: null,
          worktreeState: "untracked",
        },
      ],
      truncated: false,
    };
  },
  async readDiff(relativePath, comparison) {
    const contents = relativePath.endsWith("TaskWorkspace.tsx")
      ? {
          beforeContent: [
            "import { Composer } from \"./Composer\";",
            "import { ContextSummary } from \"./ContextSummary\";",
            "",
            "export function TaskWorkspace() {",
            "  const files = [\"README.md\", \"src/App.tsx\", \"Cargo.toml\"];",
            "",
            "  return (",
            "    <main>",
            "      <ContextSummary files={files.length} />",
            "      <Composer placeholder=\"Describe a task\" />",
            "    </main>",
            "  );",
            "}",
            "",
          ].join("\n"),
          afterContent: [
            "import { Composer } from \"./Composer\";",
            "import { ContextInspector } from \"./ContextInspector\";",
            "import { ReviewTrigger } from \"./ReviewTrigger\";",
            "",
            "export function TaskWorkspace() {",
            "  const files = [\"README.md\", \"src/App.tsx\", \"Cargo.toml\"];",
            "  const tokenBudget = 128_000;",
            "",
            "  return (",
            "    <main>",
            "      <ContextInspector files={files} tokenBudget={tokenBudget} />",
            "      <Composer placeholder=\"Describe a task\" />",
            "      <ReviewTrigger label=\"Review exact context\" />",
            "    </main>",
            "  );",
            "}",
            "",
          ].join("\n"),
        }
      : relativePath.endsWith("review-panel.css")
        ? {
            beforeContent: null,
            afterContent: ".git-review {\n  display: flex;\n  flex-direction: column;\n}\n",
          }
        : {
            beforeContent: "<ChangesPanel />\n",
            afterContent: "<GitReviewPanel comparison=\"head_to_worktree\" />\n",
          };
    return {
      relativePath,
      comparison,
      ...contents,
      truncated: false,
    };
  },
};

const retainedHelpProjectionUnavailableMessage =
  "A retained BMAD Help session from an earlier version exists, but its authenticated projection is unavailable. You can create a new local skill-guidance session for this workspace grant.";

function recoveryManualReviewMessage(
  reasonCode: Extract<ChangesRecoveryPrepared, { status: "manual_review" }>["reasonCode"],
): string {
  switch (reasonCode) {
    case "checkpoint_incomplete_or_inconsistent":
      return "Recovery cannot continue automatically because the durable checkpoint is incomplete or inconsistent. Keep this journal quarantined for manual review.";
  }
}

export type PrimaryRoute =
  | { kind: "home" }
  | { kind: "task"; taskId: string | null };
export type AppModalKind =
  | "workspace-manager"
  | "settings"
  | "account"
  | "delete-task"
  | null;

export interface AppProps {
  hostRuntimeLoader?: () => Promise<HostRuntime>;
  projectionPollIntervalMs?: number;
}

function applyThemePreference(preference: ThemePreference) {
  const resolved = preference === "system"
    ? window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
    : preference;
  document.documentElement.dataset.theme = resolved;
  document.querySelector('meta[name="theme-color"]')?.setAttribute(
    "content",
    resolved === "dark" ? "#06121f" : "#f4f7fb",
  );
}
export function App({
  hostRuntimeLoader = getDefaultHostRuntime,
  projectionPollIntervalMs = 1_500,
}: AppProps = {}) {
  const [primaryRoute, setPrimaryRoute] = useState<PrimaryRoute>({ kind: "home" });
  const [density, setDensity] = useState<DensityPreference>("comfortable");
  const [tasks, setTasks] = useState<readonly TaskRecord[]>([]);
  /// The task a pending delete confirmation refers to. Held separately from
  /// the selection so the dialog can never act on a different task than the
  /// one it named.
  const [pendingDeleteTaskId, setPendingDeleteTaskId] = useState<string | null>(null);
  const [taskDrafts, setTaskDrafts] = useState<ReadonlyMap<string, string>>(new Map());
  const [contextDrawer, setContextDrawer] = useState<ContextDrawerKind | null>(null);
  const [theme, setTheme] = useState<ThemePreference>("dark");
  const [appModal, setAppModal] = useState<AppModalKind>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [utilitySettingsPage, setUtilitySettingsPage] = useState<SettingsSection>("general");
  /*
   * FE-10 step 3: two host-read projections.
   *
   * Both outlive the binding they were started against, and `useHostProjection`
   * captures the binding generation before it awaits so a result from a
   * replaced host is discarded. The retention manifest is the sharper case: it
   * lists what is still on disk, and a revoked host's answer tells the user
   * their data is somewhere it is not.
   */
  const about = useHostProjection<AboutProjection>();
  const offboarding = useHostProjection<RetentionManifestProjection, "erasing" | "erased">();
  // The `set` callbacks are stable (`useCallback` with no dependencies), so
  // naming them individually lets `invalidateHostBoundUi` list honest
  // dependencies -- the projection objects themselves are new every render.
  const { set: resetAboutProjection } = about;
  const { set: resetOffboardingProjection } = offboarding;
  const aboutProjection = about.projection;
  const aboutStatus = about.status;
  const offboardingManifest = offboarding.projection;
  const offboardingStatus: OffboardingStatus = offboarding.status;
  const [preferencesNotice, setPreferencesNotice] = useState<string | null>(null);
  const [offboardingNotice, setOffboardingNotice] = useState<string | null>(null);
  // Per-task capability execution state lives in `capabilitySessions` (keyed by
  // task session id) so a late completion routes to its owning task instead of
  // whichever tab is visible. Bound after `selectedSessionId` is derived.
  /*
   * FE-10 step 3: the workspace list, its selection and its busy flags belong
   * to the workspace controller. Destructured under the names the rest of this
   * component already uses, so the move is a change of owner rather than a
   * rename sprayed across two thousand lines.
   */
  const workspaceState = useWorkspaceState();
  const {
    actionError: workspaceActionError,
    activeWorkspace,
    activeWorkspaceId,
    adoptWorkspaces,
    promoteWorkspace,
    removalBusyId: workspaceRemovalBusyId,
    resetActionState: resetWorkspaceActionState,
    selectionBusy: workspaceSelectionBusy,
    setActionError: setWorkspaceActionError,
    workspaces: hostWorkspaces,
  } = workspaceState;
  const [bmadLibraryState, setBmadLibraryState] = useState<BmadLibraryUiState>({ kind: "idle" });
  const [personaPerspectives, setPersonaPerspectives] = useState<
    ReadonlyMap<string, BmadPersonaPerspectiveView>
  >(new Map());
  const [bmadHelpState, setBmadHelpState] = useState<BmadRequestState>(initialBmadRequestState);
  const [currentBmadIntent, setCurrentBmadIntent] = useState<{
    readonly createdAt?: number | undefined;
    readonly text: string;
  } | null>(null);
  const [modelAuthStatus, setModelAuthStatus] = useState<ModelAuthStatusProjection | null>(null);
  const [modelSelectionBusy, setModelSelectionBusy] = useState(false);
  const [modelSelectionNotice, setModelSelectionNotice] = useState<string | null>(null);
  // Per-task governed-changes flow: a slow proposal/decision must land on the
  // task it was started from, not whichever tab is visible when it resolves. The
  // shared `changesGenerationRef` still coordinates the whole changes subsystem
  // (history, undo, enable), so concurrent cross-task ops over-drop rather than
  // mis-display — strictly safer than a single global flow.
  const [changesFlowByTask, setChangesFlowByTask] =
    useState<ReadonlyMap<string, GovernedChangesUiState | null>>(new Map());
  const [changesError, setChangesError] = useState<string | null>(null);
  /*
   * FE-10 step 3: the Activity journal's read state.
   *
   * The projection and "has it been read" only make sense together --
   * `projection === null` meant both "never asked" and "asked, nothing there",
   * and Activity rendered the pair identically as "No activity yet". Every
   * transition lives in the hook so the two cannot disagree.
   */
  const changesHistoryState = useChangesHistoryState();
  const {
    busy: changesHistoryBusy,
    invalidate: invalidateChangesHistory,
    projection: changesHistory,
    setBusy: setChangesHistoryBusy,
    status: changesHistoryStatus,
  } = changesHistoryState;
  /*
   * FE-10 step 3: the journal-recovery review.
   *
   * Recovering a quarantined journal replays writes into the user's
   * repository, so the decision is consent and the controller binds it to what
   * was displayed.
   */
  const [enableEditsBusy, setEnableEditsBusy] = useState(false);
  const changesGenerationRef = useRef(0);
  const recovery = useRecoveryReview();
  const recoveryReview = recovery.review;
  const recoveryBusy = recovery.busy;
  const recoveryReturnFocusTarget = recovery.returnFocusTarget;
  const { clear: clearRecoveryReview, decisionInFlight: recoveryDecisionInFlight } = recovery;
  const drawerIsOverlay = useMediaQuery(DRAWER_OVERLAY_QUERY);
  /*
   * FE-10 step 3: where focus goes when an overlay closes.
   *
   * Three surfaces, one mechanism. Each used to carry its own captured ref, a
   * second "pending" ref armed on dismiss, and an effect to consume it -- and
   * every one of those parts is a way to leave focus on `<body>`, which sends
   * the next keystroke nowhere.
   */
  const utilityFocus = useDeferredFocusReturn(appModal !== null);
  const drawerFocus = useDeferredFocusReturn(contextDrawer !== null);
  const workspacePanelFocus = useDeferredFocusReturn(appModal !== null);
  /// Whether the drawer should take focus itself on open, rather than leaving
  /// it where it was. True only when the drawer replaces a surface that had
  /// focus -- otherwise opening a side panel would steal it from the composer.
  const focusContextDrawerOnOpenRef = useRef(false);
  const bmadProjectionGenerationRef = useRef(0);

  /*
   * The invalidator reaches the hook through a ref because it is defined below,
   * after the projection clearers it composes, while the hook has to sit above
   * everything derived from `hostRuntime` -- which is most of this component.
   * Safe because the port is only ever called from an event handler or from a
   * resolved promise, never during render, so `.current` is always the
   * committed version by the time it runs.
   */
  const hostBoundUiInvalidatorRef = useRef<() => void>(() => undefined);
  const invalidateHostBoundUiPort = useCallback(() => {
    hostBoundUiInvalidatorRef.current();
  }, []);
  /*
   * FE-10: the host runtime state machine lives in `useAppBootstrap`.
   *
   * The three transitions -- adopt, degrade to recovery, drop to unavailable --
   * share one invariant: each bumps the binding generation before publishing,
   * so every async read still in flight against the previous binding lands
   * stale and is discarded. Keeping them in one place is what makes that
   * invariant checkable; interleaved with forty other pieces of state it was
   * not.
   */
  const {
    adoptHostRuntime,
    hostBindingGenerationRef,
    hostRuntime,
    markHostUnavailable,
    markReadOnlyRecovery,
    republishHostRuntime,
    retryHostRuntime,
  } = useAppBootstrap({
    adoptWorkspaces,
    browserDemoWorkspaces: BROWSER_DEMO_WORKSPACES,
    invalidateHostBoundUi: invalidateHostBoundUiPort,
    loadRuntime: hostRuntimeLoader,
    reportRecoveryNotice: setWorkspaceActionError,
  });

  const bmadLibraryRequestedRef = useRef(false);
  const bmadHelpGenerationRef = useRef(0);
  const bmadHelpCreationRef = useRef<Promise<void> | null>(null);
  const bmadHelpOperationRef = useRef<"approve" | "cancel" | "submit" | null>(null);
  const bmadHelpStateRef = useRef<BmadRequestState>(initialBmadRequestState);
  const workspaceAuthorityKeyRef = useRef("");

  const routedTaskId = primaryRoute.kind === "task" ? primaryRoute.taskId : null;
  const selectedSessionId = routedTaskId ?? NO_TASK_SESSION_ID;
  const capabilitySessions = useCapabilitySessions(selectedSessionId);
  /*
   * FE-13: attachments belong to a task.
   *
   * They used to be four app-global values, so switching tasks destroyed them
   * -- `selectSession` cleared them on every switch precisely because the
   * alternative was sending one task's files with another task's message. A
   * task now keeps what it has while the user works somewhere else.
   */
  const taskSessions = useTaskSessions(selectedSessionId);
  const attachments = taskSessions.session.attachments;
  /// Summed from the descriptors, so it cannot disagree with the list.
  const attachedTokens = attachments.descriptors.length === 0
    ? undefined
    : attachments.descriptors.reduce((total, item) => total + item.estimatedTokens, 0);
  /// Named separately so the invalidators can list a stable dependency; the
  /// api object itself is rebuilt every render.
  const clearAllTaskSessions = taskSessions.clearAll;
  /** Writes fields of the *active* task's attachments. */
  const patchAttachments = useCallback(
    (patch: Partial<TaskAttachments>) => {
      taskSessions.updateActive((session) => ({
        ...session,
        attachments: { ...session.attachments, ...patch },
      }));
    },
    [taskSessions],
  );
  // Chat is the default lane: every task owns a conversation, and BMAD is an
  // optional overlay on it rather than the only way to say something.
  const chatSessions = useChatSessions(selectedSessionId);
  const chatLane = chatSessions.lane;
  // All four take the task they act on as an argument, so — unlike
  // `capabilitySessions`' active-task members below — they are safe to capture
  // in an effect or a memoized callback without pinning a stale session.
  const {
    applyToLane: applyToChatLane,
    beginHistoryRestore: beginChatHistoryRestore,
    clearAll: clearAllChatLanes,
    retryHistoryRestore: retryChatHistoryRestore,
  } = chatSessions;
  // Deterministic method progress: computed by the host, never a model claim.
  const [workflowState, setWorkflowState] = useState<{
    state: BmadWorkflowStateProjection | null;
    loading: boolean;
    error: string | null;
  }>({ state: null, loading: false, error: null });
  const capabilityRun = capabilitySessions.capabilityRun;
  // Destructured because it is session-independent and referentially stable, so
  // it is safe to capture inside a `useCallback`. The other members are bound to
  // the *active* task, so a stale capture of them would write to the wrong task:
  // reach for them only from render-scoped handlers, never from a memoized
  // callback that omits `capabilitySessions` from its dependencies.
  const { clearAllCapabilityRuns } = capabilitySessions;
  const changesFlow = changesFlowByTask.get(selectedSessionId) ?? null;
  const setChangesFlowForTask = useCallback(
    (taskId: string, value: GovernedChangesUiState | null) => {
      setChangesFlowByTask((current) => {
        const next = new Map(current);
        // `changesState` derives "idle" from both a missing entry and an explicit
        // `{kind:"idle"}`, so drop idle/null to keep the map bounded to active or
        // terminal flows rather than accumulating an entry per task ever touched.
        if (value === null || value.kind === "idle") {
          next.delete(taskId);
        } else {
          next.set(taskId, value);
          // Terminal flows (applied/discarded/undo_unavailable) persist, so bound
          // the map the same way the capability store is bounded: evict the
          // oldest entries (Map keeps insertion order), never the one just set.
          for (const key of next.keys()) {
            if (next.size <= MAX_LANE_SESSIONS) break;
            if (key !== taskId) next.delete(key);
          }
        }
        return next;
      });
    },
    [],
  );
  const clearAllChangesFlows = useCallback(() => setChangesFlowByTask(new Map()), []);
  const selectedTask = useMemo(
    () => findTask(tasks, routedTaskId),
    [routedTaskId, tasks],
  );
  // Tabs read left to right in creation order; Home lists the newest first.
  const openTaskTabs = useMemo(() => openTaskList(tasks), [tasks]);
  const recentTaskList = useMemo(() => openTaskList(tasks).toReversed(), [tasks]);
  const closedTasks = useMemo(() => closedTaskList(tasks), [tasks]);
  const pendingDeleteTask = useMemo(
    () => findTask(tasks, pendingDeleteTaskId),
    [pendingDeleteTaskId, tasks],
  );
  const workspaceName = activeWorkspace?.displayName ?? "No local workspace selected";
  const workspaceDescription = describeWorkspace({
    permissions: activeWorkspace?.permissions ?? null,
    runtimeKind: hostRuntime.kind,
  });
  const hostStatusLabel = describeHostStatus(hostRuntime.kind);
  const canSelectWorkspace = hostRuntime.kind === "ready"
    && hostRuntime.bootstrap.supportedCommands.includes("workspace.select_folder");
  const canActivateWorkspace = hostRuntime.kind === "ready"
    && hostRuntime.bootstrap.supportedCommands.includes("workspace.list");
  const canRemoveWorkspace = hostRuntime.kind === "ready"
    && hostRuntime.bootstrap.supportedCommands.includes("workspace.revoke");
  const methodLibraryClient = hostRuntime.kind === "ready"
    && hostRuntime.bootstrap.supportedCommands.includes("bmad.library.snapshot")
    ? hostRuntime.client
    : null;
  const methodLibraryAvailable = methodLibraryClient !== null;
  const viewPersonaPerspective = useCallback(
    async (agentCode: string) => {
      if (
        hostRuntime.kind !== "ready" ||
        !hostRuntime.bootstrap.supportedCommands.includes("bmad.persona.view")
      ) {
        return;
      }
      try {
        const perspective = await hostRuntime.client.viewBmadPersona(agentCode);
        setPersonaPerspectives((current) => {
          const next = new Map(current);
          next.set(agentCode, {
            agentCode: perspective.agentCode,
            instructionMarkdown: perspective.instructionMarkdown,
            instructionHash: perspective.instructionHash,
          });
          return next;
        });
      } catch {
        // The perspective stays hidden; the library remains usable.
      }
    },
    [hostRuntime],
  );
  const skillsAgentsStatusLabel = !methodLibraryAvailable
    ? "Unavailable"
    : bmadLibraryState.kind === "ready"
      ? "Loaded"
      : bmadLibraryState.kind === "loading"
        ? "Loading"
        : bmadLibraryState.kind === "unavailable"
          ? "Unavailable"
          : "Not loaded";
  const methodGuidanceClient = hostRuntime.kind === "ready"
    && activeWorkspace !== null
    && activeWorkspace.grantEpoch >= 1
    && ([
      "bmad.library.snapshot",
      "model.auth.status",
      "model.auth.sign_in",
      "model.auth.sign_out",
      "model.selection.set",
      "bmad.help.prepare",
      "bmad.help.approve",
      "bmad.help.cancel",
      "bmad.help.submit",
      "bmad.help.latest",
      "run.create",
    ] as const).every(
      (command) => hostRuntime.bootstrap.supportedCommands.includes(command),
    )
    ? hostRuntime.client
    : null;
  const methodGuidanceAvailable = methodGuidanceClient !== null;
  /// Gated on `chat.history` alone, not on the whole guidance command set. A
  /// host that can hand back a transcript should hand it back even if it has
  /// no BMAD advisor: reading what the user already said is not a Method
  /// feature. The grant epoch matches the send gate because the host
  /// authorizes the read against the same workspace scope.
  const chatHistoryClient = hostRuntime.kind === "ready"
    && activeWorkspace !== null
    && activeWorkspace.grantEpoch >= 1
    && hostRuntime.bootstrap.supportedCommands.includes("chat.history")
    ? hostRuntime.client
    : null;
  /*
   * FE-12: one capability check per feature.
   *
   * `chatSendAvailable` used to be `methodGuidanceClient !== null && model
   * ready`, and `methodGuidanceClient` required all eleven BMAD commands. So a
   * host advertising `chat.send`, a granted workspace and a connected model was
   * still reported unable to send, because it did not also advertise
   * `bmad.help.approve` -- and the composer said the model was not connected,
   * which was not true.
   *
   * The two closures are now disjoint by construction, and a test keeps them
   * that way: the defect returns the moment someone adds "just one" BMAD
   * command to the chat gate.
   */
  const capabilities = resolveCapabilities({
    browserPreview: hostRuntime.kind === "browser_demo",
    hostReady: hostRuntime.kind === "ready",
    libraryLoaded: methodLibraryAvailable,
    modelReady: modelAuthStatus?.status === "development_ready",
    supportedCommands: hostRuntime.kind === "ready"
      ? hostRuntime.bootstrap.supportedCommands
      : [],
    workspaceGrantEpoch: activeWorkspace?.grantEpoch ?? null,
  });
  const chatSendAvailable = isAvailable(capabilities.chat);
  const methodGuidanceAvailableForRequests = isAvailable(capabilities.help);
  /*
   * FE-12: the send transport, gated on chat's own closure.
   *
   * Fixing the *gate* without fixing this would be worse than the defect it
   * replaces: the composer would enable and the send would fail at the host,
   * turning a clear "not available" into a message that silently does not
   * arrive.
   */
  const chatSendClient: ChatClient | null = hostRuntime.kind === "ready"
    && isAvailable(capabilities.chat)
    ? hostRuntime.client
    : null;
  const methodRequestInFlight = bmadHelpState.kind === "creating"
    || bmadHelpState.kind === "review_required"
    || bmadHelpState.kind === "approving"
    || bmadHelpState.kind === "approved"
    || bmadHelpState.kind === "submitting";
  const localTaskDraftingAvailable = hostRuntime.kind === "ready"
    || hostRuntime.kind === "browser_demo";
  const modelAccess = describeModelAccess({
    libraryLoaded: methodLibraryAvailable,
    modelAuth: modelAuthStatus,
    modelCommandsAvailable: methodGuidanceClient !== null,
    runtimeKind: hostRuntime.kind,
  });
  const modelProviderLabel = describeProvider(modelAuthStatus?.providerId);
  // Browser preview used to answer "forge-bmad-preview" here, so the session
  // panel named a model for a session that had never contacted one. There is no
  // model in browser preview; saying so is `undefined`, which renders as "—".
  const sessionContextModelId = modelAuthStatus?.modelId;
  const modelSelection: ModelSelection = {
    providerId: modelAuthStatus?.providerId === "azure" ? "azure" : "openai",
    modelId: modelAuthStatus?.modelId ?? "gpt-5-mini",
  };
  const methodStatusLabel = describeMethodStatus({
    chatAvailable: chatSendAvailable,
    guidanceCommandsAvailable: methodGuidanceAvailable,
    runtimeKind: hostRuntime.kind,
  });
  const workspaceAuthorityKey = hostRuntime.kind === "ready"
    || hostRuntime.kind === "read_only_recovery"
    ? `${hostRuntime.bootstrap.rendererSessionId}:${activeWorkspace?.workspaceId ?? "none"}:${activeWorkspace?.grantEpoch ?? 0}`
    : hostRuntime.kind;
  const methodGuidanceBindingKey = workspaceAuthorityKey;
  workspaceAuthorityKeyRef.current = workspaceAuthorityKey;
  const workspaceSource = useMemo(() => {
    if (hostRuntime.kind === "browser_demo") {
      return browserDemoWorkspaceSource;
    }
    if (hostRuntime.kind !== "ready" || !activeWorkspace) {
      return null;
    }
    const requiredCommands = [
      "workspace.list_entries",
      "workspace.read_text",
      "workspace.search",
      "bmad.scan",
      "context.preview",
    ] as const;
    if (!requiredCommands.every((command) => hostRuntime.bootstrap.supportedCommands.includes(command))) {
      return null;
    }
    return createHostWorkspaceSource(hostRuntime.client, activeWorkspace.workspaceId);
  }, [activeWorkspace, hostRuntime]);
  const gitReviewSource = useMemo<GitReviewSource | null>(() => {
    if (hostRuntime.kind === "browser_demo") {
      return browserDemoGitReviewSource;
    }
    if (hostRuntime.kind !== "ready" || activeWorkspace === null) {
      return null;
    }
    if (!([
      "workspace.git_status",
      "workspace.git_diff",
    ] as const).every((command) => hostRuntime.bootstrap.supportedCommands.includes(command))) {
      return null;
    }
    const client = hostRuntime.client;
    const workspaceId = activeWorkspace.workspaceId;
    return {
      readStatus: () => client.readGitStatus(workspaceId),
      readDiff: (relativePath, comparison) =>
        client.readGitDiff(workspaceId, relativePath, comparison),
    };
  }, [activeWorkspace, hostRuntime]);
  const gitReviewAvailabilityMessage = hostRuntime.kind === "read_only_recovery"
    ? "Git review is paused while Forge is in read-only recovery."
    : hostRuntime.kind === "unavailable"
      ? "Git review requires the Forge desktop host."
      : activeWorkspace === null
        ? "Choose a workspace to review its Git changes."
        : "Git review is unavailable in this host version.";
  const canBrowseFiles = hostRuntime.kind === "ready"
    && activeWorkspace !== null
    && workspaceSource !== null
    && hostRuntime.bootstrap.supportedCommands.includes("workspace.pick_files");
  const canBrowseFolder = hostRuntime.kind === "ready"
    && activeWorkspace !== null
    && workspaceSource !== null
    && hostRuntime.bootstrap.supportedCommands.includes("workspace.pick_folder");

  /**
   * Picks files and reports what came of it.
   *
   * FE-14: the return value is what lets the composer tell a cancel from an
   * import that attached nothing. `undefined` means "nothing to say" -- the
   * user cancelled, or the result arrived for a task that has moved on -- and
   * an outcome means the pick resolved, rejections and all.
   */
  async function browseAndAttach(
    picker: (client: WorkspaceClient, workspaceId: string) => Promise<WorkspaceFilePick>,
  ): Promise<ImportOutcome | undefined> {
    if (hostRuntime.kind !== "ready" || !activeWorkspace || !workspaceSource) {
      return undefined;
    }
    const client = hostRuntime.client;
    const workspaceId = activeWorkspace.workspaceId;
    const existingPaths = attachmentPaths(attachments);
    /*
     * Two tokens, because two different things can invalidate this.
     *
     * `routing` is the task's: it goes stale if the task is closed or a newer
     * selection supersedes this one, and it is what makes the result land on
     * the task that asked rather than the visible one.
     *
     * `authorityKey` is the workspace's: a grant change invalidates the paths
     * themselves, and its reset runs in an effect, so between the switch and
     * that effect the task generation has not moved yet.
     */
    const routing = taskSessions.beginRequest();
    const authorityKey = workspaceAuthorityKey;
    /*
     * Both halves, and the same predicate on every path including the failure
     * one. The workspace-authority invalidation runs in an effect, so between
     * the switch and that effect the generation has not moved yet -- a catch
     * that checked only the generation would surface the old workspace's error
     * against the new one.
     */
    const isStale = () => !taskSessions.isCurrent(routing)
      || authorityKey !== workspaceAuthorityKeyRef.current;
    patchAttachments({ notice: null });
    try {
      const pick = await picker(client, workspaceId);
      // A cancelled dialog is not an outcome. It is the user deciding not to
      // attach anything, and it says nothing.
      if (pick.kind === "no_selection") return undefined;
      if (pick.value.relativePaths.length === 0) {
        if (isStale()) return undefined;
        // Chose files, got none. From the user's side that is identical to
        // cancelling, so the refusals are the only way they learn otherwise.
        return { attached: 0, rejected: pickRejections(pick.value) };
      }
      const merge = mergeContextPaths(existingPaths, pick.value.relativePaths);
      const projection = await workspaceSource.previewContext([...merge.relativePaths]);
      if (isStale()) return undefined;
      taskSessions.route(routing, (session) => ({
        ...session,
        attachments: adoptPreview(projection, "manual", null),
      }));
      return { attached: merge.relativePaths.length, rejected: pickRejections(pick.value, merge) };
    } catch (error) {
      // A stale failure belongs to a task the user has left. Reporting it
      // against the visible one is the same mistake `isStale` exists to stop.
      if (isStale()) return undefined;
      // Rethrown rather than written to the notice, so the composer machine is
      // the single channel for what a pick did. Two channels is how a failed
      // import ends up reported twice, or not at all.
      throw error;
    }
  }

  function browseAndAttachFiles(): Promise<ImportOutcome | undefined> {
    return browseAndAttach((client, workspaceId) => client.pickWorkspaceFiles(workspaceId));
  }

  function browseAndAttachFolder(): Promise<ImportOutcome | undefined> {
    return browseAndAttach((client, workspaceId) => client.pickWorkspaceFolder(workspaceId));
  }

  /**
   * Attaches one previously refused file again.
   *
   * Resolves to whether it worked, which the host answers by simply not
   * returning an item for a file it still cannot read. That is the honest test:
   * asking for the path back and seeing whether it comes.
   *
   * Routed like every other attachment request, because a retry pressed just
   * before switching tasks would otherwise land its result on whichever task
   * is now on screen.
   */
  async function retryAttachment(relativePath: string): Promise<boolean> {
    if (workspaceSource === null) return false;
    const routing = taskSessions.beginRequest();
    const authorityKey = workspaceAuthorityKey;
    const isStale = () => !taskSessions.isCurrent(routing)
      || authorityKey !== workspaceAuthorityKeyRef.current;
    const existing = attachmentPaths(taskSessions.sessionFor(routing.taskId).attachments);
    const merge = mergeContextPaths(existing, [relativePath]);
    const projection = await workspaceSource.previewContext([...merge.relativePaths]);
    // A stale result is not a failure, but it is not a success either: saying
    // it attached would leave the row cleared on a task that never got it.
    if (isStale()) return false;
    const attached = projection.items
      .some((item) => item.relativePath === relativePath);
    taskSessions.route(routing, (session) => ({
      ...session,
      attachments: adoptPreview(projection, session.attachments.selectionMode, null),
    }));
    return attached;
  }

  /** Drops one attachment from the task that owns it. */
  function removeAttachment(attachmentId: string) {
    taskSessions.updateActive((session) => ({
      ...session,
      attachments: dropAttachment(session.attachments, attachmentId),
    }));
  }

  async function suggestContext(intent: string) {
    const terms = contextSearchTerms(intent);
    if (workspaceSource === null || terms.length === 0 || attachments.suggesting) {
      if (terms.length === 0) {
        patchAttachments({
          notice: "Add a more specific task description before auto-selecting context.",
        });
      }
      return;
    }
    const source = workspaceSource;
    const authorityKey = workspaceAuthorityKey;
    /// Same two tokens as `browseAndAttach`, for the same two reasons.
    const routing = taskSessions.beginRequest();
    const isStale = () => !taskSessions.isCurrent(routing)
      || authorityKey !== workspaceAuthorityKeyRef.current;
    // `suggesting` is the per-task lock: a second auto-select for *this* task
    // is refused, while another task may run its own.
    patchAttachments({ notice: null, suggesting: true });
    try {
      const resultGroups: WorkspaceSearchMatch[][] = [];
      for (const term of terms) {
        resultGroups.push(await source.search(term, 20));
        if (isStale()) return;
      }
      const relativePaths = rankContextPaths(resultGroups, 8);
      if (relativePaths.length === 0) {
        if (isStale()) return;
        patchAttachments({
          notice: "Forge found no local files matching this task. Pin files from Files instead.",
        });
        return;
      }
      const projection = await source.previewContext(relativePaths);
      if (isStale()) return;
      const count = projection.items.length;
      taskSessions.route(routing, (session) => ({
        ...session,
        attachments: adoptPreview(
          projection,
          "automatic",
          `Auto-selected ${count} local ${count === 1 ? "file" : "files"} for review.`,
        ),
      }));
    } catch (error) {
      if (!isStale()) patchAttachments({ notice: getSafeHostMessage(error) });
    } finally {
      /*
       * Cleared on the task that started it, unconditionally, even when the
       * result was discarded. `route` would skip a superseded token and leave
       * that task's spinner running for the rest of the session.
       */
      taskSessions.update(routing.taskId, (session) => ({
        ...session,
        attachments: { ...session.attachments, suggesting: false },
      }));
    }
  }

  /**
   * Drops the active task's attachments.
   *
   * No longer called on a task switch -- that was only ever necessary because
   * the projection was app-global. It is now what it says: the user clearing
   * their own selection, or a send spending it.
   */
  function clearContext(notice: string | null = null) {
    taskSessions.supersede();
    taskSessions.updateActive((session) => ({
      ...session,
      attachments: clearAttachments(notice),
    }));
  }
  const explorerAvailabilityMessage = describeExplorerAvailability({
    hasWorkspace: activeWorkspace !== null,
    runtimeKind: hostRuntime.kind,
    unavailableMessage: hostRuntime.kind === "unavailable" ? hostRuntime.message : null,
  });

  const applyBmadEvent = useCallback((event: BmadRequestEvent): BmadRequestState => {
    const next = transitionBmadRequest(bmadHelpStateRef.current, event);
    bmadHelpStateRef.current = next;
    setBmadHelpState(next);
    return next;
  }, []);

  const currentBmadAuthority = useCallback((runId: string): BmadAuthoritySnapshot | null => {
    if (!activeWorkspace || !modelAuthStatus) return null;
    return {
      workspaceId: activeWorkspace.workspaceId,
      workspaceGrantEpoch: activeWorkspace.grantEpoch,
      runId,
      authEpoch: modelAuthStatus.authEpoch,
      rendererGeneration: bmadHelpGenerationRef.current,
      now: Date.now(),
    };
  }, [activeWorkspace, modelAuthStatus]);

  /*
   * FE-11: narrow ports rather than the whole facade.
   *
   * This reads the library and, when the renderer session has expired,
   * re-bootstraps -- so `BmadCapabilityClient & AppClient` is exactly its
   * authority. Publishing the rebound runtime needs the full client, and that
   * is a *different* job, so it arrives as a callback the caller supplies
   * rather than as a wider parameter here. Widening the parameter to make one
   * line compile is how a controller ends up holding the facade again.
   */
  const loadMethodLibrary = useCallback(async (
    client: BmadCapabilityClient & AppClient,
    rebind: {
      readonly republish: (bootstrap: BootstrapReply) => void;
      readonly abandon: () => void;
    },
  ) => {
    bmadLibraryRequestedRef.current = true;
    const generation = ++bmadProjectionGenerationRef.current;
    setBmadLibraryState({ kind: "loading" });
    let rendererSessionRebindAttempted = false;
    let rendererSessionRebound = false;
    try {
      let projection: BmadLibrarySnapshot;
      try {
        projection = await client.bmadLibrarySnapshot(null);
      } catch (error) {
        if (!(error instanceof HostCommandError && error.details.code === "renderer_session_expired")) {
          throw error;
        }
        rendererSessionRebindAttempted = true;
        const hostBindingGeneration = ++hostBindingGenerationRef.current;
        const bootstrap = await client.bootstrap();
        rendererSessionRebound = true;
        if (
          generation !== bmadProjectionGenerationRef.current
          || hostBindingGeneration !== hostBindingGenerationRef.current
        ) {
          return;
        }
        adoptWorkspaces(bootstrap.workspaces, true);
        // `republishHostRuntime`, not `adoptHostRuntime`: the new binding
        // generation was claimed above, before the await, and adopting here
        // would claim a second one -- invalidating the very read that is
        // currently succeeding.
        rebind.republish(bootstrap);
        if (bootstrap.bootMode !== "ready") return;
        if (!bootstrap.supportedCommands.includes("bmad.library.snapshot")) {
          return;
        }
        projection = await client.bmadLibrarySnapshot(null);
      }
      if (generation === bmadProjectionGenerationRef.current) {
        setBmadLibraryState({ kind: "ready", projection });
      }
    } catch (error) {
      if (generation !== bmadProjectionGenerationRef.current) {
        return;
      }
      if (
        rendererSessionRebindAttempted
        && (
          !rendererSessionRebound
          || (error instanceof HostCommandError && error.details.code === "renderer_session_expired")
        )
      ) {
        bmadLibraryRequestedRef.current = false;
        bmadProjectionGenerationRef.current += 1;
        setBmadLibraryState({ kind: "idle" });
        clearAllTaskSessions();
        // `abandon` bumps the binding generation itself, so the explicit bump
        // that used to sit at the top of this branch is gone -- two bumps for
        // one transition would have been harmless but is one more place the
        // epoch could drift.
        rebind.abandon();
        return;
      }
      setBmadLibraryState({
        kind: "unavailable",
        message: getSafeHostMessage(error),
        retryable: error instanceof HostCommandError && error.details.retryable,
      });
    }
  }, [
    // Both stable for the life of the component -- a ref object and a
    // `useCallback` with no dependencies -- so naming them keeps this callback
    // stable while satisfying the exhaustive-deps rule honestly rather than by
    // suppression.
    adoptWorkspaces,
    clearAllTaskSessions,
    hostBindingGenerationRef,
  ]);

  /**
   * How `loadMethodLibrary` re-publishes a runtime it had to re-bootstrap.
   *
   * Built here because publishing needs the full client, which the library
   * reader deliberately does not hold. `republishHostRuntime` rather than
   * `adoptHostRuntime`: the new binding generation was already claimed before
   * the await, and adopting would claim a second one -- invalidating the very
   * read that is currently succeeding.
   */
  const methodLibraryRebind = useCallback((client: DesktopHostClient) => ({
    abandon: () => markHostUnavailable(client),
    republish: (bootstrap: BootstrapReply) => republishHostRuntime({
      kind: bootstrap.bootMode === "ready" ? "ready" : "read_only_recovery",
      client,
      bootstrap,
    }),
  // `useCallback`, not a declaration: it is captured by the projection poller,
  // and a new function every render would restart the poller every render.
  }), [markHostUnavailable, republishHostRuntime]);

  const clearMethodLibraryProjection = useCallback(() => {
    bmadLibraryRequestedRef.current = false;
    bmadProjectionGenerationRef.current += 1;
    setBmadLibraryState({ kind: "idle" });
  }, []);

  const clearBmadHelpProjection = useCallback(() => {
    bmadHelpGenerationRef.current += 1;
    bmadHelpCreationRef.current = null;
    bmadHelpOperationRef.current = null;
    setModelAuthStatus(null);
    const current = bmadHelpStateRef.current;
    const next = current.kind === "review_required"
      || current.kind === "approving"
      || current.kind === "approved"
      || current.kind === "submitting"
      ? transitionBmadRequest(current, {
        type: "authority_invalidated",
        reason: "authority_changed",
      })
      : initialBmadRequestState;
    bmadHelpStateRef.current = next;
    setBmadHelpState(next);
    setCurrentBmadIntent(null);
  }, []);

  const invalidateWorkspaceBoundUi = useCallback(() => {
    // Transcripts and the persona bound to them are workspace-scoped: the
    // agent roster differs per workspace, so a lane carried across a switch
    // would keep showing the old workspace's messages and would send its
    // agentCode to the new one while the selector displayed "Plain chat".
    clearAllChatLanes();
    setWorkflowState({ state: null, loading: false, error: null });
    changesGenerationRef.current += 1;
    clearAllChangesFlows();
    setChangesError(null);
    invalidateChangesHistory();
    setChangesHistoryBusy(false);
    setEnableEditsBusy(false);
    clearRecoveryReview();
    // Every task's attachments, because they resolve against a workspace grant
    // this invalidation is discarding.
    clearAllTaskSessions();
    setContextDrawer(null);
  }, [
    clearAllChangesFlows,
    clearAllChatLanes,
    clearAllTaskSessions,
    clearRecoveryReview,
    invalidateChangesHistory,
    setChangesHistoryBusy,
  ]);

  const invalidateHostBoundUi = useCallback(() => {
    invalidateWorkspaceBoundUi();
    clearMethodLibraryProjection();
    clearBmadHelpProjection();
    setPersonaPerspectives(new Map());
    clearAllCapabilityRuns();
    resetAboutProjection(null, "unavailable");
    resetOffboardingProjection(null, "unavailable");
    setOffboardingNotice(null);
    setModelSelectionBusy(false);
    setModelSelectionNotice(null);
    setPreferencesNotice(null);
    resetWorkspaceActionState();
  }, [
    clearAllCapabilityRuns,
    clearBmadHelpProjection,
    clearMethodLibraryProjection,
    invalidateWorkspaceBoundUi,
    resetAboutProjection,
    resetOffboardingProjection,
    resetWorkspaceActionState,
  ]);

  // Closes the loop opened by `invalidateHostBoundUiPort` above. Assigned
  // during render, matching how this component already keeps
  // `workspaceAuthorityKeyRef` and `bmadHelpStateRef` current.
  hostBoundUiInvalidatorRef.current = invalidateHostBoundUi;

  function dismissWorkspacePanel() {
    workspacePanelFocus.arm();
    setAppModal(null);
  }

  function openUtilityPanel(
    mode: "account" | "settings",
    settingsPage: SettingsSection = "general",
    returnFocusTarget: HTMLElement | null = null,
  ) {
    utilityFocus.capture(returnFocusTarget);
    setContextDrawer(null);
    setUtilitySettingsPage(mode === "settings" ? settingsPage : "general");
    setAppModal(mode);
    void loadAboutProjection();
  }

  /// Reads the About projection for the host binding that was current when the
  /// call started, and drops the answer if that binding has since been replaced.
  ///
  /// Every other host read in this file is generation-guarded; this one was not.
  /// `invalidateHostBoundUi` clears `aboutProjection` and sets the status back
  /// to `unavailable` precisely so a revoked or recovered host's identity is not
  /// left on screen — but an `app.about` call already in flight resolved
  /// afterwards and wrote the *old* host's version, build and licence back into
  /// the dialog, with the status reading `ready`.
  async function loadAboutProjection() {
    if (
      hostRuntime.kind !== "ready"
      || !hostRuntime.bootstrap.supportedCommands.includes("app.about")
    ) {
      about.setStatus("unavailable");
      return;
    }
    const client = hostRuntime.client;
    const generation = hostBindingGenerationRef.current;
    await about.read({
      generation: () => hostBindingGenerationRef.current,
      load: () => client.getAbout(),
    });
    // The retention read is the second half of the same await chain, so it is
    // guarded against the generation captured before the *first* one -- leaving
    // it unguarded kept the hole open through the second call.
    if (generation !== hostBindingGenerationRef.current) return;
    await loadRetentionManifest();
  }


  // Method and Builder capabilities open the same reviewed run; the panel's
  // separate props keep the caller-side distinction (Builder output stays inert).
  function openCapabilityRun(capabilityId: string, label: string) {
    capabilitySessions.openActiveCapabilityRun({
      capabilityId,
      label,
      phase: { kind: "selecting" },
    });
  }

  /*
   * FE-10 step 3: the reviewed capability run.
   *
   * All four actions go through `runCapabilityAction`, which captures the
   * routing token before awaiting the host. That ordering is the point: a run
   * belongs to the task it was started from, and the host call outlives the
   * user's attention. Re-reading the active session after the await would offer
   * one task's manifest for approval under another task's work.
   */
  const runCapabilityAction = createCapabilityRunner({
    run: capabilityRun,
    sessions: {
      captureActive: capabilitySessions.captureActive,
      route: capabilitySessions.routeCapabilityRun,
    },
    workspace: hostRuntime.kind === "ready" && activeWorkspace !== null
      ? {
        grantEpoch: activeWorkspace.grantEpoch,
        workspaceId: activeWorkspace.workspaceId,
      }
      : null,
  });

  /// Narrows the runtime for the closures below, which need the typed client.
  const capabilityClient = hostRuntime.kind === "ready" ? hostRuntime.client : null;

  async function prepareCapabilityRun(contextPaths: readonly string[]) {
    await runCapabilityAction(anyPhase, async ({ route, run, workspace }) => {
      if (capabilityClient === null) return;
      capabilitySessions.setActiveCapabilityRun({ ...run, phase: { kind: "preparing" } });
      const review = await capabilityClient.prepareCapabilityRun(
        workspace.workspaceId,
        workspace.grantEpoch,
        run.capabilityId,
        [...contextPaths],
      );
      route((current) => current === null
        ? null
        : { ...current, phase: { kind: "review", review, contextPaths } });
    });
  }

  async function approveCapabilityRun(manifestHash: string) {
    await runCapabilityAction(underReview, async ({ route, run, workspace }) => {
      if (capabilityClient === null || run.phase.kind !== "review") return;
      const reviewPhase = run.phase;
      const approvedRun = await capabilityClient.approveCapabilityRun(
        workspace.workspaceId,
        workspace.grantEpoch,
        run.capabilityId,
        manifestHash,
      );
      route((current) => current === null
        ? null
        : {
          ...current,
          phase: {
            approved: approvedRun,
            contextPaths: reviewPhase.contextPaths,
            kind: "approved",
            review: reviewPhase.review,
            sending: false,
          },
        });
    });
  }

  async function cancelCapabilityRun(manifestHash: string, decisionId: string) {
    await runCapabilityAction(anyPhase, async ({ route, run, workspace }) => {
      if (capabilityClient === null) return;
      await capabilityClient.cancelCapabilityRun(
        workspace.workspaceId,
        workspace.grantEpoch,
        run.capabilityId,
        manifestHash,
        decisionId,
      );
      route(null);
    });
  }

  async function submitCapabilityRun(manifestHash: string, decisionId: string) {
    await runCapabilityAction(approved, async ({ route, run, workspace }) => {
      if (capabilityClient === null || run.phase.kind !== "approved") return;
      capabilitySessions.setActiveCapabilityRun({
        ...run,
        phase: { ...run.phase, sending: true },
      });
      const completed = await capabilityClient.submitCapabilityRun(
        workspace.workspaceId,
        workspace.grantEpoch,
        run.capabilityId,
        manifestHash,
        decisionId,
      );
      // The result read is best-effort: the run has already completed, and
      // failing to fetch its output must not report the run itself as failed.
      let resultJson: string | null = null;
      try {
        const latest = await capabilityClient.latestCapabilityRun(
          workspace.workspaceId,
          workspace.grantEpoch,
          run.capabilityId,
        );
        resultJson = latestResultForRun(latest, completed.runId);
      } catch {
        resultJson = null;
      }
      route((current) => current === null
        ? null
        : { ...current, phase: { kind: "completed", completed, resultJson } });
    });
  }

  async function loadRetentionManifest() {
    // `erased` is terminal for the session: the store is gone, and re-reading
    // it would replace "erased" with "unavailable" and lose the confirmation
    // the user is looking at.
    if (offboarding.status === "erased") return;
    if (
      hostRuntime.kind !== "ready"
      || !hostRuntime.bootstrap.supportedCommands.includes("app.offboarding.inspect")
    ) {
      offboarding.setStatus("unavailable");
      return;
    }
    const client = hostRuntime.client;
    await offboarding.read({
      generation: () => hostBindingGenerationRef.current,
      load: () => client.inspectOffboarding(),
    });
  }

  /**
   * Discards the renderer's mirror of everything the host's erase destroyed.
   *
   * The list this satisfies is `localDataDiscards`, which exists so a reviewer
   * can compare it against what the host actually erases. Leaving any of it up
   * shows the user data they just asked to have destroyed.
   */
  function discardErasedLocalData() {
    invalidateWorkspaceBoundUi();
    setTasks([]);
    adoptWorkspaces([], false);
    setModelAuthStatus(null);
    setTaskDrafts(new Map());
    clearContext();
    setContextDrawer(null);
    setPrimaryRoute({ kind: "home" });
  }

  async function eraseOffboardingData(confirm: string) {
    await eraseLocalData(confirm, {
      discardEverything: discardErasedLocalData,
      erase: hostRuntime.kind === "ready"
        && hostRuntime.bootstrap.supportedCommands.includes("app.offboarding.erase")
        ? (confirmation) => hostRuntime.client.eraseOffboarding(confirmation)
        : null,
      safeMessage: getSafeHostMessage,
      setNotice: setOffboardingNotice,
      setStatus: offboarding.setStatus,
    });
  }

  function persistPreferences(
    nextTheme: ThemePreference,
    nextDensity: DensityPreference,
  ) {
    if (
      hostRuntime.kind !== "ready"
      || !hostRuntime.bootstrap.supportedCommands.includes("app.preferences.set")
    ) {
      setPreferencesNotice(
        hostRuntime.kind === "browser_demo"
          ? "Preferences are not saved in the browser demo."
          : null,
      );
      return;
    }
    hostRuntime.client
      .setPreferences(nextTheme, nextDensity)
      .then(() => setPreferencesNotice(null))
      .catch(() => {
        setPreferencesNotice("The preference change could not be saved on this device.");
      });
  }

  function changeTheme(nextTheme: ThemePreference) {
    setTheme(nextTheme);
    persistPreferences(nextTheme, density);
  }

  function changeDensity(nextDensity: DensityPreference) {
    setDensity(nextDensity);
    persistPreferences(theme, nextDensity);
  }

  /*
   * FE-10 step 3: the model provider actions.
   *
   * All three shared one shape and one obligation. The obligation is the
   * important half: each of them changes the model authority -- which provider
   * and model the host will send to, and whether it is authenticated -- so
   * everything derived from the old authority has to be discarded with it. Two
   * of them called `resetModelBoundUi`; the third had the same six lines copied
   * inline. `adoptAuthority` is now the only way to publish a new status, so
   * the reset cannot be forgotten.
   */
  function adoptModelAuthority(status: ModelAuthStatusProjection) {
    // An approval granted against the previous model is not an approval of
    // this one, and a capability result was produced by a model that is no
    // longer selected.
    bmadHelpGenerationRef.current += 1;
    bmadHelpCreationRef.current = null;
    bmadHelpOperationRef.current = null;
    bmadHelpStateRef.current = initialBmadRequestState;
    setBmadHelpState(initialBmadRequestState);
    setCurrentBmadIntent(null);
    clearAllCapabilityRuns();
    setModelAuthStatus(status);
  }

  const {
    changeModelSelection,
    connectModelProvider,
    disconnectModelProvider,
  } = createModelActions({
    adoptAuthority: adoptModelAuthority,
    host: hostRuntime.kind === "ready"
      ? {
        canSelectModel: hostRuntime.bootstrap.supportedCommands.includes("model.selection.set"),
        canSignIn: hostRuntime.bootstrap.supportedCommands.includes("model.auth.sign_in"),
        canSignOut: hostRuntime.bootstrap.supportedCommands.includes("model.auth.sign_out"),
        setModelSelection: (providerId, modelId) =>
          hostRuntime.client.setModelSelection(providerId, modelId),
        signIn: (connection) => hostRuntime.client.modelAuthSignIn(connection),
        signOut: () => hostRuntime.client.modelAuthSignOut(),
      }
      : null,
    setBusy: setModelSelectionBusy,
    setNotice: setModelSelectionNotice,
  });

  function dismissUtilityPanel(restoreFocus = true) {
    utilityFocus.arm(restoreFocus);
    setAppModal(null);
  }

  function openContextDrawer(
    kind: Exclude<ContextDrawerKind, null>,
    returnFocusTarget: HTMLElement | null = null,
    focusDrawerOnOpen = false,
  ) {
    drawerFocus.capture(returnFocusTarget);
    focusContextDrawerOnOpenRef.current = focusDrawerOnOpen;
    setContextDrawer(kind);
  }

  function dismissContextDrawer(restoreFocus = true) {
    drawerFocus.arm(restoreFocus);
    focusContextDrawerOnOpenRef.current = false;
    setContextDrawer(null);
  }

  useEffect(() => {
    if (contextDrawer === null || !focusContextDrawerOnOpenRef.current) return;
    focusContextDrawerOnOpenRef.current = false;
    document.querySelector<HTMLElement>(".context-drawer__close")?.focus();
  }, [contextDrawer]);

  useEffect(() => {
    invalidateWorkspaceBoundUi();
  }, [invalidateWorkspaceBoundUi, workspaceAuthorityKey]);

  /// Reads the host's copy of the active task's transcript once per lane.
  ///
  /// The renderer's chat store is a bounded cache — twenty lanes, oldest
  /// evicted — and `clearAllLanes` empties it outright on a workspace switch or
  /// a host rebind. Both were silent data loss to look at: the conversation is
  /// still on disk and still what the host builds the next request from, but the
  /// screen showed an empty task. Nothing re-read it. This is the re-read.
  ///
  /// Activation is the trigger rather than a background sweep, because it is the
  /// only moment the answer is about to be looked at, and it is exactly when the
  /// three ways a lane goes missing all present: opening an evicted tab, and
  /// landing on a task after a workspace switch, are both activations.
  ///
  /// The lane's own `historyStatus` bounds it to one read per lane — see
  /// `beginHistoryRestore`, which claims it atomically — so re-running this
  /// effect is free and neither tab-switching nor a re-render can amplify into
  /// repeated host traffic.
  useEffect(() => {
    if (routedTaskId === null || chatHistoryClient === null || activeWorkspace === null) {
      return;
    }
    if (chatLane.historyStatus !== "unread") {
      return;
    }
    const routing = beginChatHistoryRestore(routedTaskId);
    if (routing === null) {
      return;
    }
    chatHistoryClient
      .chatHistory({
        workspaceId: activeWorkspace.workspaceId,
        workspaceGrantEpoch: activeWorkspace.grantEpoch,
        // The task id *is* the durable session ref — the same value `chat.send`
        // carries — which is what makes this read return the transcript this tab
        // has been writing to rather than some other one.
        clientSessionRef: routing.sessionId,
      })
      .then((projection) => {
        applyToChatLane(routing, (lane) => applyRestoredHistory(lane, projection));
      })
      .catch((error: unknown) => {
        // Deliberately not `markReadOnlyRecovery` even for an integrity code:
        // failing to read one transcript is not evidence the host's authority is
        // broken, and tearing the whole session down over it would be a far
        // larger response than the failure warrants. The lane says so and offers
        // a retry; everything else keeps working.
        applyToChatLane(routing, (lane) => failHistoryRestore(lane, getSafeHostMessage(error)));
      });
  }, [
    activeWorkspace,
    applyToChatLane,
    beginChatHistoryRestore,
    chatHistoryClient,
    chatLane.historyStatus,
    routedTaskId,
  ]);

  useEffect(() => {
    clearMethodLibraryProjection();
    if (!methodLibraryClient) {
      setContextDrawer((current) => current === "skills" ? null : current);
      return;
    }
    void loadMethodLibrary(methodLibraryClient, methodLibraryRebind(methodLibraryClient));
  }, [
    activeWorkspaceId,
    clearMethodLibraryProjection,
    loadMethodLibrary,
    methodLibraryClient,
    methodLibraryRebind,
  ]);

  useEffect(() => {
    const generation = ++bmadHelpGenerationRef.current;
    bmadHelpCreationRef.current = null;
    bmadHelpOperationRef.current = null;
    setModelAuthStatus(null);
    applyBmadEvent({ type: "recover_started" });
    if (!methodGuidanceClient || !activeWorkspace) {
      applyBmadEvent({ type: "recovered", run: null });
      return;
    }

    const client = methodGuidanceClient;
    const workspace = activeWorkspace;
    const projectionSequence = hostRuntime.kind === "ready"
      ? hostRuntime.bootstrap.projectionSequence
      : 0;
    void Promise.all([
      client.modelAuthStatus(),
      client.latestBmadHelpRun(workspace.workspaceId, workspace.grantEpoch),
    ])
      .then(([authStatus, result]) => {
        if (generation !== bmadHelpGenerationRef.current) {
          return;
        }
        setModelAuthStatus(authStatus);
        if (result.kind === "no_run") {
          setCurrentBmadIntent(null);
          applyBmadEvent({ type: "recovered", run: null });
          return;
        }
        if (result.kind === "retained") {
          setCurrentBmadIntent({ text: result.run.currentIntent });
          applyBmadEvent({ type: "recovered", run: result.run });
          return;
        }
        if (result.kind === "interrupted") {
          setCurrentBmadIntent({ text: result.run.currentIntent });
          applyBmadEvent({ type: "interrupted", result: result.run });
          return;
        }
        if (result.kind === "completed") {
          applyBmadEvent({ type: "completed", result: result.result });
          if (bmadHelpStateRef.current.kind !== "completed") {
            bmadHelpStateRef.current = { kind: "completed", result: result.result };
            setBmadHelpState(bmadHelpStateRef.current);
          }
          return;
        }
        if (result.kind === "terminal") {
          applyBmadEvent({ type: "terminal", reason: result.terminal.reason });
          return;
        }
        if (result.kind === "projection_unavailable") {
          applyBmadEvent({
            type: "unavailable",
            message: retainedHelpProjectionUnavailableMessage,
          });
          return;
        }
        if (authStatus.status !== "development_ready") {
          applyBmadEvent({
            type: "unavailable",
            message: "A retained request cannot be restored while model support is unavailable.",
          });
          return;
        }
        const review = result.review;
        const authority: BmadAuthoritySnapshot = {
          workspaceId: workspace.workspaceId,
          workspaceGrantEpoch: workspace.grantEpoch,
          runId: review.runId,
          authEpoch: authStatus.authEpoch,
          rendererGeneration: generation,
          now: Date.now(),
        };
        const recovered = applyBmadEvent({ type: "review_recovered", review, authority });
        if (result.kind === "approved" && recovered.kind === "review_required") {
          applyBmadEvent({ type: "approve_started" });
          applyBmadEvent({ type: "approved", approval: result.approval });
        }
      })
      .catch((error: unknown) => {
        if (generation !== bmadHelpGenerationRef.current) {
          return;
        }
        // The shared predicate, like every other host call site. This was the
        // seventh inline copy of it.
        if (isRecoveryFailure(error)) {
          markReadOnlyRecovery(client, projectionSequence);
          return;
        }
        applyBmadEvent({ type: "unavailable", message: getSafeHostMessage(error) });
      });

    return () => {
      if (generation === bmadHelpGenerationRef.current) {
        bmadHelpGenerationRef.current += 1;
      }
      bmadHelpCreationRef.current = null;
      bmadHelpOperationRef.current = null;
    };
    // `hostRuntime` and `activeWorkspace` are declared even though
    // `methodGuidanceBindingKey` already encodes the authority they carry:
    // the key is a string, so it cannot tell the linter that the *objects* read
    // in the body are current. Neither churns per poll tick — `hostRuntime` is
    // replaced only on adopt/recovery/unavailable, and `activeWorkspace` is an
    // element of `hostWorkspaces`, not a fresh object per render — so declaring
    // them costs no extra runs and makes the closure honest.
  }, [
    activeWorkspace,
    applyBmadEvent,
    hostRuntime,
    markReadOnlyRecovery,
    methodGuidanceBindingKey,
    methodGuidanceClient,
  ]);

  useEffect(() => {
    if (
      bmadHelpState.kind !== "review_required"
      && bmadHelpState.kind !== "approving"
      && bmadHelpState.kind !== "approved"
      && bmadHelpState.kind !== "submitting"
    ) return;
    const remaining = bmadHelpState.authority.expiresAt - Date.now();
    if (remaining <= 0) {
      applyBmadEvent({ type: "authority_invalidated", reason: "consent_expired" });
      return;
    }
    const timer = window.setTimeout(() => {
      applyBmadEvent({ type: "authority_invalidated", reason: "consent_expired" });
    }, Math.min(remaining, 2_147_483_647));
    return () => window.clearTimeout(timer);
  }, [applyBmadEvent, bmadHelpState]);

  /*
   * FE-10 step 3: the projection subscription.
   *
   * The loop, its timer and the cursor-gap recovery live in
   * `startProjectionSubscription`; this is the effect boundary and the ports it
   * needs. What was 135 lines of interleaved polling and classification is now
   * a subscription with a declared dependency set.
   */
  // The effect returns the subscription's stop function directly: starting
  // returns the teardown, which is exactly the contract `useEffect` wants.
  useEffect(() => startProjectionSubscription({
    adoptReboundRuntime: (client, bootstrap) => adoptHostRuntime({
      kind: bootstrap.bootMode === "ready" ? "ready" : "read_only_recovery",
      client,
      bootstrap: bootstrap as BootstrapReply,
    }, true),
    bindingGeneration: () => hostBindingGenerationRef.current,
    fetchEvents: (client, cursor) => client.projectionEvents(cursor),
    fetchSnapshot: (client) => client.projectionSnapshot(),
    libraryWasRequested: () => bmadLibraryRequestedRef.current,
    markHostUnavailable,
    markReadOnlyRecovery,
    pollIntervalMs: projectionPollIntervalMs,
    reBootstrap: (client) => client.bootstrap(),
    refreshMethodLibrary: (client) => loadMethodLibrary(client, methodLibraryRebind(client)),
    refreshWorkspaces: async (client) => {
      adoptWorkspaces(await client.listWorkspaces(), true);
    },
    runtime: hostRuntime.kind === "ready" || hostRuntime.kind === "read_only_recovery"
      ? { client: hostRuntime.client, startSequence: hostRuntime.bootstrap.projectionSequence }
      : null,
  }), [
    adoptHostRuntime,
    // Stable: `useCallback`s over stable dependencies. Naming them does not
    // restart the poller, which matters -- a poller that restarted every render
    // would hammer the host.
    adoptWorkspaces,
    methodLibraryRebind,
    // A ref object, so naming it never restarts the poller. The subscription
    // reads `.current` on every tick to decide whether its binding is still
    // the live one, which is why it must be a dependency at all.
    hostBindingGenerationRef,
    hostRuntime,
    loadMethodLibrary,
    markHostUnavailable,
    markReadOnlyRecovery,
    projectionPollIntervalMs,
  ]);

  useEffect(() => {
    if (
      hostRuntime.kind !== "ready"
      || !hostRuntime.bootstrap.supportedCommands.includes("app.preferences.get")
    ) {
      return;
    }
    let cancelled = false;
    hostRuntime.client
      .getPreferences()
      .then((preferences) => {
        if (cancelled) return;
        setTheme(preferences.theme);
        setDensity(preferences.density);
      })
      .catch(() => {
        // Defaults stay applied; preferences remain writable later.
      });
    return () => {
      cancelled = true;
    };
  }, [hostRuntime]);

  useEffect(() => {
    applyThemePreference(theme);
    if (theme !== "system") {
      return;
    }

    const colorScheme = window.matchMedia("(prefers-color-scheme: dark)");
    const onColorSchemeChange = () => applyThemePreference("system");
    colorScheme.addEventListener("change", onColorSchemeChange);
    return () => colorScheme.removeEventListener("change", onColorSchemeChange);
  }, [theme]);

  useEffect(() => {
    document.documentElement.dataset.density = density;
  }, [density]);

  /*
   * FE-10 step 3: the task lifecycle.
   *
   * Twelve closures over App state became one controller with a declared
   * dependency set. The set is large and genuinely so -- opening or deleting a
   * task touches the list, the route, drafts, chat lanes, capability runs,
   * changes flows, context and the drawer -- but it is now countable, and a new
   * dependency has to be declared rather than captured.
   */
  const {
    autoTitleFromMessage,
    closeTaskById,
    confirmTaskDeletion: confirmTaskDeletionFor,
    dismissTaskDeletion,
    renameSelectedTask,
    reopenTaskById,
    requestTaskDeletion,
    selectSession,
    startNewSession,
  } = useTaskLifecycle({
    canStartTask: () => {
      // A task started mid-request would leave the in-flight approval pointing
      // at a task the user is no longer looking at.
      const current = bmadHelpStateRef.current;
      return current.kind !== "creating"
        && current.kind !== "approving"
        && current.kind !== "approved"
        && current.kind !== "submitting";
    },
    closeDrawer: () => setContextDrawer(null),
    closeModal: () => setAppModal(null),
    createTaskId,
    drafts: {
      drop: (taskId) => setTaskDrafts((current) => {
        if (!current.has(taskId)) return current;
        const next = new Map(current);
        next.delete(taskId);
        return next;
      }),
      set: (taskId, draft) => setTaskDrafts((current) => {
        const next = new Map(current);
        next.set(taskId, draft);
        return next;
      }),
    },
    dropChatSession: (taskId) => chatSessions.dropSession(taskId),
    openDeleteConfirmation: () => setAppModal("delete-task"),
    releaseRunState: (taskId) => {
      // FE-13: the task's own session goes too -- its attachments were selected
      // against a workspace grant for a message that was never sent.
      taskSessions.close(taskId);
      // A capability run and a governed-changes flow are in-progress work, not
      // a record of what was said, so neither survives the task leaving the
      // open list. `closeCapabilitySession` also makes a late result for that
      // task drop instead of reappearing.
      capabilitySessions.closeCapabilitySession(taskId);
      setChangesFlowForTask(taskId, null);
    },
    resetGuidanceForNewTask: () => {
      bmadHelpGenerationRef.current += 1;
      bmadHelpCreationRef.current = null;
      bmadHelpOperationRef.current = null;
      bmadHelpStateRef.current = initialBmadRequestState;
      setBmadHelpState(initialBmadRequestState);
      setCurrentBmadIntent(null);
    },
    routedTaskId,
    selectedSessionId,
    setPendingDeleteTaskId,
    setPrimaryRoute,
    setTasks,
    tasks,
  });

  function confirmTaskDeletion() {
    confirmTaskDeletionFor(pendingDeleteTaskId);
  }

  function openMethodLibrary() {
    if (!methodLibraryClient) {
      return;
    }
    const openingFromUtility = appModal !== null;
    // The drawer inherits the panel's return target: closing the panel to open
    // the drawer should not send focus back to whatever opened the panel.
    const returnFocus = openingFromUtility ? utilityFocus.transfer() : null;
    if (openingFromUtility) {
      dismissUtilityPanel(false);
    }
    openContextDrawer("skills", returnFocus, openingFromUtility);
    if (!bmadLibraryRequestedRef.current) {
      void loadMethodLibrary(methodLibraryClient, methodLibraryRebind(methodLibraryClient));
    }
  }

  /*
   * FE-10 step 3: the method-guidance request lifecycle.
   *
   * Two ~90-line functions sharing three locks and a generation, all held as
   * refs closed over from this body. The dependency set is the interesting
   * part -- this flow spends the user's money and writes to their repository,
   * and every await in it is a place the authority behind the request can
   * change. It is now declared rather than captured.
   */
  const guidance = createGuidanceRequest({
    applyEvent: applyBmadEvent,
    client: methodGuidanceClient,
    contextPaths: attachmentPaths(attachments),
    creation: {
      get: () => bmadHelpCreationRef.current,
      set: (value) => { bmadHelpCreationRef.current = value; },
    },
    currentAuthority: currentBmadAuthority,
    currentGeneration: () => bmadHelpGenerationRef.current,
    currentState: () => bmadHelpStateRef.current,
    enterReadOnlyRecovery: () => {
      if (hostRuntime.kind === "ready") {
        markReadOnlyRecovery(hostRuntime.client, hostRuntime.bootstrap.projectionSequence);
      }
    },
    modelAuth: modelAuthStatus,
    nextGeneration: () => ++bmadHelpGenerationRef.current,
    now: () => Date.now(),
    operation: {
      get: () => bmadHelpOperationRef.current,
      set: (value) => { bmadHelpOperationRef.current = value; },
    },
    recordIntent: (intent) => {
      autoTitleFromMessage(selectedSessionId, intent);
      setCurrentBmadIntent({ createdAt: Date.now(), text: intent });
    },
    workspace: activeWorkspace,
  });

  function sendBmadMessage(currentIntent: string): Promise<void> {
    // The availability gate stays here: it is a property of the shell's
    // capability matrix, not of the request lifecycle.
    if (!methodGuidanceAvailableForRequests && bmadHelpCreationRef.current === null) {
      return Promise.reject(
        new HostCapabilityError("Skill guidance is unavailable for the active workspace grant."),
      );
    }
    return guidance.send(currentIntent);
  }

  async function sendChatMessage(intent: string, sessionId?: string): Promise<void> {
    const client = chatSendClient;
    const workspace = activeWorkspace;
    if (!client || !workspace) {
      throw new HostCapabilityError("Choose a local workspace before sending a message.");
    }
    if (modelAuthStatus?.status !== "development_ready") {
      throw new HostCapabilityError(
        "Connect a model provider in Settings before sending a message.",
      );
    }
    // One question at a time per task. Without this, a double-click sends the
    // same message twice and the host's strict user->assistant alternation
    // rejects the second, losing it.
    if (chatLane.sending) return;
    // Name the task after its opening message. This is the whole reason tasks
    // used to sit there called "New task": only the Home composer ever titled
    // one, so every task started from a tab or the palette stayed unnamed
    // forever. Applied before the send so the tab is labelled even if the
    // request then fails, and a no-op once the task has any title.
    autoTitleFromMessage(sessionId ?? selectedSessionId, intent);
    const dispatch = resolveMenuDispatch({
      boundAgent: chatLane.agentCode,
      intent,
      library: bmadLibraryState,
    });
    if (dispatch !== null) {
      // Record the exchange before leaving. A dispatch is still a turn the
      // user took: returning straight to the capability run cleared the
      // composer and dropped the message, so the conversation lost the very
      // sentence that caused the run to open.
      const dispatchRouting = sessionId === undefined
        ? chatSessions.captureRouting()
        : chatSessions.routingFor(sessionId);
      const dispatchedAt = Date.now();
      chatSessions.applyToLane(dispatchRouting, (lane) =>
        appendDispatchExchange(lane, intent, dispatch.label, dispatchedAt),
      );
      openCapabilityRun(dispatch.capabilityId, dispatch.label);
      return;
    }
    // The Home composer creates the task and sends in the same tick, so the
    // active-session state has not settled yet. Binding to the id the caller
    // just created keeps the exchange on the task that asked for it.
    const routing = sessionId === undefined
      ? chatSessions.captureRouting()
      : chatSessions.routingFor(sessionId);
    // Read from the task the send belongs to, not the visible one: the Home
    // composer creates a task and sends in the same tick.
    const contextPaths = attachmentPaths(
      sessionId === undefined ? attachments : taskSessions.sessionFor(sessionId).attachments,
    );
    const agentCode = chatLane.agentCode;
    chatSessions.applyToLane(routing, (lane) => beginSend(lane, intent));
    try {
      const turn = await client.sendChatMessage({
        workspaceId: workspace.workspaceId,
        workspaceGrantEpoch: workspace.grantEpoch,
        clientSessionRef: routing.sessionId,
        agentCode,
        text: intent,
        contextPaths: [...contextPaths],
      });
      chatSessions.applyToLane(routing, (lane) => applySentTurn(lane, turn));
      // Attachments are spent by the send that carried them. Leaving them armed
      // meant the next message silently re-sent, re-billed and re-entered every
      // one of them into the reviewed-context manifest — consent given once,
      // acted on every turn, with nothing on screen distinguishing "these went
      // last time" from "these go again".
      //
      // Cleared and announced in one write, on the task that sent them.
      // Previously the clear reset the notice to null and the notice had to be
      // written after it; one call cannot order itself wrongly.
      if (contextPaths.length > 0) {
        const count = contextPaths.length;
        taskSessions.update(routing.sessionId, (session) => ({
          ...session,
          attachments: clearAttachments(
            `Sent ${count} attached ${count === 1 ? "file" : "files"} with this message. `
            + `Attach again to include ${count === 1 ? "it" : "them"} in the next one.`,
          ),
        }));
      }
    } catch (error: unknown) {
      chatSessions.applyToLane(routing, (lane) => failSend(
        lane,
        getSafeHostMessage(error),
        error instanceof HostCommandError && error.details.retryable,
      ));
      throw error;
    }
  }

  async function refreshWorkflowState(): Promise<void> {
    const client = methodGuidanceClient;
    const workspace = activeWorkspace;
    if (!client || !workspace) return;
    setWorkflowState((current) => ({ ...current, loading: true, error: null }));
    try {
      const state = await client.bmadWorkflowState({
        workspaceId: workspace.workspaceId,
        workspaceGrantEpoch: workspace.grantEpoch,
      });
      setWorkflowState({ state, loading: false, error: null });
    } catch (error: unknown) {
      // Keep the progress already on screen. Nulling it collapsed four phases,
      // their gates and their evidence paths down to one error sentence, which
      // is indistinguishable from a workspace that has no method installed —
      // so a transient refresh failure looked like the feature was missing.
      // The panel renders the failure as a banner above the retained state.
      setWorkflowState((current) => ({
        state: current.state,
        loading: false,
        error: getSafeHostMessage(error),
      }));
    }
  }

  const governedEditsCommandsAvailable = hostRuntime.kind === "ready"
    && ([
      "workspace.enable_edits",
      "changes.propose",
      "approval.decide",
      "rollback.request",
      "changes.history",
      "changes.recovery.prepare",
      "changes.recovery.decide",
    ] as const)
      .every((command) => hostRuntime.bootstrap.supportedCommands.includes(command));
  // Includes the grant level, because `refreshChangesHistory` returns early on
  // `!editsEnabled` *before* it sets `loading`. Without it, a read-only
  // workspace — which is what every workspace is until the user enables
  // governed edits — reported the history as available, sat at `unread`
  // forever, and told the user to press Refresh, which called a function that
  // returned immediately. The old copy for this state ("Governed changes …
  // will appear here after they happen") was true; a broken instruction is
  // worse than the sentence it replaced.
  const changesHistoryAvailable = governedEditsCommandsAvailable
    && activeWorkspace !== null
    && activeWorkspace.permissions === "governed_edits";

  // Read the execution history when Activity is opened.
  //
  // Nothing did. `refreshChangesHistory` was reachable only from the sites that
  // mutate history and from the Refresh button, from no effect at all — so
  // opening Activity on a workspace with real governed changes showed "No
  // activity yet" indefinitely. Someone opening it to check whether yesterday's
  // change could still be rolled back was told there was nothing to roll back.
  //
  // Gated on `unread`, so this is one read per workspace binding rather than a
  // poll: `loading` is its own lock, and the binding change that resets the
  // status back to `unread` is exactly when a re-read is wanted.
  useEffect(() => {
    if (
      contextDrawer !== "activity"
      || changesHistoryStatus !== "unread"
      || !changesHistoryAvailable
    ) {
      return;
    }
    void refreshChangesHistory();
    // `refreshChangesHistory` is re-created every render and is guarded
    // internally; depending on it here would re-run this effect on every
    // render, which is the poll this is written to avoid.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [changesHistoryAvailable, changesHistoryStatus, contextDrawer]);

  const editsEnabled = activeWorkspace?.permissions === "governed_edits";
  const canEnableEdits = governedEditsCommandsAvailable
    && activeWorkspace !== null
    && activeWorkspace.permissions === "read_only"
    && !enableEditsBusy;
  const changesState: GovernedChangesUiState = (() => {
    if (hostRuntime.kind === "browser_demo") {
      return {
        kind: "unavailable",
        reason: "Governed edits require the Windows desktop host. This browser view is for visual QA only.",
      };
    }
    if (!governedEditsCommandsAvailable || hostRuntime.kind !== "ready") {
      return {
        kind: "unavailable",
        reason: "Governed local edits are unavailable in the current host mode.",
      };
    }
    if (!activeWorkspace) {
      return {
        kind: "unavailable",
        reason: "Choose a local workspace before proposing changes.",
      };
    }
    if (!editsEnabled) {
      return {
        kind: "unavailable",
        reason: "This workspace is read only. Allow governed edits to review and apply exact, checkpointed file changes.",
      };
    }
    return changesFlow ?? { kind: "idle" };
  })();

  const sessionContext = useMemo(() => {
    const reviewState = bmadHelpState.kind === "review_required"
      || bmadHelpState.kind === "approving"
      || bmadHelpState.kind === "approved"
      || bmadHelpState.kind === "submitting"
      ? bmadHelpState
      : null;
    const sessionId = reviewState?.run.sessionId
      ?? (bmadHelpState.kind === "completed" || bmadHelpState.kind === "interrupted"
        ? bmadHelpState.result.sessionId
        : bmadHelpState.kind === "idle" || bmadHelpState.kind === "unavailable"
          ? bmadHelpState.run?.sessionId
          : undefined)
      ?? selectedSessionId;
    const messages: SessionContextMessage[] = [];
    // A fabricated two-message transcript used to be pushed here in browser
    // preview, with a fabricated provider, model and token count alongside it.
    // A brand-new task that had never sent anything showed a two-turn
    // conversation with a named model and a token cost, so a reader concluded a
    // model had been contacted and billed. Nothing had been sent anywhere.
    //
    // Deleted rather than labelled: the panel already has an honest empty state
    // ("No raw messages in this session yet"), and annotating an invented
    // conversation is a worse answer than not inventing one.
    if (currentBmadIntent) {
      const id = `msg_user_${sessionId}`;
      messages.push({
        id,
        role: "user",
        createdAt: currentBmadIntent.createdAt,
        value: {
          message: {
            id,
            sessionID: sessionId,
            role: "user",
            time: currentBmadIntent.createdAt === undefined
              ? undefined
              : { created: currentBmadIntent.createdAt },
          },
          agent: "bmad",
          model: {
            modelID: sessionContextModelId,
            providerID: "forge",
          },
          parts: [{ type: "text", text: currentBmadIntent.text }],
        },
      });
    }
    if (bmadHelpState.kind === "completed") {
      const { result } = bmadHelpState;
      const id = `msg_assistant_${result.runId}`;
      const recommendation = result.recommendation.recommendationKind === "recommended_capability"
        ? `${result.recommendation.displayName}: ${result.recommendation.rationaleSummary}`
        : `No recommendation: ${result.recommendation.reasonCode}`;
      messages.push({
        id,
        role: "assistant",
        createdAt: result.receipt.completedAt,
        value: {
          message: {
            id,
            sessionID: sessionId,
            role: "assistant",
            time: {
              created: result.receipt.startedAt,
              completed: result.receipt.completedAt,
            },
          },
          agent: "bmad",
          model: {
            modelID: sessionContextModelId,
            providerID: "forge",
          },
          parts: [{ type: "text", text: recommendation }],
          bmad: result,
        },
      });
    }
    return {
      createdAt: currentBmadIntent?.createdAt
        ?? (bmadHelpState.kind === "completed"
          ? bmadHelpState.result.receipt.startedAt
          : undefined),
      // No invented token count either. It fed the context meter and the session
      // panel's "Total Tokens", so a session that had sent nothing reported a
      // cost. An absent number renders as "—", which is the truth.
      inputTokens: reviewState?.review.totalTokenEstimate ?? attachedTokens,
      messages,
    };
    // `hostRuntime.kind` was a dependency only because this used to branch on
    // browser preview to fabricate a transcript. Nothing in here reads it now.
  }, [attachedTokens, bmadHelpState, currentBmadIntent, selectedSessionId, sessionContextModelId]);

  function resetChangesFlow() {
    changesGenerationRef.current += 1;
    setChangesFlowForTask(selectedSessionId, null);
    setChangesError(null);
  }

  function resetAllChangesFlows() {
    changesGenerationRef.current += 1;
    clearAllChangesFlows();
    setChangesError(null);
  }

  useEffect(() => {
    if (recoveryReview === null) return;
    let timeout: number | null = null;
    const clearAtExpiry = () => {
      const remaining = recoveryReview.expiresAt - Date.now();
      if (remaining <= 0) {
        // A decision already in flight owns the review; expiring it under
        // the host call would drop the response.
        if (recoveryDecisionInFlight()) return;
        clearRecoveryReview();
        return;
      }
      timeout = window.setTimeout(clearAtExpiry, Math.min(remaining, 2_147_483_647));
    };
    clearAtExpiry();
    return () => {
      if (timeout !== null) window.clearTimeout(timeout);
    };
    // Declared against the whole review, not two of its fields. The body reads
    // `recoveryReview.expiresAt` through the closure, so listing the fields
    // pinned the *timer* to the current review while the *closure* stayed on
    // whichever review was current when those two fields last changed.
    // `setRecoveryReview` is called with a fresh object per prepared recovery,
    // so this adds no extra runs.
  }, [clearRecoveryReview, recoveryDecisionInFlight, recoveryReview]);

  async function refreshChangesHistory() {
    if (recoveryDecisionInFlight()) {
      return;
    }
    if (hostRuntime.kind !== "ready" || !activeWorkspace || !editsEnabled) {
      return;
    }
    clearRecoveryReview();
    const client = hostRuntime.client;
    const workspace = activeWorkspace;
    const authorityKey = workspaceAuthorityKey;
    const generation = changesGenerationRef.current;
    const sequence = hostRuntime.bootstrap.projectionSequence;
    setChangesHistoryBusy(true);
    changesHistoryState.beginRead();
    setChangesError(null);
    try {
      const history = await client.changesHistory(workspace.workspaceId, workspace.grantEpoch);
      if (
        generation === changesGenerationRef.current
        && workspaceAuthorityKeyRef.current === authorityKey
        && hostRuntime.kind === "ready"
      ) {
        changesHistoryState.completeRead(history);
      }
    } catch (error) {
      // Inside the staleness guard, like every other write in this function.
      // Outside it, a read for workspace A that rejects after the user has
      // switched to B marks *B* unavailable — and because `handleChangesFailure`
      // is guarded, `changesError` stays null, so B's Activity panel claims its
      // history could not be read, gives no reason, and never re-reads, since
      // the load-on-open effect only fires on `unread`.
      if (
        generation === changesGenerationRef.current
        && workspaceAuthorityKeyRef.current === authorityKey
      ) {
        changesHistoryState.failRead();
        handleChangesFailure(client, error, sequence);
      }
    } finally {
      if (
        generation === changesGenerationRef.current
        && workspaceAuthorityKeyRef.current === authorityKey
      ) {
        setChangesHistoryBusy(false);
      }
    }
  }

  const { decide: decideChangesRecovery, prepare: prepareChangesRecovery } =
    createRecoveryActions(recovery, {
      authorityIsCurrent: () => workspaceAuthorityKeyRef.current === workspaceAuthorityKey,
      host: hostRuntime.kind === "ready" && activeWorkspace !== null && editsEnabled
        ? {
          decide: (input) => hostRuntime.client.decideChangesRecovery(input),
          prepare: (input) => hostRuntime.client.prepareChangesRecovery(input),
        }
        : null,
      journalHasReview: (journalId) => changesHistory?.openJournals.some(
        (journal) => journal.journalId === journalId
          && journal.recoveryAvailability === "review_available",
      ) ?? false,
      manualReviewMessage: recoveryManualReviewMessage,
      refreshHistory: refreshChangesHistory,
      reportFailure: (error) => {
        if (hostRuntime.kind === "ready") {
          handleChangesFailure(
            hostRuntime.client,
            error,
            hostRuntime.bootstrap.projectionSequence,
          );
        }
      },
      setError: setChangesError,
      workspace: activeWorkspace,
    });

  function handleChangesFailure(
    client: BoundHostClient,
    error: unknown,
    sequence: number,
  ) {
    // One definition of "recovery failure", shared with the workspace and
    // changes runners. Reporting a host that demanded recovery as an ordinary
    // error leaves the app running against one that has told it to stop.
    if (isRecoveryFailure(error)) {
      markReadOnlyRecovery(client, sequence);
      return;
    }
    setChangesError(getSafeHostMessage(error));
  }

  async function enableGovernedEdits() {
    if (hostRuntime.kind !== "ready" || !activeWorkspace || !canEnableEdits) {
      return;
    }
    const client = hostRuntime.client;
    const sequence = hostRuntime.bootstrap.projectionSequence;
    const generation = ++changesGenerationRef.current;
    setEnableEditsBusy(true);
    setChangesError(null);
    try {
      const enabled = await client.enableWorkspaceEdits(activeWorkspace);
      if (generation !== changesGenerationRef.current) {
        return;
      }
      promoteWorkspace(enabled);
      clearAllChangesFlows();
      invalidateChangesHistory();
    } catch (error) {
      if (generation === changesGenerationRef.current) {
        handleChangesFailure(client, error, sequence);
      }
    } finally {
      if (generation === changesGenerationRef.current) {
        setEnableEditsBusy(false);
      }
    }
  }

  /*
   * FE-10 step 3: the governed-changes flow actions.
   *
   * `runChangesAction` captures two things before the host is called, because
   * all three actions need both and all three used to write them out:
   *
   *   `originTask` -- a governed change is a diff the user is being asked to
   *   approve, and applying it to whichever task is on screen after the await
   *   is asking them to approve edits they did not request.
   *
   *   `generation` -- the supersede mechanism. A slow proposal must not land on
   *   top of a review the user is already reading.
   */
  const runChangesAction = createChangesRunner({
    currentGeneration: () => changesGenerationRef.current,
    enterReadOnlyRecovery: () => {
      if (hostRuntime.kind === "ready") {
        markReadOnlyRecovery(hostRuntime.client, hostRuntime.bootstrap.projectionSequence);
      }
    },
    nextGeneration: () => ++changesGenerationRef.current,
    originTask: selectedSessionId,
    resetFlow: (originTask) => setChangesFlowForTask(originTask, { kind: "idle" }),
    setError: setChangesError,
  });

  async function proposeCapabilityChangeSet(capabilityId: string) {
    openContextDrawer("changes");
    if (hostRuntime.kind !== "ready" || !activeWorkspace || !editsEnabled) {
      return;
    }
    const client = hostRuntime.client;
    const workspace = activeWorkspace;
    await runChangesAction(async ({ isCurrent, originTask }) => {
      setChangesFlowForTask(originTask, { kind: "preparing" });
      const review = await client.proposeCapabilityChanges(
        workspace.workspaceId,
        workspace.grantEpoch,
        capabilityId,
      );
      if (!isCurrent()) return;
      setChangesFlowForTask(originTask, { kind: "review", busy: false, review });
    });
  }

  async function decideGovernedChange(choice: ApprovalChoice) {
    if (hostRuntime.kind !== "ready" || changesState.kind !== "review") {
      return;
    }
    const client = hostRuntime.client;
    const review = changesState.review;
    await runChangesAction(async ({ isCurrent, originTask }) => {
      setChangesFlowForTask(originTask, { kind: "review", busy: true, review });
      const decision = await client.decideApproval(review, choice);
      if (!isCurrent()) return;
      if (decision.disposition === "applied" && decision.execution) {
        setChangesFlowForTask(originTask, {
          kind: "applied",
          busy: false,
          execution: decision.execution,
        });
        invalidateChangesHistory();
      } else if (decision.disposition === "discarded") {
        setChangesFlowForTask(originTask, { kind: "discarded" });
      } else {
        setChangesFlowForTask(originTask, { kind: "idle" });
      }
    });
  }

  async function undoGovernedChange(executionId: string) {
    if (hostRuntime.kind !== "ready" || !activeWorkspace || !editsEnabled) {
      return;
    }
    const client = hostRuntime.client;
    const priorState = changesState;
    await runChangesAction(async ({ isCurrent, originTask }) => {
      setChangesFlowForTask(
        originTask,
        priorState.kind === "applied"
          ? { kind: "applied", busy: true, execution: priorState.execution }
          : { kind: "preparing" },
      );
      const result = await client.requestRollback(executionId);
      if (!isCurrent()) return;
      if (result.kind === "review") {
        setChangesFlowForTask(originTask, { kind: "review", busy: false, review: result.value });
      } else {
        setChangesFlowForTask(originTask, { kind: "undo_unavailable", value: result.value });
      }
    });
  }

  /*
   * The three mutating workspace actions, built from the controller.
   *
   * Each used to repeat the same six steps by hand -- guard, mark busy, clear
   * the error, call the host, classify the failure, clear busy. The
   * classification is the part that mattered: `recovery_required` and
   * `integrity_failure` are the host saying its own state is untrustworthy, and
   * an action that missed that branch reported a host demanding recovery as an
   * ordinary error while the app carried on believing it was healthy. It is now
   * written once, in `runWorkspaceAction`.
   */
  const { activateWorkspace, removeWorkspace, selectWorkspace } = createWorkspaceActions(
    workspaceState,
    {
      host: hostRuntime.kind === "ready"
        ? {
          canActivate: canActivateWorkspace,
          canRemove: canRemoveWorkspace,
          canSelect: canSelectWorkspace,
          enterReadOnlyRecovery: () => markReadOnlyRecovery(
            hostRuntime.client,
            hostRuntime.bootstrap.projectionSequence,
          ),
          revokeWorkspace: (expected) => hostRuntime.client.revokeWorkspace(expected),
          selectWorkspace: () => hostRuntime.client.selectWorkspace(),
        }
        : null,
      onActiveWorkspaceChanged: () => {
        resetAllChangesFlows();
        invalidateChangesHistory();
        clearAllCapabilityRuns();
      },
    },
  );

  function openWorkspaceManager() {
    workspacePanelFocus.capture();
    setContextDrawer(null);
    setAppModal("workspace-manager");
  }

  function openWorkspaceManagerFromUtilityPanel() {
    // Same inheritance as `openMethodLibrary`: the manager replaces the panel,
    // so it takes over the panel's return target rather than capturing the
    // panel itself as the place to go back to.
    workspacePanelFocus.capture(utilityFocus.transfer());
    setContextDrawer(null);
    setAppModal("workspace-manager");
  }

  // `paletteOpen` belongs here. The palette declares `aria-modal="true"`, and
  // that is a claim about the rest of the page, not about the palette: with the
  // palette excluded from this predicate the title bar behind it stayed
  // focusable and stayed exposed to assistive technology, so the "modal" was
  // one Tab press deep.
  const shellOverlayOpen = appModal !== null
    || paletteOpen
    || (drawerIsOverlay && contextDrawer !== null);

  /*
   * FE-10: the shell model.
   *
   * Grouped by feature so the composition reads as a map of what the app is
   * made of. The surfaces below take no props -- between them they need 122 of
   * this component's locals, and a hundred-parameter interface is not a
   * boundary.
   */
  const shellModel: ShellModel = {
    bmad: {
      approveRun: (manifestHash) => void approveCapabilityRun(manifestHash),
      cancelRun: (manifestHash, decisionId) => void cancelCapabilityRun(manifestHash, decisionId),
      closeRun: () => capabilitySessions.setActiveCapabilityRun(null),
      helpState: bmadHelpState,
      library: bmadLibraryState,
      openRun: openCapabilityRun,
      personaPerspectives,
      prepareRun: (contextPaths) => void prepareCapabilityRun(contextPaths),
      proposeChangeSet: (capabilityId) => void proposeCapabilityChangeSet(capabilityId),
      refreshWorkflow: () => void refreshWorkflowState(),
      reloadLibrary: () => {
        if (methodLibraryClient) {
          void loadMethodLibrary(methodLibraryClient, methodLibraryRebind(methodLibraryClient));
        }
      },
      run: capabilityRun,
      submitRun: (manifestHash, decisionId) => void submitCapabilityRun(manifestHash, decisionId),
      viewPersona: (agentCode) => void viewPersonaPerspective(agentCode),
      workflow: workflowState,
    },
    changes: {
      canEnableEdits,
      decide: (choice) => void decideGovernedChange(choice),
      enableEdits: () => void enableGovernedEdits(),
      enableEditsBusy,
      error: changesError,
      gitReviewAvailabilityMessage,
      gitReviewSource,
      history: changesHistory,
      historyAvailable: changesHistoryAvailable,
      historyBusy: changesHistoryBusy,
      historyStatus: changesHistoryStatus,
      refreshHistory: () => void refreshChangesHistory(),
      startNewProposal: resetChangesFlow,
      state: changesState,
      undo: (executionId) => void undoGovernedChange(executionId),
    },
    context: {
      adoptReviewed: (projection) => {
        // A reviewed selection supersedes any auto-suggestion in flight, and
        // becomes a manual one: the user chose these files.
        taskSessions.supersede();
        taskSessions.updateActive((session) => ({
          ...session,
          attachments: adoptPreview(projection, "manual"),
        }));
      },
      session: {
        createdAt: sessionContext.createdAt,
        inputTokens: sessionContext.inputTokens,
        messages: sessionContext.messages,
        // The same per-task value the composer's Context surface reads, so the
        // drawer and the composer cannot report different numbers for one
        // session.
        modelContext: taskSessions.session.modelContext,
      },
    },
    deletion: pendingDeleteTask === null ? null : {
      cancel: dismissTaskDeletion,
      confirm: confirmTaskDeletion,
      /*
       * Browser preview never dispatches the read -- `chatHistoryClient`
       * requires a ready host -- so the lane sits at `unread` for the session
       * and the dialog would say Forge has not finished reading this task's
       * messages. There is no host, nothing to read, and the count is provably
       * right. `loaded` is the honest status for a surface with no store
       * behind it.
       */
      historyStatus: hostRuntime.kind === "browser_demo"
        ? "loaded"
        : chatSessions.historyStatusFor(pendingDeleteTask.id),
      /*
       * A browser preview has no host and no store, so nothing was ever
       * written. A task proven to hold no messages never reached `chat.send`,
       * so no durable row exists for it either. Anything else -- including a
       * history read that has not finished or failed -- might have one, and
       * the warning stands.
       */
      mayHaveDurableCopy: hostRuntime.kind !== "browser_demo"
        && !(
          chatSessions.historyStatusFor(pendingDeleteTask.id) === "loaded"
          && chatSessions.messageCountFor(pendingDeleteTask.id) === 0
        ),
      messageCount: chatSessions.messageCountFor(pendingDeleteTask.id),
      taskTitle: pendingDeleteTask.title,
    },
    host: { runtimeKind: hostRuntime.kind },
    model: {
      destinationLabel: modelAccess.label,
      modelId: sessionContextModelId,
      providerLabel: modelProviderLabel,
    },
    overlays: {
      closeDrawer: dismissContextDrawer,
      drawerIsOverlay,
      drawerKind: contextDrawer,
      modalKind: appModal,
      selectDrawerTab: (tab) => setContextDrawer(tab),
    },
    recovery: {
      busy: recoveryBusy,
      decide: (choice) => void decideChangesRecovery(choice),
      prepare: (journalId, trigger) => void prepareChangesRecovery(journalId, trigger),
      returnFocusTarget: recoveryReturnFocusTarget,
      review: recoveryReview,
    },
    selectedTaskTitle: selectedTask?.title ?? null,
    settings: {
      props: {
        about: aboutProjection,
        aboutStatus,
        agentStatusLabel: methodStatusLabel,
        density,
        initialSection: utilitySettingsPage,
        modelAccessDetail: modelAccess.detail,
        modelAccessLabel: modelAccess.label,
        modelAuthStatus,
        modelSelection,
        modelSelectionBusy,
        modelSelectionNotice,
        offboardingManifest,
        offboardingNotice,
        offboardingStatus,
        onClose: () => dismissUtilityPanel(),
        onDensityChange: changeDensity,
        onEraseOffboarding: (confirm: string) => void eraseOffboardingData(confirm),
        onManageWorkspaces: openWorkspaceManagerFromUtilityPanel,
        onModelConnect: connectModelProvider,
        onModelDisconnect: disconnectModelProvider,
        onModelSelectionChange: changeModelSelection,
        onOpenSkillsAndAgents: openMethodLibrary,
        onThemeChange: changeTheme,
        preferencesNotice,
        runtimeLabel: hostStatusLabel,
        skillsAgentsAvailable: methodLibraryAvailable,
        skillsAgentsStatusLabel,
        theme,
        updateStatusLabel: aboutProjection === null
          ? "Managed by your organization"
          : aboutProjection.updateConfigured
            ? "Managed release channel"
            : "Not configured for this build",
        workspaceDetail: workspaceDescription,
        workspaceLabel: workspaceName,
      },
      section: utilitySettingsPage,
    },
    views: {
      home: {
        activeWorkspaceId,
        canOpenFolder: canSelectWorkspace,
        closedTasks,
        composerDisabled: !localTaskDraftingAvailable
          || !activeWorkspace
          || methodRequestInFlight,
        /*
         * Home cannot tell "you have not opened a folder yet" from "Forge
         * cannot reach the process that would open one" without this, and the
         * two want opposite things from the user. Unwired it assumes `ready`
         * and tells someone to open a folder while the host that would open it
         * is still starting.
         */
        hostKind: hostRuntime.kind,
        onActivateWorkspace: activateWorkspace,
        onNewTask: () => { startNewSession(); },
        onOpenWorkspace: () => void selectWorkspace(),
        onOpenWorkspaceManager: openWorkspaceManager,
        onReopenTask: reopenTaskById,
        onRetryHost: hostRuntime.kind === "unavailable" ? retryHostRuntime : undefined,
        onSelectTask: selectSession,
        onSubmitIntent: (intent: string) => {
          const startedId = startNewSession(intent, !chatSendAvailable);
          // The Home composer opens an ordinary conversation, like any other
          // chat app. BMAD joins it only if asked for.
          if (startedId !== null && chatSendAvailable) {
            void sendChatMessage(intent, startedId).catch(() => undefined);
          }
        },
        recentTasks: recentTaskList,
        /*
         * Names the blocker that is actually in the way.
         *
         * This read `chatSendAvailable` alone, so with no workspace bound it
         * told the user to connect OpenAI -- and connecting one would not have
         * enabled the composer, because what disabled it was the missing
         * folder. Advice that cannot work is worse than none: the user follows
         * it, nothing changes, and they have no reason to look elsewhere.
         *
         * Ordered the way the composer's own gate is ordered, so the hint and
         * the disabled state can never disagree about why.
         */
        statusHint: !activeWorkspace
          ? "Open a project folder to start a task."
          : methodRequestInFlight
            ? "Finishing the current request."
            : chatSendAvailable
              ? "Ready to send through the selected model."
              : "Start locally, then connect OpenAI in Settings before sending.",
        workspaces: hostWorkspaces,
        workspaceStatusLabel: workspaceDescription,
      },
      noWorkspace: {
        copy: explorerAvailabilityMessage,
        mode: hostRuntime.kind,
        onOpenWorkspace: () => void selectWorkspace(),
        onRetryHost: hostRuntime.kind === "unavailable" ? retryHostRuntime : undefined,
      },
      task: selectedTask === null || activeWorkspace === null ? null : {
        // Remounts on a task switch *or* a guidance rebind: the composer and
        // the transcript are per-task, and carrying either across a binding
        // change would show one task's state under another's title.
        key: `${selectedSessionId}:${methodGuidanceBindingKey}`,
        props: {
          agentLibrary: bmadLibraryState,
          attachments,
          canAttachFiles: canBrowseFiles || canBrowseFolder,
          chatLane,
          chatSendAvailable,
          hostStatusLabel,
          initialDraft: taskDrafts.get(selectedTask.id) ?? "",
          interactionDisabled: !localTaskDraftingAvailable,
          isBrowserDemo: hostRuntime.kind === "browser_demo",
          isReadOnlyRecovery: hostRuntime.kind === "read_only_recovery",
          methodGuidanceAvailable: methodGuidanceAvailableForRequests,
          methodGuidanceState: bmadHelpState,
          methodLibraryAvailable,
          modelAccessDetail: modelAccess.detail,
          modelAccessLabel: modelAccess.label,
          // This task's own measurement, not the app's. FE-13 keyed it per task
          // precisely so a figure measured for one conversation cannot be shown
          // beside another's attachments.
          modelContext: taskSessions.session.modelContext,
          onAskMethodGuidance: sendBmadMessage,
          onAttachFiles: canBrowseFiles ? browseAndAttachFiles : undefined,
          onAttachFolder: canBrowseFolder ? browseAndAttachFolder : undefined,
          // Both gated on the same capability that gates attaching in the first
          // place. A remove control on a surface that cannot re-add is a
          // one-way door, and a retry needs the same host reach the pick did.
          onRemoveAttachment: canBrowseFiles ? removeAttachment : undefined,
          onRetryAttachment: canBrowseFiles ? retryAttachment : undefined,
          onClearContext: clearContext,
          onCloseTask: () => closeTaskById(selectedTask.id),
          onDeleteTask: () => requestTaskDeletion(selectedTask.id),
          onDraftChange: (draft: string) => {
            setTaskDrafts((current) => {
              const next = new Map(current);
              if (draft) next.set(selectedTask.id, draft);
              else next.delete(selectedTask.id);
              return next;
            });
          },
          onOpenChanges: () => openContextDrawer("changes"),
          onOpenContext: () => openContextDrawer("context"),
          onOpenMethodLibrary: openMethodLibrary,
          onOpenProviders: () => openUtilityPanel("settings", "agent-model"),
          onOpenRunDetails: () => openContextDrawer("activity"),
          onOpenWorkspaceManager: openWorkspaceManager,
          onRenameTask: renameSelectedTask,
          onRetryHistory: () => retryChatHistoryRestore(selectedTask.id),
          onSelectAgent: (agentCode: string | null) => {
            chatSessions.setActiveLane((lane) => ({ ...lane, agentCode }));
          },
          onSendRequest: sendChatMessage,
          onSuggestContext: suggestContext,
          sessionTitle: selectedTask.title,
          workspaceName,
          workspaceStatusLabel: workspaceDescription,
        },
      },
    },
    workspace: {
      active: activeWorkspace,
      explorerAvailabilityMessage,
      manager: {
        activate: activateWorkspace,
        canActivate: canActivateWorkspace,
        canRemove: canRemoveWorkspace,
        canSelect: canSelectWorkspace,
        close: dismissWorkspacePanel,
        error: workspaceActionError,
        remove: (workspaceId) => void removeWorkspace(workspaceId),
        removalBusyId: workspaceRemovalBusyId,
        select: () => void selectWorkspace(),
        selectionBusy: workspaceSelectionBusy,
        workspaces: hostWorkspaces,
      },
      name: workspaceName,
      source: workspaceSource,
    },
  };

  const drawer = contextDrawer ? <AppDrawer /> : undefined;

  /*
   * `hasRenderableModal`, not `appModal !== null`. `AppShellLayout` decides
   * focus containment and where Escape goes from whether it was given a modal
   * at all, so an element that renders nothing would trap focus in an empty
   * container -- reachable for `"account"`, which has no dialog yet.
   */
  const modal = hasRenderableModal(shellModel.overlays.modalKind, shellModel.deletion)
    ? <AppModals />
    : undefined;

  /*
   * FE-09: one catalog behind every entry point.
   *
   * The palette used to build its own array and Ctrl+K used to be an unrelated
   * document listener, so the two could disagree about what a command was and
   * whether it could run. Both now read the same registry, and a command that
   * cannot run is *listed with its reason* rather than omitted -- an omitted
   * command is indistinguishable from a feature the app does not have.
   */
  const commandContext: CommandContext = {
    selectedTaskId: selectedTask?.id ?? null,
    selectedTaskTitle: selectedTask?.title ?? null,
    workspaceAvailable: hostRuntime.kind !== "browser_demo",
    actions: {
      startNewSession: () => { startNewSession(); },
      selectSession,
      closeTask: closeTaskById,
      requestTaskDeletion,
      openWorkspaceManager,
      openContextDrawer,
      openSettings: () => openUtilityPanel("settings"),
      togglePalette: () => setPaletteOpen((current) => !current),
    },
  };
  const commandRegistry = createCommandRegistry(commandContext);
  const apple = isApplePlatform();

  // Still a real document listener, not a React handler on the shell: React
  // attaches at the root container, so a keydown whose target is <body> — the
  // state on first paint, and again every time the palette or a disabled
  // composer blurs — never reached the shell and Ctrl+K silently did nothing.
  //
  // What changed is what it does once it fires: it resolves an id and hands it
  // to the registry, so a shortcut cannot run something the palette greys.
  const commandRegistryRef = useRef(commandRegistry);
  const commandContextRef = useRef(commandContext);
  commandRegistryRef.current = commandRegistry;
  commandContextRef.current = commandContext;
  useEffect(() => {
    function onShortcut(event: KeyboardEvent) {
      const id = commandForEvent(commandRegistryRef.current, event, apple);
      if (id === undefined) return;
      event.preventDefault();
      void commandRegistryRef.current.invoke(id, commandContextRef.current, "keybinding");
    }
    document.addEventListener("keydown", onShortcut);
    return () => document.removeEventListener("keydown", onShortcut);
  }, [apple]);

  const paletteActions: readonly PaletteAction[] = commandRegistry
    .resolveAll(commandContext)
    .map(({ definition, availability }) => ({
      id: definition.id,
      label: definition.title,
      category: definition.category,
      ...(definition.keywords === undefined ? {} : { keywords: definition.keywords }),
      ...(definition.keybinding === undefined
        ? {}
        : { keybinding: formatKeybinding(definition.keybinding, apple) }),
      ...(availability.kind === "unavailable"
        ? { unavailableReason: availability.reason }
        : {}),
      run: () => {
        void commandRegistry.invoke(definition.id, commandContext, "palette");
      },
    }));

  return (
    <ShellProvider model={shellModel}>
      <AppWorkbench
      commandPalette={
        <CommandPalette
          actions={paletteActions}
          onClose={() => setPaletteOpen(false)}
          open={paletteOpen}
        />
      }
      drawer={drawer}
      main={<AppMainView route={primaryRoute} />}
      modal={modal}
      onCloseDrawer={dismissContextDrawer}
      onCloseModal={() => {
        if (appModal === "workspace-manager") {
          dismissWorkspacePanel();
        } else if (appModal === "delete-task") {
          // Escape and the scrim both mean "no". Dismissing a destructive
          // confirmation must never be the thing that runs it.
          dismissTaskDeletion();
        } else {
          dismissUtilityPanel();
        }
      }}
      paletteOpen={paletteOpen}
      shellOverlayOpen={shellOverlayOpen}
      titleBar={{
        activeTaskId: routedTaskId ?? undefined,
        onCloseTask: closeTaskById,
        onHome: () => {
          setContextDrawer(null);
          setPrimaryRoute({ kind: "home" });
        },
        onMenu: () => setPaletteOpen(true),
        onNewTask: () => { startNewSession(); },
        onOpenSettings: () => openUtilityPanel("settings"),
        onSelectTask: selectSession,
        taskTabs: openTaskTabs,
      }}
    />
    </ShellProvider>
  );
}
