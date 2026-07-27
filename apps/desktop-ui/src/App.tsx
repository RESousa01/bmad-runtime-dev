import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  BmadLibraryPanel,
  type BmadPersonaPerspectiveView,
} from "./components/BmadLibraryPanel";
import { GovernedChangesPanel } from "./components/GovernedChangesPanel";
import {
  ReviewPanel,
  type GitReviewSource,
} from "./components/GitReviewPanel";
import type { ContextSelectionMode } from "./components/ContextWindow";
import {
  SessionContextPanel,
  type SessionContextMessage,
} from "./components/SessionContextPanel";
import "./components/review-panel.css";
import { TaskWorkspace } from "./components/TaskWorkspace";
import { TitleBar } from "./components/TitleBar";
import { BmadCapabilityPanel } from "./components/BmadCapabilityPanel";
import { BmadWorkflowPanel } from "./components/BmadWorkflowPanel";
import {
  type CapabilityRouting,
  MAX_LANE_SESSIONS,
  useCapabilitySessions,
} from "./state/taskSessions";
import {
  appendDispatchExchange,
  applyRestoredHistory,
  failHistoryRestore,
  useChatSessions,
} from "./state/chatSessions";
import {
  appendTask,
  autoTitleTask,
  closeTask,
  closedTasks as closedTaskList,
  createTask,
  createTaskId,
  deleteTask,
  findTask,
  nextSelectionAfter,
  openTasks as openTaskList,
  renameTask,
  reopenTask,
  type TaskRecord,
} from "./state/taskList";
import { DeleteTaskDialog } from "./components/DeleteTaskDialog";
import { matchMenuIntent } from "./lib/menuIntent";
import {
  type OffboardingStatus,
  SettingsDialog,
  type SettingsSection,
} from "./components/settings/SettingsDialog";
import "./components/settings/settings.css";
import { WorkspacePanel } from "./components/WorkspacePanel";
import { WorkspaceExplorer } from "./components/WorkspaceExplorer";
import { CommandPalette, type PaletteAction } from "./components/CommandPalette";
import { HomeView } from "./components/HomeView";
import {
  AppShellLayout,
  DRAWER_OVERLAY_QUERY,
} from "./components/redesign/AppShellLayout";
import {
  ContextDrawer,
  type ContextDrawerKind,
} from "./components/redesign/ContextDrawer";
import { NoWorkspaceState } from "./components/redesign/NoWorkspaceState";
import { ActivityPanel } from "./components/panels/ActivityPanel";
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
  resetDefaultHostRuntime,
  type AboutProjection,
  type BmadWorkflowStateProjection,
  type RetentionManifestProjection,
  type ApprovalChoice,
  type DensityPreference,
  type DesktopHostClient,
  type ThemePreference,
  HostCapabilityError,
  HostCommandError,
  type ContextPreviewProjection,
  type ChangesHistoryProjection,
  type ChangesRecoveryPrepared,
  type HostRuntime,
  type RecoveryApprovalChoice,
  type WorkspaceProjection,
  type WorkspaceFilePick,
  type WorkspaceSearchMatch,
} from "./lib/hostClient";
import type { GovernedChangesUiState } from "./components/GovernedChangesPanel";
import type {
  BmadLibrarySnapshot,
  BmadLibraryUiState,
} from "./lib/bmadProjection";
import {
  bmadRequestAuthorityIsCurrent,
  initialBmadRequestState,
  transitionBmadRequest,
  type BmadAuthoritySnapshot,
  type BmadRequestEvent,
  type BmadRequestState,
  type ModelAuthStatusProjection,
  type ModelProviderConnection,
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

type HostUiRuntime = HostRuntime | { kind: "loading" };

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
  const [aboutProjection, setAboutProjection] = useState<AboutProjection | null>(null);
  const [aboutStatus, setAboutStatus] = useState<"loading" | "unavailable" | "ready">(
    "unavailable",
  );
  const [preferencesNotice, setPreferencesNotice] = useState<string | null>(null);
  const [offboardingManifest, setOffboardingManifest] =
    useState<RetentionManifestProjection | null>(null);
  const [offboardingStatus, setOffboardingStatus] =
    useState<OffboardingStatus>("unavailable");
  const [offboardingNotice, setOffboardingNotice] = useState<string | null>(null);
  // Per-task capability execution state lives in `capabilitySessions` (keyed by
  // task session id) so a late completion routes to its owning task instead of
  // whichever tab is visible. Bound after `selectedSessionId` is derived.
  const [attachNotice, setAttachNotice] = useState<string | null>(null);
  const [hostRuntime, setHostRuntime] = useState<HostUiRuntime>({ kind: "loading" });
  /// Bumped to re-run the bootstrap effect. `unavailable` is otherwise
  /// terminal for the life of the window, because the default loader memoises
  /// its promise — so a host that failed to verify once, or a persisted
  /// projection this renderer could not parse, could never be retried.
  const [hostRuntimeAttempt, setHostRuntimeAttempt] = useState(0);
  const [hostWorkspaces, setHostWorkspaces] = useState<WorkspaceProjection[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string | null>(null);
  const [workspaceSelectionBusy, setWorkspaceSelectionBusy] = useState(false);
  const [workspaceRemovalBusyId, setWorkspaceRemovalBusyId] = useState<string | null>(null);
  const [workspaceActionError, setWorkspaceActionError] = useState<string | null>(null);
  const [contextPreview, setContextPreview] = useState<ContextPreviewProjection | null>(null);
  const [contextSelectionMode, setContextSelectionMode] =
    useState<ContextSelectionMode>("manual");
  const [contextSuggestionBusy, setContextSuggestionBusy] = useState(false);
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
  const [changesHistory, setChangesHistory] = useState<ChangesHistoryProjection | null>(null);
  /// Whether the workspace's execution history has actually been read.
  ///
  /// `changesHistory === null` meant both "never asked" and "asked, nothing
  /// there", and the Activity panel rendered the pair identically as "No
  /// activity yet". Worse, nothing asked: `refreshChangesHistory` was reachable
  /// only from post-mutation sites and the Refresh button, from no effect at
  /// all — so opening Activity on a workspace with real governed history showed
  /// "No activity yet" until the user happened to press Refresh. Someone
  /// checking whether a change could still be rolled back was told it could not.
  const [changesHistoryStatus, setChangesHistoryStatus] = useState<
    "unread" | "loading" | "loaded" | "unavailable"
  >("unread");
  const [changesHistoryBusy, setChangesHistoryBusy] = useState(false);
  const [recoveryReview, setRecoveryReview] = useState<
    Extract<ChangesRecoveryPrepared, { status: "review_required" }> | null
  >(null);
  const [recoveryBusy, setRecoveryBusy] = useState(false);
  const [recoveryReturnFocusTarget, setRecoveryReturnFocusTarget] =
    useState<HTMLElement | null>(null);
  const [enableEditsBusy, setEnableEditsBusy] = useState(false);
  const changesGenerationRef = useRef(0);
  const recoveryGenerationRef = useRef(0);
  const recoveryDecisionRef = useRef(false);
  const recoveryPreparationRef = useRef(false);
  const drawerIsOverlay = useMediaQuery(DRAWER_OVERLAY_QUERY);
  const workspaceActionBusyRef = useRef(false);
  const workspaceReturnFocusRef = useRef<HTMLElement | null>(null);
  const utilityReturnFocusRef = useRef<HTMLElement | null>(null);
  const pendingUtilityReturnFocusRef = useRef<HTMLElement | null>(null);
  const contextDrawerReturnFocusRef = useRef<HTMLElement | null>(null);
  const pendingContextDrawerReturnFocusRef = useRef<HTMLElement | null>(null);
  const focusContextDrawerOnOpenRef = useRef(false);
  const hostBindingGenerationRef = useRef(0);
  const bmadProjectionGenerationRef = useRef(0);
  const bmadLibraryRequestedRef = useRef(false);
  const bmadHelpGenerationRef = useRef(0);
  const bmadHelpCreationRef = useRef<Promise<void> | null>(null);
  const bmadHelpOperationRef = useRef<"approve" | "cancel" | "submit" | null>(null);
  const bmadHelpStateRef = useRef<BmadRequestState>(initialBmadRequestState);
  const workspaceAuthorityKeyRef = useRef("");
  const contextSuggestionGenerationRef = useRef(0);
  const contextSuggestionInFlightRef = useRef(false);

  const routedTaskId = primaryRoute.kind === "task" ? primaryRoute.taskId : null;
  const selectedSessionId = routedTaskId ?? NO_TASK_SESSION_ID;
  const capabilitySessions = useCapabilitySessions(selectedSessionId);
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
  const activeWorkspace = hostWorkspaces.find(({ workspaceId }) => workspaceId === activeWorkspaceId)
    ?? hostWorkspaces[0]
    ?? null;
  const workspaceName = activeWorkspace?.displayName ?? "No local workspace selected";
  const workspaceDescription = hostRuntime.kind === "browser_demo"
    ? "Sample data · Read only"
    : activeWorkspace
      ? activeWorkspace.permissions === "governed_edits"
        ? "Governed edits"
        : "Read only"
      : "Choose a workspace";
  const hostStatusLabel = (() => {
    switch (hostRuntime.kind) {
      case "loading": return "Verifying local host";
      case "browser_demo": return "Browser preview";
      case "ready": return "Local host ready";
      case "read_only_recovery": return "Read-only recovery";
      case "unavailable": return "Host unavailable";
    }
  })();
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
  const methodRequestAvailable = methodGuidanceClient !== null
    && modelAuthStatus?.status === "development_ready";
  const methodRequestInFlight = bmadHelpState.kind === "creating"
    || bmadHelpState.kind === "review_required"
    || bmadHelpState.kind === "approving"
    || bmadHelpState.kind === "approved"
    || bmadHelpState.kind === "submitting";
  const localTaskDraftingAvailable = hostRuntime.kind === "ready"
    || hostRuntime.kind === "browser_demo";
  const modelAccess = (() => {
    if (hostRuntime.kind === "browser_demo") {
      return {
        detail: "No local folder, consent grant, or model request is active.",
        label: "No model access",
      };
    }
    if (modelAuthStatus?.status === "development_ready") {
      const modeLabel = modelAuthStatus.mode === "deterministic_development"
        ? "Deterministic development"
        : modelAuthStatus.providerId === "azure"
          ? "Azure OpenAI"
          : modelAuthStatus.providerId === "openai"
            ? "OpenAI"
            : "Direct provider";
      return {
        detail: `${modeLabel} · ready`,
        label: modelAuthStatus.destinationLabel || "Development model",
      };
    }
    if (hostRuntime.kind === "loading") {
      return { detail: "Waiting for the native desktop host.", label: "Checking access" };
    }
    if (methodGuidanceClient !== null && modelAuthStatus === null) {
      return { detail: "The desktop host is checking the current model connection.", label: "Checking model access" };
    }
    return {
      detail: methodLibraryAvailable
        ? "Choose OpenAI or Azure OpenAI in Settings to send messages."
        : "No model provider or skills catalog is active.",
      label: "Connect a provider",
    };
  })();
  const modelProviderLabel = modelAuthStatus?.providerId === "azure"
    ? "Azure OpenAI"
    : modelAuthStatus?.providerId === "openai"
      ? "OpenAI"
      : modelAuthStatus?.providerId === "forge-local"
        ? "Forge local"
        : "Forge BMAD";
  const sessionContextModelId = hostRuntime.kind === "browser_demo"
    ? "forge-bmad-preview"
    : modelAuthStatus?.modelId;
  const modelSelection: ModelSelection = {
    providerId: modelAuthStatus?.providerId === "azure" ? "azure" : "openai",
    modelId: modelAuthStatus?.modelId ?? "gpt-5-mini",
  };
  const methodStatusLabel = methodRequestAvailable
    ? "Model ready"
    : methodGuidanceAvailable
      ? "Local only"
    : hostRuntime.kind === "browser_demo"
      ? "Demo only"
      : "Not connected";
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

  async function browseAndAttach(
    picker: (client: DesktopHostClient, workspaceId: string) => Promise<WorkspaceFilePick>,
  ) {
    if (hostRuntime.kind !== "ready" || !activeWorkspace || !workspaceSource) {
      return;
    }
    const client = hostRuntime.client;
    const workspaceId = activeWorkspace.workspaceId;
    const existingPaths = contextPreview?.items.map(({ relativePath }) => relativePath) ?? [];
    const generation = ++contextSuggestionGenerationRef.current;
    const authorityKey = workspaceAuthorityKey;
    setAttachNotice(null);
    try {
      const pick = await picker(client, workspaceId);
      if (pick.kind === "no_selection" || pick.value.relativePaths.length === 0) {
        const rejected = pick.kind === "picked"
          ? pick.value.rejectedOutsideRoot + pick.value.rejectedUnreadable
          : 0;
        if (rejected > 0) {
          setAttachNotice(
            `No files were attached. ${rejected} were outside the workspace or unreadable.`,
          );
        }
        return;
      }
      const combined = new Map<string, string>();
      for (const relativePath of [...existingPaths, ...pick.value.relativePaths]) {
        combined.set(relativePath.toLocaleLowerCase("en-US"), relativePath);
      }
      const relativePaths = [...combined.values()].slice(0, 100);
      const projection = await workspaceSource.previewContext(
        relativePaths,
      );
      if (
        generation !== contextSuggestionGenerationRef.current
        || authorityKey !== workspaceAuthorityKeyRef.current
      ) {
        return;
      }
      setContextPreview(projection);
      setContextSelectionMode("manual");
      const skipped = pick.value.rejectedOutsideRoot + pick.value.rejectedUnreadable;
      const truncated = pick.value.truncated || combined.size > relativePaths.length;
      setAttachNotice(
        skipped > 0
          ? `Added ${pick.value.selectedCount} files. ${skipped} unsupported files were skipped.`
          : truncated
            ? "Added the first 100 supported files."
            : null,
      );
    } catch (error) {
      if (generation === contextSuggestionGenerationRef.current) {
        setAttachNotice(getSafeHostMessage(error));
      }
    }
  }

  function browseAndAttachFiles(): Promise<void> {
    return browseAndAttach((client, workspaceId) => client.pickWorkspaceFiles(workspaceId));
  }

  function browseAndAttachFolder(): Promise<void> {
    return browseAndAttach((client, workspaceId) => client.pickWorkspaceFolder(workspaceId));
  }

  async function suggestContext(intent: string) {
    const terms = contextSearchTerms(intent);
    if (
      workspaceSource === null
      || terms.length === 0
      || contextSuggestionInFlightRef.current
    ) {
      if (terms.length === 0) {
        setAttachNotice("Add a more specific task description before auto-selecting context.");
      }
      return;
    }
    const source = workspaceSource;
    const authorityKey = workspaceAuthorityKey;
    const generation = ++contextSuggestionGenerationRef.current;
    contextSuggestionInFlightRef.current = true;
    setContextSuggestionBusy(true);
    setAttachNotice(null);
    try {
      const resultGroups: WorkspaceSearchMatch[][] = [];
      for (const term of terms) {
        resultGroups.push(await source.search(term, 20));
        if (
          generation !== contextSuggestionGenerationRef.current
          || authorityKey !== workspaceAuthorityKeyRef.current
        ) {
          return;
        }
      }
      const relativePaths = rankContextPaths(resultGroups, 8);
      if (relativePaths.length === 0) {
        setAttachNotice("Forge found no local files matching this task. Pin files from Files instead.");
        return;
      }
      const projection = await source.previewContext(relativePaths);
      if (
        generation !== contextSuggestionGenerationRef.current
        || authorityKey !== workspaceAuthorityKeyRef.current
      ) {
        return;
      }
      setContextPreview(projection);
      setContextSelectionMode("automatic");
      setAttachNotice(
        `Auto-selected ${projection.items.length} local ${projection.items.length === 1 ? "file" : "files"} for review.`,
      );
    } catch (error) {
      if (generation === contextSuggestionGenerationRef.current) {
        setAttachNotice(getSafeHostMessage(error));
      }
    } finally {
      // `contextSuggestionInFlightRef` guards against a second concurrent
      // suggestion, so no other suggestContext can be running when this one
      // settles. Reset it (and the spinner) unconditionally: a concurrent
      // browseAndAttach/clearContext may have advanced the generation without
      // clearing the in-flight flag, which would otherwise strand auto-select.
      contextSuggestionInFlightRef.current = false;
      setContextSuggestionBusy(false);
    }
  }

  function clearContext() {
    contextSuggestionGenerationRef.current += 1;
    contextSuggestionInFlightRef.current = false;
    setContextSuggestionBusy(false);
    setContextPreview(null);
    setContextSelectionMode("manual");
    setAttachNotice(null);
  }
  const explorerAvailabilityMessage = (() => {
    switch (hostRuntime.kind) {
      case "loading":
        return "Forge is still connecting to the Windows host.";
      case "read_only_recovery":
        return "The local authority store is in recovery. Existing workspace names remain visible, but file projections are blocked.";
      case "unavailable":
        return hostRuntime.message;
      case "ready":
        return activeWorkspace
          ? "The host did not project the complete D1 read capability set."
          : "Choose a local workspace before opening Explorer.";
      case "browser_demo":
        return "Browser demo data is unavailable.";
    }
  })();

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

  const loadMethodLibrary = useCallback(async (client: DesktopHostClient) => {
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
        setHostWorkspaces(bootstrap.workspaces);
        setActiveWorkspaceId((current) => current !== null
          && bootstrap.workspaces.some(({ workspaceId }) => workspaceId === current)
          ? current
          : bootstrap.workspaces[0]?.workspaceId ?? null);
        if (bootstrap.bootMode !== "ready") {
          setHostRuntime({ kind: "read_only_recovery", client, bootstrap });
          return;
        }
        setHostRuntime({ kind: "ready", client, bootstrap });
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
        hostBindingGenerationRef.current += 1;
        bmadLibraryRequestedRef.current = false;
        bmadProjectionGenerationRef.current += 1;
        setBmadLibraryState({ kind: "idle" });
        contextSuggestionGenerationRef.current += 1;
        contextSuggestionInFlightRef.current = false;
        setContextSuggestionBusy(false);
        setContextPreview(null);
        setContextSelectionMode("manual");
        setHostRuntime((current) => current.kind === "ready" && current.client === client
          ? {
            kind: "unavailable",
            client: null,
            bootstrap: null,
            message: "Forge could not reach the Windows host. Local actions remain unavailable.",
          }
          : current);
        return;
      }
      setBmadLibraryState({
        kind: "unavailable",
        message: getSafeHostMessage(error),
        retryable: error instanceof HostCommandError && error.details.retryable,
      });
    }
  }, []);

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
    setChangesHistory(null);
    setChangesHistoryStatus("unread");
    setChangesHistoryBusy(false);
    setEnableEditsBusy(false);
    recoveryGenerationRef.current += 1;
    recoveryDecisionRef.current = false;
    recoveryPreparationRef.current = false;
    setRecoveryReview(null);
    setRecoveryBusy(false);
    setRecoveryReturnFocusTarget(null);
    contextSuggestionGenerationRef.current += 1;
    contextSuggestionInFlightRef.current = false;
    setContextSuggestionBusy(false);
    setContextPreview(null);
    setContextSelectionMode("manual");
    setContextDrawer(null);
  }, [clearAllChangesFlows, clearAllChatLanes]);

  const invalidateHostBoundUi = useCallback(() => {
    invalidateWorkspaceBoundUi();
    clearMethodLibraryProjection();
    clearBmadHelpProjection();
    setPersonaPerspectives(new Map());
    clearAllCapabilityRuns();
    setAboutProjection(null);
    setAboutStatus("unavailable");
    setOffboardingManifest(null);
    setOffboardingStatus("unavailable");
    setOffboardingNotice(null);
    setModelSelectionBusy(false);
    setModelSelectionNotice(null);
    setPreferencesNotice(null);
    setAttachNotice(null);
    workspaceActionBusyRef.current = false;
    setWorkspaceSelectionBusy(false);
    setWorkspaceRemovalBusyId(null);
    setWorkspaceActionError(null);
  }, [
    clearAllCapabilityRuns,
    clearBmadHelpProjection,
    clearMethodLibraryProjection,
    invalidateWorkspaceBoundUi,
  ]);

  const adoptHostRuntime = useCallback((
    runtime: HostRuntime,
    preserveActiveWorkspace: boolean,
  ) => {
    hostBindingGenerationRef.current += 1;
    invalidateHostBoundUi();
    setHostRuntime(runtime);
    const workspaces = runtime.kind === "browser_demo"
      ? [browserDemoWorkspace]
      : runtime.kind === "ready" || runtime.kind === "read_only_recovery"
        ? runtime.bootstrap.workspaces
        : [];
    setHostWorkspaces(workspaces);
    setActiveWorkspaceId((current) => (
      preserveActiveWorkspace
      && current !== null
      && workspaces.some(({ workspaceId }) => workspaceId === current)
        ? current
        : workspaces[0]?.workspaceId ?? null
    ));
  }, [invalidateHostBoundUi]);

  // `useCallback`, not a plain declaration, because both markers are captured
  // by long-lived effects — the projection poller and the method-guidance
  // recovery effect. A declaration is a new function on every render, so it can
  // never be an honest dependency: naming it would restart the poller on every
  // render, and omitting it (which is what the code did) leaves the poller
  // holding the closure from whichever render happened to mount it. That
  // closure reads `invalidateHostBoundUi` from a stale render, so the transition
  // into read-only recovery could invalidate the wrong host binding's UI.
  const markReadOnlyRecovery = useCallback((
    client: Extract<HostRuntime, { kind: "ready" | "read_only_recovery" }>["client"],
    sequence: number,
  ) => {
    hostBindingGenerationRef.current += 1;
    invalidateHostBoundUi();
    setHostRuntime((current) => {
      if (
        (current.kind !== "ready" && current.kind !== "read_only_recovery")
        || current.client !== client
      ) {
        return current;
      }
      return {
        kind: "read_only_recovery",
        client,
        bootstrap: {
          ...current.bootstrap,
          bootMode: "read_only_recovery",
          supportedCommands: current.bootstrap.supportedCommands.filter(
            (command) => command === "app.get_boot_state" || command === "workspace.list",
          ),
          projectionSequence: Math.max(current.bootstrap.projectionSequence, sequence),
        },
      };
    });
    setWorkspaceActionError(
      "Forge entered read-only recovery. Workspace access was not changed.",
    );
  }, [invalidateHostBoundUi]);

  /// Stabilised for the same reason as `markReadOnlyRecovery` above.
  const markHostUnavailable = useCallback((
    client: Extract<HostRuntime, { kind: "ready" | "read_only_recovery" }>["client"],
  ) => {
    hostBindingGenerationRef.current += 1;
    invalidateHostBoundUi();
    setHostRuntime((current) => {
      if (
        (current.kind !== "ready" && current.kind !== "read_only_recovery")
        || current.client !== client
      ) {
        return current;
      }
      return {
        kind: "unavailable",
        client: null,
        bootstrap: null,
        message: "Forge could not reach the Windows host. Local actions remain unavailable.",
      };
    });
  }, [invalidateHostBoundUi]);

  function dismissWorkspacePanel() {
    setAppModal(null);
    const returnFocus = workspaceReturnFocusRef.current;
    workspaceReturnFocusRef.current = null;
    window.requestAnimationFrame(() => returnFocus?.focus());
  }

  function openUtilityPanel(
    mode: "account" | "settings",
    settingsPage: SettingsSection = "general",
    returnFocusTarget: HTMLElement | null = null,
  ) {
    const activeElement = document.activeElement;
    utilityReturnFocusRef.current = returnFocusTarget
      ?? (activeElement instanceof HTMLElement
        && activeElement !== document.body
        && activeElement !== document.documentElement
        ? activeElement
        : null);
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
      setAboutStatus("unavailable");
      return;
    }
    const hostBindingGeneration = hostBindingGenerationRef.current;
    setAboutStatus("loading");
    try {
      const about = await hostRuntime.client.getAbout();
      if (hostBindingGeneration !== hostBindingGenerationRef.current) return;
      setAboutProjection(about);
      setAboutStatus("ready");
    } catch {
      if (hostBindingGeneration !== hostBindingGenerationRef.current) return;
      setAboutStatus("unavailable");
    }
    if (hostBindingGeneration !== hostBindingGenerationRef.current) return;
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

  function capabilityWorkspace(): { workspaceId: string; grantEpoch: number } | null {
    if (hostRuntime.kind !== "ready" || activeWorkspace === null) return null;
    return {
      workspaceId: activeWorkspace.workspaceId,
      grantEpoch: activeWorkspace.grantEpoch,
    };
  }

  function failCapabilityRun(routing: CapabilityRouting, error: unknown) {
    capabilitySessions.routeCapabilityRun(routing, (current) =>
      current === null
        ? null
        : { ...current, phase: { kind: "error", message: getSafeHostMessage(error) } },
    );
  }

  async function prepareCapabilityRun(contextPaths: readonly string[]) {
    const run = capabilityRun;
    const workspace = capabilityWorkspace();
    if (run === null || workspace === null || hostRuntime.kind !== "ready") return;
    const routing = capabilitySessions.captureActive();
    capabilitySessions.setActiveCapabilityRun({ ...run, phase: { kind: "preparing" } });
    try {
      const review = await hostRuntime.client.prepareCapabilityRun(
        workspace.workspaceId,
        workspace.grantEpoch,
        run.capabilityId,
        [...contextPaths],
      );
      capabilitySessions.routeCapabilityRun(routing, (current) =>
        current === null
          ? null
          : { ...current, phase: { kind: "review", review, contextPaths } },
      );
    } catch (error) {
      failCapabilityRun(routing, error);
    }
  }

  async function approveCapabilityRun(manifestHash: string) {
    const run = capabilityRun;
    const workspace = capabilityWorkspace();
    if (
      run === null
      || workspace === null
      || hostRuntime.kind !== "ready"
      || run.phase.kind !== "review"
    ) {
      return;
    }
    const routing = capabilitySessions.captureActive();
    const reviewPhase = run.phase;
    try {
      const approved = await hostRuntime.client.approveCapabilityRun(
        workspace.workspaceId,
        workspace.grantEpoch,
        run.capabilityId,
        manifestHash,
      );
      capabilitySessions.routeCapabilityRun(routing, (current) =>
        current === null
          ? null
          : {
              ...current,
              phase: {
                kind: "approved",
                review: reviewPhase.review,
                approved,
                contextPaths: reviewPhase.contextPaths,
                sending: false,
              },
            },
      );
    } catch (error) {
      failCapabilityRun(routing, error);
    }
  }

  async function cancelCapabilityRun(manifestHash: string, decisionId: string) {
    const run = capabilityRun;
    const workspace = capabilityWorkspace();
    if (run === null || workspace === null || hostRuntime.kind !== "ready") return;
    const routing = capabilitySessions.captureActive();
    try {
      await hostRuntime.client.cancelCapabilityRun(
        workspace.workspaceId,
        workspace.grantEpoch,
        run.capabilityId,
        manifestHash,
        decisionId,
      );
      capabilitySessions.routeCapabilityRun(routing, null);
    } catch (error) {
      failCapabilityRun(routing, error);
    }
  }

  async function submitCapabilityRun(manifestHash: string, decisionId: string) {
    const run = capabilityRun;
    const workspace = capabilityWorkspace();
    if (
      run === null
      || workspace === null
      || hostRuntime.kind !== "ready"
      || run.phase.kind !== "approved"
    ) {
      return;
    }
    const routing = capabilitySessions.captureActive();
    capabilitySessions.setActiveCapabilityRun({
      ...run,
      phase: { ...run.phase, sending: true },
    });
    try {
      const completed = await hostRuntime.client.submitCapabilityRun(
        workspace.workspaceId,
        workspace.grantEpoch,
        run.capabilityId,
        manifestHash,
        decisionId,
      );
      let resultJson: string | null = null;
      try {
        const latest = await hostRuntime.client.latestCapabilityRun(
          workspace.workspaceId,
          workspace.grantEpoch,
          run.capabilityId,
        );
        resultJson = latestResultForRun(latest, completed.runId);
      } catch {
        resultJson = null;
      }
      capabilitySessions.routeCapabilityRun(routing, (current) =>
        current === null
          ? null
          : { ...current, phase: { kind: "completed", completed, resultJson } },
      );
    } catch (error) {
      failCapabilityRun(routing, error);
    }
  }

  async function loadRetentionManifest() {
    if (offboardingStatus === "erased") return;
    if (
      hostRuntime.kind !== "ready"
      || !hostRuntime.bootstrap.supportedCommands.includes("app.offboarding.inspect")
    ) {
      setOffboardingStatus("unavailable");
      return;
    }
    // Same guard as `loadAboutProjection`: this is the second half of the same
    // await chain, so leaving it unguarded would keep the hole open — the
    // retention manifest of a revoked workspace's host would still land.
    const hostBindingGeneration = hostBindingGenerationRef.current;
    setOffboardingStatus("loading");
    try {
      const manifest = await hostRuntime.client.inspectOffboarding();
      if (hostBindingGeneration !== hostBindingGenerationRef.current) return;
      setOffboardingManifest(manifest);
      setOffboardingStatus("ready");
    } catch {
      if (hostBindingGeneration !== hostBindingGenerationRef.current) return;
      setOffboardingStatus("unavailable");
    }
  }

  async function eraseOffboardingData(confirm: string) {
    if (
      hostRuntime.kind !== "ready"
      || !hostRuntime.bootstrap.supportedCommands.includes("app.offboarding.erase")
    ) {
      setOffboardingNotice("Erasing local data is unavailable in this mode.");
      return;
    }
    setOffboardingStatus("erasing");
    setOffboardingNotice(null);
    try {
      await hostRuntime.client.eraseOffboarding(confirm);
      setOffboardingManifest(null);
      setOffboardingStatus("erased");
    } catch (error) {
      setOffboardingStatus("ready");
      setOffboardingNotice(getSafeHostMessage(error));
    }
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

  async function changeModelSelection(selection: ModelSelection) {
    if (
      hostRuntime.kind !== "ready"
      || !hostRuntime.bootstrap.supportedCommands.includes("model.selection.set")
    ) {
      setModelSelectionNotice("Model selection requires the Forge desktop host.");
      return;
    }
    setModelSelectionBusy(true);
    setModelSelectionNotice(null);
    try {
      const status = await hostRuntime.client.setModelSelection(
        selection.providerId,
        selection.modelId,
      );
      bmadHelpGenerationRef.current += 1;
      bmadHelpCreationRef.current = null;
      bmadHelpOperationRef.current = null;
      bmadHelpStateRef.current = initialBmadRequestState;
      setBmadHelpState(initialBmadRequestState);
      setCurrentBmadIntent(null);
      clearAllCapabilityRuns();
      setModelAuthStatus(status);
      setModelSelectionNotice(
        status.status === "development_ready"
          ? `${status.destinationLabel} is ready.`
          : `${status.destinationLabel} is selected. Enter its API key below to connect.`,
      );
    } catch (error) {
      setModelSelectionNotice(getSafeHostMessage(error));
    } finally {
      setModelSelectionBusy(false);
    }
  }

  function resetModelBoundUi(status: ModelAuthStatusProjection) {
    bmadHelpGenerationRef.current += 1;
    bmadHelpCreationRef.current = null;
    bmadHelpOperationRef.current = null;
    bmadHelpStateRef.current = initialBmadRequestState;
    setBmadHelpState(initialBmadRequestState);
    setCurrentBmadIntent(null);
    clearAllCapabilityRuns();
    setModelAuthStatus(status);
  }

  async function connectModelProvider(connection: ModelProviderConnection) {
    if (
      hostRuntime.kind !== "ready"
      || !hostRuntime.bootstrap.supportedCommands.includes("model.auth.sign_in")
    ) {
      setModelSelectionNotice("Provider connection requires the Forge desktop host.");
      return;
    }
    setModelSelectionBusy(true);
    setModelSelectionNotice(null);
    try {
      const status = await hostRuntime.client.modelAuthSignIn(connection);
      resetModelBoundUi(status);
      setModelSelectionNotice(`${status.destinationLabel} is connected for this session.`);
    } catch (error) {
      setModelSelectionNotice(getSafeHostMessage(error));
    } finally {
      setModelSelectionBusy(false);
    }
  }

  async function disconnectModelProvider() {
    if (
      hostRuntime.kind !== "ready"
      || !hostRuntime.bootstrap.supportedCommands.includes("model.auth.sign_out")
    ) {
      setModelSelectionNotice("Provider disconnection requires the Forge desktop host.");
      return;
    }
    setModelSelectionBusy(true);
    setModelSelectionNotice(null);
    try {
      const status = await hostRuntime.client.modelAuthSignOut();
      resetModelBoundUi(status);
      setModelSelectionNotice("Provider disconnected. The in-memory API key was cleared.");
    } catch (error) {
      setModelSelectionNotice(getSafeHostMessage(error));
    } finally {
      setModelSelectionBusy(false);
    }
  }

  function dismissUtilityPanel(restoreFocus = true) {
    pendingUtilityReturnFocusRef.current = restoreFocus ? utilityReturnFocusRef.current : null;
    setAppModal(null);
    utilityReturnFocusRef.current = null;
  }

  function openContextDrawer(
    kind: Exclude<ContextDrawerKind, null>,
    returnFocusTarget: HTMLElement | null = null,
    focusDrawerOnOpen = false,
  ) {
    const activeElement = document.activeElement;
    contextDrawerReturnFocusRef.current = returnFocusTarget
      ?? (activeElement instanceof HTMLElement
        && activeElement !== document.body
        && activeElement !== document.documentElement
        ? activeElement
        : null);
    focusContextDrawerOnOpenRef.current = focusDrawerOnOpen;
    setContextDrawer(kind);
  }

  function dismissContextDrawer(restoreFocus = true) {
    pendingContextDrawerReturnFocusRef.current = restoreFocus
      ? contextDrawerReturnFocusRef.current
      : null;
    contextDrawerReturnFocusRef.current = null;
    focusContextDrawerOnOpenRef.current = false;
    setContextDrawer(null);
  }

  useEffect(() => {
    if (appModal !== null) return;
    const returnFocus = pendingUtilityReturnFocusRef.current;
    pendingUtilityReturnFocusRef.current = null;
    if (returnFocus?.isConnected && !returnFocus.closest("[inert]")) {
      returnFocus.focus();
    }
  }, [appModal]);

  useEffect(() => {
    if (contextDrawer !== null) {
      if (focusContextDrawerOnOpenRef.current) {
        focusContextDrawerOnOpenRef.current = false;
        document.querySelector<HTMLElement>(".context-drawer__close")?.focus();
      }
      return;
    }
    const returnFocus = pendingContextDrawerReturnFocusRef.current;
    pendingContextDrawerReturnFocusRef.current = null;
    if (returnFocus?.isConnected && !returnFocus.closest("[inert]")) {
      returnFocus.focus();
    }
  }, [contextDrawer]);

  useEffect(() => {
    let isCurrent = true;
    void hostRuntimeLoader()
      .then((runtime) => {
        if (!isCurrent) {
          return;
        }
        adoptHostRuntime(runtime, false);
      })
      .catch(() => {
        if (!isCurrent) {
          return;
        }
        adoptHostRuntime({
          kind: "unavailable",
          client: null,
          bootstrap: null,
          message: "Forge could not reach the Windows host. Local actions remain unavailable.",
        }, false);
      });
    return () => {
      isCurrent = false;
    };
  }, [adoptHostRuntime, hostRuntimeAttempt, hostRuntimeLoader]);

  /// The way back out of `unavailable`. Clearing the memoised default runtime
  /// is what makes the retry real: without it the loader would hand back the
  /// same settled failure forever.
  const retryHostRuntime = useCallback(() => {
    resetDefaultHostRuntime();
    setHostRuntime({ kind: "loading" });
    setHostRuntimeAttempt((attempt) => attempt + 1);
  }, []);

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
    void loadMethodLibrary(methodLibraryClient);
  }, [
    activeWorkspaceId,
    clearMethodLibraryProjection,
    loadMethodLibrary,
    methodLibraryClient,
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
        if (
          error instanceof HostCommandError
          && (error.details.code === "recovery_required" || error.details.code === "integrity_failure")
        ) {
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

  useEffect(() => {
    if (hostRuntime.kind !== "ready" && hostRuntime.kind !== "read_only_recovery") {
      return;
    }
    const { bootstrap, client } = hostRuntime;
    const hostBindingGeneration = hostBindingGenerationRef.current;
    let cancelled = false;
    let cursor = bootstrap.projectionSequence;
    let timerId: number | undefined;
    const isStale = () => cancelled
      || hostBindingGeneration !== hostBindingGenerationRef.current;
    const handleProjectionFailure = (error: unknown) => {
      if (
        error instanceof HostCommandError
        && (error.details.code === "recovery_required" || error.details.code === "integrity_failure")
      ) {
        markReadOnlyRecovery(client, cursor);
      } else if (!(error instanceof HostCommandError && error.details.retryable)) {
        markHostUnavailable(client);
      }
    };
    const refreshWorkspaceProjection = async () => {
      const workspaces = await client.listWorkspaces();
      if (isStale()) {
        return false;
      }
      setHostWorkspaces(workspaces);
      setActiveWorkspaceId((current) => current !== null
        && workspaces.some(({ workspaceId }) => workspaceId === current)
        ? current
        : workspaces[0]?.workspaceId ?? null);
      return true;
    };

    const poll = async () => {
      try {
        const events = await client.projectionEvents(cursor);
        if (isStale()) {
          return;
        }
        if (events.length > 0) {
          cursor = events.at(-1)!.sequence;
        }
        let refreshWorkspaces = false;
        let refreshMethodLibrary = false;
        let enterReadOnlyRecovery = false;
        for (const { event } of events) {
          if (event.type === "boot_state_changed" && event.projection.mode === "read_only_recovery") {
            enterReadOnlyRecovery = true;
          }
          if (event.type === "workspace_changed") {
            refreshWorkspaces = true;
          }
          if (event.type === "bmad.projection_changed") {
            refreshMethodLibrary = true;
          }
        }
        if (refreshWorkspaces) {
          if (!(await refreshWorkspaceProjection())) {
            return;
          }
        }
        if (enterReadOnlyRecovery) {
          markReadOnlyRecovery(client, cursor);
          return;
        }
        if (refreshMethodLibrary && bmadLibraryRequestedRef.current) {
          await loadMethodLibrary(client);
        }
      } catch (error) {
        if (isStale()) {
          return;
        }
        if (
          error instanceof HostCommandError
          && error.details.code === "conflict"
        ) {
          try {
            const snapshot = await client.projectionSnapshot();
            if (isStale()) {
              return;
            }
            if (snapshot.bootMode === "read_only_recovery") {
              cursor = snapshot.sequence;
              markReadOnlyRecovery(client, cursor);
              return;
            }
            // The snapshot is intentionally summary-only. A cursor gap can
            // hide session, approval, execution, evidence, connectivity, and
            // update events, so establish a fresh renderer authority instead
            // of advancing past projections this renderer cannot reconstruct.
            let reboundBootstrap: Awaited<ReturnType<DesktopHostClient["bootstrap"]>>;
            try {
              reboundBootstrap = await client.bootstrap();
            } catch {
              if (!isStale()) {
                markHostUnavailable(client);
              }
              return;
            }
            if (isStale()) {
              return;
            }
            const reloadMethodLibrary = bmadLibraryRequestedRef.current
              && reboundBootstrap.bootMode === "ready"
              && reboundBootstrap.supportedCommands.includes("bmad.library.snapshot");
            adoptHostRuntime({
              kind: reboundBootstrap.bootMode === "ready"
                ? "ready"
                : "read_only_recovery",
              client,
              bootstrap: reboundBootstrap,
            }, true);
            if (reloadMethodLibrary) {
              void loadMethodLibrary(client);
            }
          } catch (snapshotError) {
            if (!isStale()) {
              handleProjectionFailure(snapshotError);
            }
          }
        } else {
          handleProjectionFailure(error);
        }
      } finally {
        if (!isStale()) {
          timerId = window.setTimeout(() => void poll(), projectionPollIntervalMs);
        }
      }
    };

    timerId = window.setTimeout(() => void poll(), projectionPollIntervalMs);
    return () => {
      cancelled = true;
      if (timerId !== undefined) {
        window.clearTimeout(timerId);
      }
    };
  }, [
    adoptHostRuntime,
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

  function selectSession(id: string) {
    if (findTask(tasks, id) === null) {
      return;
    }
    if (id !== selectedSessionId) {
      // Attached context is scoped to the message about to be sent, and the
      // projection is app-global. Without this, files pinned in one task stayed
      // pinned after switching and were sent with the next task's message —
      // files the user never attached there.
      clearContext();
    }
    setPrimaryRoute({ kind: "task", taskId: id });
    setContextDrawer(null);
  }

  function startNewSession(intent?: string, preserveIntentAsDraft = false): string | null {
    const currentRequest = bmadHelpStateRef.current;
    if (
      currentRequest.kind === "creating"
      || currentRequest.kind === "approving"
      || currentRequest.kind === "approved"
      || currentRequest.kind === "submitting"
    ) return null;
    // The id becomes the host's `client_session_ref`, from which the durable
    // chat session identity is derived. It must never repeat — see
    // `createTaskId` — or a fresh task silently adopts an older task's
    // persisted transcript, including one the user deleted.
    const session = createTask(createTaskId(), intent);
    bmadHelpGenerationRef.current += 1;
    bmadHelpCreationRef.current = null;
    bmadHelpOperationRef.current = null;
    bmadHelpStateRef.current = initialBmadRequestState;
    setBmadHelpState(initialBmadRequestState);
    setCurrentBmadIntent(null);
    clearContext();
    if (preserveIntentAsDraft && intent?.trim()) {
      setTaskDrafts((current) => {
        const next = new Map(current);
        next.set(session.id, intent.trim());
        return next;
      });
    }
    setTasks((current) => appendTask(current, session));
    setPrimaryRoute({ kind: "task", taskId: session.id });
    setContextDrawer(null);
    // The new task starts with no capability run of its own; other tasks keep
    // theirs (per-task isolation), so no global capability reset here.
    return session.id;
  }

  /// Names a task from the first message sent in it.
  ///
  /// Most tasks never get named by hand, so the opening message is the only
  /// honest label available. `autoTitleTask` is a no-op once the task has any
  /// title, which is what keeps a name the user typed from being overwritten.
  function autoTitleFromMessage(taskId: string, message: string) {
    setTasks((current) => autoTitleTask(current, taskId, message));
  }

  function renameSelectedTask(title: string) {
    if (routedTaskId === null) return;
    setTasks((current) => renameTask(current, routedTaskId, title));
  }

  /// Drops the transient run state a task owns, leaving its transcript alone.
  ///
  /// Shared by close and delete. A capability run and a governed-changes flow
  /// are in-progress work, not a record of what was said, so neither survives
  /// the task leaving the open list; `closeCapabilitySession` also makes a late
  /// result for that task drop instead of reappearing.
  function releaseTaskRunState(taskId: string) {
    capabilitySessions.closeCapabilitySession(taskId);
    setChangesFlowForTask(taskId, null);
  }

  function dropTaskDraft(taskId: string) {
    setTaskDrafts((current) => {
      if (!current.has(taskId)) return current;
      const next = new Map(current);
      next.delete(taskId);
      return next;
    });
  }

  /// Moves the route off a task that is leaving the open list.
  function reselectAfterRemoval(taskId: string) {
    const next = nextSelectionAfter(tasks, taskId, routedTaskId);
    if (routedTaskId !== taskId) {
      return;
    }
    clearContext();
    setContextDrawer(null);
    // No phantom task to land on: with nothing open, Home *is* the empty
    // state.
    setPrimaryRoute(next === null ? { kind: "home" } : { kind: "task", taskId: next });
  }

  /// Closes a task: it leaves the tab strip but keeps its transcript and its
  /// unsent draft, so reopening it from Home restores the conversation.
  /// Non-destructive, hence no confirmation.
  function closeTaskById(taskId: string) {
    if (findTask(tasks, taskId) === null) return;
    reselectAfterRemoval(taskId);
    releaseTaskRunState(taskId);
    setTasks((current) => closeTask(current, taskId));
  }

  function reopenTaskById(taskId: string) {
    if (findTask(tasks, taskId) === null) return;
    setTasks((current) => reopenTask(current, taskId));
    clearContext();
    setContextDrawer(null);
    setPrimaryRoute({ kind: "task", taskId });
  }

  function requestTaskDeletion(taskId: string) {
    if (findTask(tasks, taskId) === null) return;
    workspaceReturnFocusRef.current = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    setPendingDeleteTaskId(taskId);
    setContextDrawer(null);
    setAppModal("delete-task");
  }

  function dismissTaskDeletion() {
    setPendingDeleteTaskId(null);
    setAppModal(null);
    const returnFocus = workspaceReturnFocusRef.current;
    workspaceReturnFocusRef.current = null;
    window.requestAnimationFrame(() => {
      if (returnFocus?.isConnected && !returnFocus.closest("[inert]")) {
        returnFocus.focus();
      }
    });
  }

  /// Destroys a task. Irreversible from Forge's side, which is why it is only
  /// reachable through the confirmation dialog.
  ///
  /// This drops the renderer's record and its conversation lane. The host's
  /// durable transcript is *not* deleted: the command catalog exposes no
  /// per-session chat delete, and the store (`crates/desktop-store/src/chat.rs`)
  /// offers no removal at all. Because task ids are never reused, the orphaned
  /// row can no longer be addressed by any task; erasing it from disk is
  /// Settings → Local data. `DeleteTaskDialog` says exactly this rather than
  /// promising an erase that does not happen.
  function confirmTaskDeletion() {
    const taskId = pendingDeleteTaskId;
    setPendingDeleteTaskId(null);
    setAppModal(null);
    if (taskId === null || findTask(tasks, taskId) === null) return;
    reselectAfterRemoval(taskId);
    releaseTaskRunState(taskId);
    dropTaskDraft(taskId);
    // Discards the conversation and supersedes it, so a reply already in
    // flight is dropped rather than resurrecting the transcript.
    chatSessions.dropSession(taskId);
    setTasks((current) => deleteTask(current, taskId));
    const returnFocus = workspaceReturnFocusRef.current;
    workspaceReturnFocusRef.current = null;
    window.requestAnimationFrame(() => {
      if (returnFocus?.isConnected && !returnFocus.closest("[inert]")) {
        returnFocus.focus();
      }
    });
  }

  function openMethodLibrary() {
    if (!methodLibraryClient) {
      return;
    }
    const openingFromUtility = appModal !== null;
    const returnFocus = openingFromUtility ? utilityReturnFocusRef.current : null;
    if (openingFromUtility) {
      dismissUtilityPanel(false);
    }
    openContextDrawer("skills", returnFocus, openingFromUtility);
    if (!bmadLibraryRequestedRef.current) {
      void loadMethodLibrary(methodLibraryClient);
    }
  }

  async function approveAndSubmitBmadRequest(
    prepared: Extract<BmadRequestState, { readonly kind: "review_required" }>,
  ): Promise<void> {
    if (
      !methodGuidanceClient
      || !activeWorkspace
      || bmadHelpOperationRef.current !== null
    ) {
      throw new HostCapabilityError("The message can no longer be sent.");
    }
    const authority = currentBmadAuthority(prepared.run.runId);
    if (!authority || !bmadRequestAuthorityIsCurrent(prepared, authority)) {
      applyBmadEvent({ type: "authority_invalidated", reason: "authority_changed" });
      throw new HostCapabilityError("The message authority changed. Send it again.");
    }

    const client = methodGuidanceClient;
    const workspace = activeWorkspace;
    const generation = authority.rendererGeneration;
    bmadHelpOperationRef.current = "approve";
    applyBmadEvent({ type: "approve_started" });
    try {
      const approval = await client.approveBmadHelp(
        workspace.workspaceId,
        workspace.grantEpoch,
        prepared.authority.manifestHash,
      );
      const approving = bmadHelpStateRef.current;
      const approvingAuthority = currentBmadAuthority(prepared.run.runId);
      if (
        generation !== bmadHelpGenerationRef.current
        || approving.kind !== "approving"
        || !approvingAuthority
        || !bmadRequestAuthorityIsCurrent(approving, approvingAuthority)
      ) {
        throw new HostCapabilityError("The message authority changed before sending.");
      }
      const approved = applyBmadEvent({ type: "approved", approval });
      if (approved.kind !== "approved") {
        throw new HostCapabilityError("The message approval did not match this request.");
      }

      bmadHelpOperationRef.current = "submit";
      applyBmadEvent({ type: "submit_started" });
      const result = await client.submitBmadHelp(
        workspace.workspaceId,
        workspace.grantEpoch,
        approved.authority.manifestHash,
        approved.approval.decisionId,
      );
      if (
        generation !== bmadHelpGenerationRef.current
        || bmadHelpStateRef.current.kind !== "submitting"
      ) {
        throw new HostCapabilityError("The completed response no longer belongs to this task.");
      }
      applyBmadEvent({ type: "completed", result });
    } catch (error: unknown) {
      const current = bmadHelpStateRef.current;
      if (
        generation === bmadHelpGenerationRef.current
        && (
          current.kind === "approving"
          || current.kind === "approved"
          || current.kind === "submitting"
        )
      ) {
        applyBmadEvent({ type: "unavailable", message: getSafeHostMessage(error) });
      }
      throw error;
    } finally {
      bmadHelpOperationRef.current = null;
    }
  }

  /// The composer's default action. An ordinary conversation turn: no BMAD
  /// concept is required to say something. When the task has an agent bound,
  /// that agent's sealed persona travels with the request, which is how the
  /// method is practised here without a separate LLM runtime.
  async function sendChatMessage(intent: string, sessionId?: string): Promise<void> {
    const client = methodGuidanceClient;
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
    // With an agent bound, a message that unambiguously names one of that
    // agent's menu items dispatches it instead — the behaviour the method's
    // own activation describes. Anything else stays an ordinary conversation.
    const boundAgent = chatLane.agentCode;
    if (boundAgent !== null && bmadLibraryState.kind === "ready") {
      const agent = bmadLibraryState.projection.methodAgents.find(
        (candidate) => candidate.agentCode === boundAgent,
      );
      const dispatched = agent ? matchMenuIntent(intent, agent.menus) : null;
      if (dispatched?.menu.capabilityId != null) {
        // Record the exchange before leaving. A dispatch is still a turn the
        // user took: returning straight to the capability run cleared the
        // composer and dropped the message, so the conversation lost the very
        // sentence that caused the run to open.
        const dispatchRouting = sessionId === undefined
          ? chatSessions.captureRouting()
          : chatSessions.routingFor(sessionId);
        const dispatchedAt = Date.now();
        const label = dispatched.menu.displayLabel;
        chatSessions.applyToLane(dispatchRouting, (lane) =>
          appendDispatchExchange(lane, intent, label, dispatchedAt),
        );
        openCapabilityRun(dispatched.menu.capabilityId, label);
        return;
      }
    }
    // The Home composer creates the task and sends in the same tick, so the
    // active-session state has not settled yet. Binding to the id the caller
    // just created keeps the exchange on the task that asked for it.
    const routing = sessionId === undefined
      ? chatSessions.captureRouting()
      : chatSessions.routingFor(sessionId);
    const contextPaths = contextPreview?.items.map(({ relativePath }) => relativePath) ?? [];
    const agentCode = chatLane.agentCode;
    chatSessions.applyToLane(routing, (lane) => ({
      ...lane,
      sending: true,
      pending: intent,
      error: null,
    }));
    try {
      const turn = await client.sendChatMessage({
        workspaceId: workspace.workspaceId,
        workspaceGrantEpoch: workspace.grantEpoch,
        clientSessionRef: routing.sessionId,
        agentCode,
        text: intent,
        contextPaths,
      });
      chatSessions.applyToLane(routing, (lane) => ({
        ...lane,
        messages: [...lane.messages, turn.userMessage, turn.assistantMessage],
        agentCode: turn.agentCode,
        sending: false,
        pending: null,
        error: null,
        retryable: false,
      }));
      // Attachments are spent by the send that carried them. Leaving them armed
      // meant the next message silently re-sent, re-billed and re-entered every
      // one of them into the reviewed-context manifest — consent given once,
      // acted on every turn, with nothing on screen distinguishing "these went
      // last time" from "these go again".
      //
      // Ordering matters: `clearContext` sets `attachNotice` to null, so the
      // notice has to be written after it or it is erased immediately.
      if (contextPaths.length > 0) {
        const count = contextPaths.length;
        clearContext();
        setAttachNotice(
          `Sent ${count} attached ${count === 1 ? "file" : "files"} with this message. `
          + `Attach again to include ${count === 1 ? "it" : "them"} in the next one.`,
        );
      }
    } catch (error: unknown) {
      // `pending` deliberately survives. Clearing it here destroyed what the
      // user had typed, so a failed turn cost them the message as well as the
      // answer — and the host can now tell them a rate limit or a provider
      // outage is worth sending again, which is useless advice if the text is
      // gone. The composer stays usable because it is gated on `sending`.
      chatSessions.applyToLane(routing, (lane) => ({
        ...lane,
        sending: false,
        error: getSafeHostMessage(error),
        retryable: error instanceof HostCommandError && error.details.retryable,
      }));
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

  function sendBmadMessage(currentIntent: string): Promise<void> {
    if (bmadHelpCreationRef.current) {
      return bmadHelpCreationRef.current;
    }
    if (
      !methodRequestAvailable
      || !methodGuidanceClient
      || !activeWorkspace
      || !modelAuthStatus
      || modelAuthStatus.status !== "development_ready"
      || bmadHelpOperationRef.current !== null
    ) {
      return Promise.reject(
        new HostCapabilityError("Skill guidance is unavailable for the active workspace grant."),
      );
    }

    const client = methodGuidanceClient;
    const workspace = activeWorkspace;
    const contextPaths = contextPreview?.items.map(({ relativePath }) => relativePath) ?? [];
    const projectionSequence = hostRuntime.kind === "ready"
      ? hostRuntime.bootstrap.projectionSequence
      : 0;
    const generation = ++bmadHelpGenerationRef.current;
    // Asking BMAD Help is still the task's opening message, so it names the
    // task the same way an ordinary chat turn does.
    autoTitleFromMessage(selectedSessionId, currentIntent);
    setCurrentBmadIntent({ createdAt: Date.now(), text: currentIntent });
    applyBmadEvent({ type: "create_started" });

    const creation = (async () => {
      try {
        const run = await client.createBmadHelpRun(
          workspace.workspaceId,
          workspace.grantEpoch,
          currentIntent,
        );
        if (generation !== bmadHelpGenerationRef.current) {
          throw new HostCapabilityError(
            "The skill-guidance result no longer belongs to the active workspace grant.",
          );
        }
        const review = await client.prepareBmadHelp(
          workspace.workspaceId,
          workspace.grantEpoch,
          contextPaths,
        );
        if (
          generation !== bmadHelpGenerationRef.current
          || modelAuthStatus.status !== "development_ready"
        ) {
          throw new HostCapabilityError(
            "The model connection changed before the message could be sent.",
          );
        }
        const next = applyBmadEvent({
          type: "review_ready",
          run,
          review,
          authority: {
            workspaceId: workspace.workspaceId,
            workspaceGrantEpoch: workspace.grantEpoch,
            runId: run.runId,
            authEpoch: modelAuthStatus.authEpoch,
            rendererGeneration: generation,
            now: Date.now(),
          },
        });
        if (next.kind !== "review_required") {
          throw new HostCapabilityError("The prepared skill-guidance request failed closed.");
        }
        await approveAndSubmitBmadRequest(next);
      } catch (error: unknown) {
        if (generation === bmadHelpGenerationRef.current) {
          if (
            error instanceof HostCommandError
            && (error.details.code === "recovery_required" || error.details.code === "integrity_failure")
          ) {
            markReadOnlyRecovery(client, projectionSequence);
          } else {
            applyBmadEvent({ type: "unavailable", message: getSafeHostMessage(error) });
          }
        }
        throw error;
      }
    })().finally(() => {
      if (bmadHelpCreationRef.current === creation) {
        bmadHelpCreationRef.current = null;
      }
    });
    bmadHelpCreationRef.current = creation;
    return creation;
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
  const changesHistoryAvailable = governedEditsCommandsAvailable && activeWorkspace !== null;

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
    const demoCreatedAt = hostRuntime.kind === "browser_demo" ? 1_784_731_394_657 : undefined;
    if (demoCreatedAt !== undefined && currentBmadIntent === null) {
      const userId = "msg_forge_demo_user";
      const assistantId = "msg_forge_demo_assistant";
      messages.push({
        id: userId,
        role: "user",
        createdAt: demoCreatedAt,
        value: {
          message: {
            id: userId,
            sessionID: sessionId,
            role: "user",
            time: { created: demoCreatedAt },
          },
          agent: "bmad",
          model: {
            modelID: "forge-bmad-preview",
            providerID: "forge",
          },
          summary: {
            title: "Review the next BMAD milestone",
            diffs: [],
          },
          parts: [{
            id: "part_forge_demo_intent",
            type: "text",
            text: "Review this workspace and identify the next BMAD milestone.",
          }],
        },
      });
      messages.push({
        id: assistantId,
        role: "assistant",
        createdAt: demoCreatedAt + 8_412,
        value: {
          message: {
            id: assistantId,
            sessionID: sessionId,
            role: "assistant",
            time: { created: demoCreatedAt + 1_205, completed: demoCreatedAt + 8_412 },
          },
          agent: "bmad",
          model: {
            modelID: "forge-bmad-preview",
            providerID: "forge",
          },
          parts: [{
            id: "part_forge_demo_result",
            type: "text",
            text: "Continue the governed Help request path, then review the exact context before model access.",
          }],
          bmad: {
            status: "browser_demo",
            authority: "read_only",
            recommendation: "Governed BMAD Help",
          },
        },
      });
    }
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
        ?? (bmadHelpState.kind === "completed" ? bmadHelpState.result.receipt.startedAt : demoCreatedAt),
      inputTokens: reviewState?.review.totalTokenEstimate
        ?? contextPreview?.estimatedTokens
        ?? (demoCreatedAt === undefined ? undefined : 1_842),
      messages,
    };
  }, [bmadHelpState, contextPreview?.estimatedTokens, currentBmadIntent, hostRuntime.kind, selectedSessionId, sessionContextModelId]);

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

  /// Stable identity: the expiry effect below has to be able to name it.
  /// It touches only refs and state setters, so it has no dependencies.
  const clearRecoveryReview = useCallback(() => {
    recoveryGenerationRef.current += 1;
    recoveryDecisionRef.current = false;
    recoveryPreparationRef.current = false;
    setRecoveryReview(null);
    setRecoveryBusy(false);
    setRecoveryReturnFocusTarget(null);
  }, []);

  useEffect(() => {
    if (recoveryReview === null) return;
    let timeout: number | null = null;
    const clearAtExpiry = () => {
      const remaining = recoveryReview.expiresAt - Date.now();
      if (remaining <= 0) {
        if (recoveryDecisionRef.current) return;
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
  }, [clearRecoveryReview, recoveryReview]);

  async function refreshChangesHistory() {
    if (recoveryDecisionRef.current) {
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
    setChangesHistoryStatus("loading");
    setChangesError(null);
    try {
      const history = await client.changesHistory(workspace.workspaceId, workspace.grantEpoch);
      if (
        generation === changesGenerationRef.current
        && workspaceAuthorityKeyRef.current === authorityKey
        && hostRuntime.kind === "ready"
      ) {
        setChangesHistory(history);
        setChangesHistoryStatus("loaded");
      }
    } catch (error) {
      setChangesHistoryStatus("unavailable");
      if (
        generation === changesGenerationRef.current
        && workspaceAuthorityKeyRef.current === authorityKey
      ) {
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

  async function prepareChangesRecovery(journalId: string, trigger: HTMLElement) {
    if (
      hostRuntime.kind !== "ready"
      || !activeWorkspace
      || !editsEnabled
      || recoveryBusy
      || recoveryReview !== null
      || recoveryPreparationRef.current
      || !changesHistory?.openJournals.some(
        (journal) => journal.journalId === journalId
          && journal.recoveryAvailability === "review_available",
      )
    ) {
      return;
    }
    const client = hostRuntime.client;
    const workspace = activeWorkspace;
    const sequence = hostRuntime.bootstrap.projectionSequence;
    const authorityKey = workspaceAuthorityKey;
    const generation = ++recoveryGenerationRef.current;
    recoveryPreparationRef.current = true;
    recoveryDecisionRef.current = false;
    setRecoveryReview(null);
    setRecoveryReturnFocusTarget(trigger);
    setRecoveryBusy(true);
    setChangesError(null);
    try {
      const prepared = await client.prepareChangesRecovery({
        workspaceId: workspace.workspaceId,
        workspaceGrantEpoch: workspace.grantEpoch,
        journalId,
      });
      if (
        generation !== recoveryGenerationRef.current
        || workspaceAuthorityKeyRef.current !== authorityKey
      ) {
        return;
      }
      setRecoveryBusy(false);
      recoveryPreparationRef.current = false;
      if (prepared.status === "review_required") {
        setRecoveryReview(prepared);
        return;
      }
      setRecoveryReview(null);
      setRecoveryReturnFocusTarget(null);
      if (prepared.status === "manual_review") {
        const manualReviewMessage = recoveryManualReviewMessage(prepared.reasonCode);
        await refreshChangesHistory();
        if (workspaceAuthorityKeyRef.current === authorityKey) {
          setChangesError(manualReviewMessage);
        }
        return;
      }
      await refreshChangesHistory();
    } catch (error) {
      if (generation === recoveryGenerationRef.current) {
        setRecoveryReview(null);
        setRecoveryBusy(false);
        setRecoveryReturnFocusTarget(null);
        recoveryPreparationRef.current = false;
        handleChangesFailure(client, error, sequence);
      }
    }
  }

  async function decideChangesRecovery(choice: RecoveryApprovalChoice) {
    if (
      hostRuntime.kind !== "ready"
      || recoveryReview === null
      || recoveryBusy
      || recoveryDecisionRef.current
    ) {
      return;
    }
    const client = hostRuntime.client;
    const sequence = hostRuntime.bootstrap.projectionSequence;
    const review = recoveryReview;
    const generation = recoveryGenerationRef.current;
    recoveryDecisionRef.current = true;
    setRecoveryBusy(true);
    setChangesError(null);
    try {
      await client.decideChangesRecovery({
        recoveryApprovalId: review.recoveryApprovalId,
        displayedRecoveryHash: review.displayedRecoveryHash,
        choice,
      });
      if (generation !== recoveryGenerationRef.current) return;
      setRecoveryReview(null);
      setRecoveryBusy(false);
      recoveryDecisionRef.current = false;
      await refreshChangesHistory();
    } catch (error) {
      if (generation === recoveryGenerationRef.current) {
        setRecoveryReview(null);
        setRecoveryBusy(false);
        setRecoveryReturnFocusTarget(null);
        recoveryDecisionRef.current = false;
        handleChangesFailure(client, error, sequence);
      }
    }
  }

  function handleChangesFailure(client: DesktopHostClient, error: unknown, sequence: number) {
    if (
      error instanceof HostCommandError
      && (error.details.code === "recovery_required" || error.details.code === "integrity_failure")
    ) {
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
      setHostWorkspaces((current) => [
        enabled,
        ...current.filter(({ workspaceId }) => workspaceId !== enabled.workspaceId),
      ]);
      setActiveWorkspaceId(enabled.workspaceId);
      clearAllChangesFlows();
      setChangesHistory(null);
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

  async function proposeCapabilityChangeSet(capabilityId: string) {
    openContextDrawer("changes");
    if (hostRuntime.kind !== "ready" || !activeWorkspace || !editsEnabled) {
      return;
    }
    const client = hostRuntime.client;
    const workspace = activeWorkspace;
    const sequence = hostRuntime.bootstrap.projectionSequence;
    const originTask = selectedSessionId;
    const generation = ++changesGenerationRef.current;
    setChangesFlowForTask(originTask, { kind: "preparing" });
    setChangesError(null);
    try {
      const review = await client.proposeCapabilityChanges(
        workspace.workspaceId,
        workspace.grantEpoch,
        capabilityId,
      );
      if (generation === changesGenerationRef.current) {
        setChangesFlowForTask(originTask, { kind: "review", busy: false, review });
      }
    } catch (error) {
      if (generation === changesGenerationRef.current) {
        setChangesFlowForTask(originTask, { kind: "idle" });
        handleChangesFailure(client, error, sequence);
      }
    }
  }

  async function decideGovernedChange(choice: ApprovalChoice) {
    if (hostRuntime.kind !== "ready" || changesState.kind !== "review") {
      return;
    }
    const client = hostRuntime.client;
    const sequence = hostRuntime.bootstrap.projectionSequence;
    const review = changesState.review;
    const originTask = selectedSessionId;
    const generation = ++changesGenerationRef.current;
    setChangesFlowForTask(originTask, { kind: "review", busy: true, review });
    setChangesError(null);
    try {
      const decision = await client.decideApproval(review, choice);
      if (generation !== changesGenerationRef.current) {
        return;
      }
      if (decision.disposition === "applied" && decision.execution) {
        setChangesFlowForTask(originTask, {
          kind: "applied",
          busy: false,
          execution: decision.execution,
        });
        setChangesHistory(null);
      } else if (decision.disposition === "discarded") {
        setChangesFlowForTask(originTask, { kind: "discarded" });
      } else {
        setChangesFlowForTask(originTask, { kind: "idle" });
      }
    } catch (error) {
      if (generation === changesGenerationRef.current) {
        // The host consumes a pending proposal on any decision attempt, so a
        // failed apply requires a fresh review.
        setChangesFlowForTask(originTask, { kind: "idle" });
        handleChangesFailure(client, error, sequence);
      }
    }
  }

  async function undoGovernedChange(executionId: string) {
    if (hostRuntime.kind !== "ready" || !activeWorkspace || !editsEnabled) {
      return;
    }
    const client = hostRuntime.client;
    const sequence = hostRuntime.bootstrap.projectionSequence;
    const originTask = selectedSessionId;
    const generation = ++changesGenerationRef.current;
    setChangesFlowForTask(
      originTask,
      changesState.kind === "applied"
        ? { kind: "applied", busy: true, execution: changesState.execution }
        : { kind: "preparing" },
    );
    setChangesError(null);
    try {
      const result = await client.requestRollback(executionId);
      if (generation !== changesGenerationRef.current) {
        return;
      }
      if (result.kind === "review") {
        setChangesFlowForTask(originTask, { kind: "review", busy: false, review: result.value });
      } else {
        setChangesFlowForTask(originTask, { kind: "undo_unavailable", value: result.value });
      }
    } catch (error) {
      if (generation === changesGenerationRef.current) {
        setChangesFlowForTask(originTask, { kind: "idle" });
        handleChangesFailure(client, error, sequence);
      }
    }
  }

  async function selectWorkspace() {
    if (hostRuntime.kind !== "ready" || !canSelectWorkspace || workspaceActionBusyRef.current) {
      return;
    }
    workspaceActionBusyRef.current = true;
    setWorkspaceSelectionBusy(true);
    setWorkspaceActionError(null);
    try {
      const selection = await hostRuntime.client.selectWorkspace();
      if (selection.kind === "workspace_selected") {
        setHostWorkspaces((current) => [
          selection.value,
          ...current.filter(({ workspaceId }) => workspaceId !== selection.value.workspaceId),
        ]);
        setActiveWorkspaceId(selection.value.workspaceId);
      }
    } catch (error) {
      if (
        error instanceof HostCommandError
        && (error.details.code === "recovery_required" || error.details.code === "integrity_failure")
      ) {
        markReadOnlyRecovery(
          hostRuntime.client,
          hostRuntime.bootstrap.projectionSequence,
        );
      } else {
        setWorkspaceActionError(getSafeHostMessage(error));
      }
    } finally {
      workspaceActionBusyRef.current = false;
      setWorkspaceSelectionBusy(false);
    }
  }

  function activateWorkspace(workspaceId: string) {
    if (
      hostRuntime.kind !== "ready"
      || !canActivateWorkspace
      || workspaceActionBusyRef.current
      || !hostWorkspaces.some((workspace) => workspace.workspaceId === workspaceId)
    ) {
      return;
    }
    setWorkspaceActionError(null);
    if (workspaceId !== activeWorkspaceId) {
      resetAllChangesFlows();
      setChangesHistory(null);
      clearAllCapabilityRuns();
    }
    setActiveWorkspaceId(workspaceId);
  }

  async function removeWorkspace(workspaceId: string) {
    if (
      hostRuntime.kind !== "ready"
      || !canRemoveWorkspace
      || workspaceActionBusyRef.current
    ) {
      return;
    }
    const expectedWorkspace = hostWorkspaces.find(
      (workspace) => workspace.workspaceId === workspaceId,
    );
    if (!expectedWorkspace) {
      return;
    }

    workspaceActionBusyRef.current = true;
    setWorkspaceRemovalBusyId(workspaceId);
    setWorkspaceActionError(null);
    try {
      const result = await hostRuntime.client.revokeWorkspace(expectedWorkspace);
      setHostWorkspaces(result.workspaces);
      setActiveWorkspaceId((current) => {
        if (
          current !== workspaceId
          && current !== null
          && result.workspaces.some((workspace) => workspace.workspaceId === current)
        ) {
          return current;
        }
        return result.workspaces[0]?.workspaceId ?? null;
      });
    } catch (error) {
      if (
        error instanceof HostCommandError
        && (error.details.code === "recovery_required" || error.details.code === "integrity_failure")
      ) {
        markReadOnlyRecovery(
          hostRuntime.client,
          hostRuntime.bootstrap.projectionSequence,
        );
      } else {
        setWorkspaceActionError(getSafeHostMessage(error));
      }
    } finally {
      workspaceActionBusyRef.current = false;
      setWorkspaceRemovalBusyId(null);
    }
  }

  function openWorkspaceManager() {
    workspaceReturnFocusRef.current = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    setContextDrawer(null);
    setAppModal("workspace-manager");
  }

  function openWorkspaceManagerFromUtilityPanel() {
    const returnFocus = utilityReturnFocusRef.current;
    utilityReturnFocusRef.current = null;
    workspaceReturnFocusRef.current = returnFocus;
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

  const drawer = contextDrawer ? (
    <ContextDrawer
      kind={contextDrawer}
      onClose={dismissContextDrawer}
      onSelectTab={(tab) => setContextDrawer(tab)}
      presentation={drawerIsOverlay ? "overlay" : "pane"}
    >
      {contextDrawer === "files" ? (
        <WorkspaceExplorer
          asPanel
          availabilityMessage={explorerAvailabilityMessage}
          key={`${workspaceSource?.provenance ?? "unavailable"}:${activeWorkspace?.workspaceId ?? "none"}:${activeWorkspace?.grantEpoch ?? 0}`}
          onContextReview={(projection) => {
            contextSuggestionGenerationRef.current += 1;
            contextSuggestionInFlightRef.current = false;
            setContextSuggestionBusy(false);
            setContextPreview(projection);
            setContextSelectionMode("manual");
            dismissContextDrawer();
          }}
          source={workspaceSource}
          workspaceName={workspaceName}
        />
      ) : contextDrawer === "changes" ? (
        <ReviewPanel
          availabilityMessage={gitReviewAvailabilityMessage}
          governedReview={(
            <GovernedChangesPanel
              canEnableEdits={canEnableEdits}
              enableEditsBusy={enableEditsBusy}
              errorMessage={changesError}
              history={changesHistory}
              historyBusy={changesHistoryBusy}
              onDecide={(choice) => void decideGovernedChange(choice)}
              onEnableEdits={() => void enableGovernedEdits()}
              onDecideRecovery={(choice) => void decideChangesRecovery(choice)}
              onPrepareRecovery={(journalId, trigger) => void prepareChangesRecovery(journalId, trigger)}
              onRefreshHistory={() => void refreshChangesHistory()}
              onStartNewProposal={resetChangesFlow}
              onUndo={(executionId) => void undoGovernedChange(executionId)}
              recoveryBusy={recoveryBusy}
              recoveryReturnFocusTarget={recoveryReturnFocusTarget}
              recoveryReview={recoveryReview}
              state={changesState}
            />
          )}
          proposalAttention={changesState.kind === "review" || recoveryReview !== null}
          source={gitReviewSource}
        />
      ) : contextDrawer === "context" ? (
        <SessionContextPanel
          createdAt={sessionContext.createdAt}
          inputTokens={sessionContext.inputTokens}
          localReviewBudget={65_536}
          messages={sessionContext.messages}
          model={sessionContextModelId}
          provider={modelProviderLabel}
          sessionTitle={selectedTask?.title ?? "No task selected"}
        />
      ) : contextDrawer === "skills" ? (
        <div className="method-library-panel">
          {capabilityRun === null ? null : (
            <BmadCapabilityPanel
              capabilityId={capabilityRun.capabilityId}
              capabilityLabel={capabilityRun.label}
              destinationLabel={modelAccess.label}
              onApprove={(manifestHash) => void approveCapabilityRun(manifestHash)}
              onCancel={(manifestHash, decisionId) =>
                void cancelCapabilityRun(manifestHash, decisionId)}
              onClose={() => capabilitySessions.setActiveCapabilityRun(null)}
              onPrepare={(contextPaths) => void prepareCapabilityRun(contextPaths)}
              onReviewChangeSet={() =>
                void proposeCapabilityChangeSet(capabilityRun.capabilityId)}
              onSubmit={(manifestHash, decisionId) =>
                void submitCapabilityRun(manifestHash, decisionId)}
              phase={capabilityRun.phase}
            />
          )}
          <BmadWorkflowPanel
            error={workflowState.error}
            loading={workflowState.loading}
            onRefresh={() => void refreshWorkflowState()}
            state={workflowState.state}
          />
          <BmadLibraryPanel
            onRunBuilderCapability={openCapabilityRun}
            onRunCapability={openCapabilityRun}
            onReload={() => {
              if (methodLibraryClient) {
                void loadMethodLibrary(methodLibraryClient);
              }
            }}
            onViewPersona={(agentCode) => {
              void viewPersonaPerspective(agentCode);
            }}
            personaPerspectives={personaPerspectives}
            state={bmadLibraryState}
          />
        </div>
      ) : (
        <ActivityPanel
          helpState={bmadHelpState}
          history={changesHistory}
          historyAvailable={changesHistoryAvailable}
          historyError={changesError}
          historyStatus={changesHistoryStatus}
          historyBusy={changesHistoryBusy}
          onRefreshHistory={() => void refreshChangesHistory()}
          onDecideRecovery={(choice) => void decideChangesRecovery(choice)}
          onPrepareRecovery={(journalId, trigger) => void prepareChangesRecovery(journalId, trigger)}
          onUndo={(executionId) => void undoGovernedChange(executionId)}
          recoveryBusy={recoveryBusy}
          recoveryReturnFocusTarget={recoveryReturnFocusTarget}
          recoveryReview={recoveryReview}
        />
      )}
    </ContextDrawer>
  ) : undefined;

  const modal = appModal === "workspace-manager" ? (
    <WorkspacePanel
      activeWorkspaceId={activeWorkspace?.workspaceId ?? null}
      busyWorkspaceId={workspaceRemovalBusyId}
      canActivate={canActivateWorkspace}
      canRemove={canRemoveWorkspace}
      canSelect={canSelectWorkspace}
      isSelecting={workspaceSelectionBusy}
      mode={hostRuntime.kind}
      onActivate={activateWorkspace}
      onClose={dismissWorkspacePanel}
      onRemove={(workspaceId) => void removeWorkspace(workspaceId)}
      onSelect={() => void selectWorkspace()}
      workspaceError={workspaceActionError}
      workspaces={hostWorkspaces}
    />
  ) : appModal === "settings" ? (
    <SettingsDialog
      about={aboutProjection}
      aboutStatus={aboutStatus}
      agentStatusLabel={methodStatusLabel}
      density={density}
      initialSection={utilitySettingsPage}
      key={`${appModal}:${utilitySettingsPage}`}
      modelAccessDetail={modelAccess.detail}
      modelAccessLabel={modelAccess.label}
      modelAuthStatus={modelAuthStatus}
      modelSelection={modelSelection}
      modelSelectionBusy={modelSelectionBusy}
      modelSelectionNotice={modelSelectionNotice}
      offboardingManifest={offboardingManifest}
      offboardingNotice={offboardingNotice}
      offboardingStatus={offboardingStatus}
      onClose={() => dismissUtilityPanel()}
      onDensityChange={changeDensity}
      onEraseOffboarding={(confirm) => void eraseOffboardingData(confirm)}
      onManageWorkspaces={openWorkspaceManagerFromUtilityPanel}
      onModelConnect={connectModelProvider}
      onModelDisconnect={disconnectModelProvider}
      onModelSelectionChange={changeModelSelection}
      onOpenSkillsAndAgents={openMethodLibrary}
      onThemeChange={changeTheme}
      preferencesNotice={preferencesNotice}
      runtimeLabel={hostStatusLabel}
      skillsAgentsAvailable={methodLibraryAvailable}
      skillsAgentsStatusLabel={skillsAgentsStatusLabel}
      theme={theme}
      updateStatusLabel={
        aboutProjection === null
          ? "Managed by your organization"
          : aboutProjection.updateConfigured
            ? "Managed release channel"
            : "Not configured for this build"
      }
      workspaceDetail={workspaceDescription}
      workspaceLabel={workspaceName}
    />
  ) : appModal === "delete-task" && pendingDeleteTask !== null ? (
    <DeleteTaskDialog
      historyStatus={chatSessions.historyStatusFor(pendingDeleteTask.id)}
      // A browser preview has no host and no store, so nothing was ever
      // written. A task proven to hold no messages never reached `chat.send`,
      // so no durable row exists for it either. Anything else — including a
      // history read that has not finished or failed — might have one, and the
      // warning stands.
      mayHaveDurableCopy={
        hostRuntime.kind !== "browser_demo"
        && !(
          chatSessions.historyStatusFor(pendingDeleteTask.id) === "loaded"
          && chatSessions.messageCountFor(pendingDeleteTask.id) === 0
        )
      }
      messageCount={chatSessions.messageCountFor(pendingDeleteTask.id)}
      onCancel={dismissTaskDeletion}
      onConfirm={confirmTaskDeletion}
      taskTitle={pendingDeleteTask.title}
    />
  ) : undefined;

  // The palette shortcut has to be a real document listener, not a React
  // handler on the shell. React attaches at the root container, so a keydown
  // whose target is <body> — the state on first paint, and again every time
  // the palette or a disabled composer blurs — never reached the shell and
  // Ctrl+K silently did nothing.
  useEffect(() => {
    function onShortcut(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen((current) => !current);
      }
    }
    document.addEventListener("keydown", onShortcut);
    return () => document.removeEventListener("keydown", onShortcut);
  }, []);

  const paletteActions: readonly PaletteAction[] = [
    { id: "new-task", label: "New task", hint: "chat", run: () => { startNewSession(); } },
    // Task lifecycle actions appear only with a task open, so the palette
    // never offers to rename or delete something that does not exist.
    ...(selectedTask === null ? [] : [
      {
        id: "current-task",
        label: `Go to task: ${selectedTask.title}`,
        hint: "tab",
        run: () => selectSession(selectedTask.id),
      },
      {
        id: "close-task",
        label: `Close task: ${selectedTask.title}`,
        hint: "task",
        run: () => closeTaskById(selectedTask.id),
      },
      {
        id: "delete-task",
        label: `Delete task: ${selectedTask.title}`,
        hint: "task",
        run: () => requestTaskDeletion(selectedTask.id),
      },
    ] satisfies PaletteAction[]),
    {
      id: "open-workspace",
      label: "Open or switch workspace",
      hint: "workspace",
      run: openWorkspaceManager,
    },
    {
      id: "drawer-files",
      label: "Show workspace files",
      hint: "drawer",
      run: () => openContextDrawer("files"),
    },
    {
      id: "drawer-changes",
      label: "Show review",
      hint: "drawer",
      run: () => openContextDrawer("changes"),
    },
    {
      id: "drawer-activity",
      label: "Show run activity",
      hint: "drawer",
      run: () => openContextDrawer("activity"),
    },
    {
      id: "drawer-skills",
      label: "Show skills and agents",
      hint: "drawer",
      run: () => openContextDrawer("skills"),
    },
    {
      id: "settings",
      label: "Open settings",
      hint: "app",
      run: () => openUtilityPanel("settings"),
    },
  ];

  return (
    <div className="app-shell app-shell--forge-v2">
      <CommandPalette
        actions={paletteActions}
        onClose={() => setPaletteOpen(false)}
        open={paletteOpen}
      />
      <TitleBar
        isInert={shellOverlayOpen}
        onHome={() => {
          setContextDrawer(null);
          setPrimaryRoute({ kind: "home" });
        }}
        onMenu={() => setPaletteOpen(true)}
        onNewTask={() => { startNewSession(); }}
        onCloseTask={closeTaskById}
        onOpenSettings={() => openUtilityPanel("settings")}
        activeTaskId={routedTaskId ?? undefined}
        onSelectTask={selectSession}
        taskTabs={openTaskTabs}
      />
      {/*
        The palette is rendered outside `AppShellLayout`, so its own modal
        bookkeeping does not cover it. Marking the workbench inert is the other
        half of honouring `aria-modal` on the palette — `AppShellLayout` already
        does the equivalent for the drawer and the utility panels.
      */}
      <div
        aria-hidden={paletteOpen || undefined}
        className="workbench workbench--task-shell"
        inert={paletteOpen || undefined}
      >
        <AppShellLayout
          drawer={drawer}
          main={primaryRoute.kind === "home" || selectedTask === null ? (
            // No task selected *is* Home. There is no phantom task to fall
            // back onto any more, so an empty list and a deleted last task
            // both land here rather than on a tab that cannot be acted on.
            <HomeView
              activeWorkspaceId={activeWorkspaceId}
              canOpenFolder={canSelectWorkspace}
              composerDisabled={
                !localTaskDraftingAvailable
                || !activeWorkspace
                || methodRequestInFlight
              }
              onActivateWorkspace={activateWorkspace}
              workspaces={hostWorkspaces}
              onOpenWorkspace={() => void selectWorkspace()}
              onOpenWorkspaceManager={openWorkspaceManager}
              onNewTask={() => { startNewSession(); }}
              onSelectTask={selectSession}
              onSubmitIntent={(intent) => {
                const startedId = startNewSession(intent, !methodRequestAvailable);
                if (startedId !== null) {
                  if (methodRequestAvailable) {
                    // The Home composer opens an ordinary conversation, like
                    // any other chat app. BMAD joins it only if asked for.
                    void sendChatMessage(intent, startedId).catch(() => undefined);
                  }
                }
              }}
              recentTasks={recentTaskList}
              closedTasks={closedTasks}
              onReopenTask={reopenTaskById}
              onRetryHost={hostRuntime.kind === "unavailable" ? retryHostRuntime : undefined}
              statusHint={methodRequestAvailable
                ? "Ready to send through the selected model."
                : "Start locally, then connect OpenAI in Settings before sending."}
              workspaceStatusLabel={workspaceDescription}
            />
          ) : activeWorkspace ? (
            <TaskWorkspace
              canAttachFiles={canBrowseFiles || canBrowseFolder}
              contextPreview={contextPreview}
              contextSelectionMode={contextSelectionMode}
              contextSuggestionBusy={contextSuggestionBusy}
              hostStatusLabel={hostStatusLabel}
              initialDraft={taskDrafts.get(selectedTask.id) ?? ""}
              interactionDisabled={!localTaskDraftingAvailable}
              isBrowserDemo={hostRuntime.kind === "browser_demo"}
              isReadOnlyRecovery={hostRuntime.kind === "read_only_recovery"}
              key={`${selectedSessionId}:${methodGuidanceBindingKey}`}
              methodGuidanceAvailable={methodRequestAvailable}
              methodGuidanceState={bmadHelpState}
              methodLibraryAvailable={methodLibraryAvailable}
              modelAccessDetail={modelAccess.detail}
              modelAccessLabel={modelAccess.label}
              onAttachFiles={canBrowseFiles ? browseAndAttachFiles : undefined}
              onAttachFolder={canBrowseFolder ? browseAndAttachFolder : undefined}
              onClearContext={clearContext}
              attachNotice={attachNotice}
              agentLibrary={bmadLibraryState}
              onOpenChanges={() => {
                openContextDrawer("changes");
              }}
              onOpenContext={() => {
                openContextDrawer("context");
              }}
              onOpenMethodLibrary={openMethodLibrary}
              onOpenProviders={() => openUtilityPanel("settings", "agent-model")}
              onOpenWorkspaceManager={openWorkspaceManager}
              workspaceStatusLabel={workspaceDescription}
              onOpenRunDetails={() => {
                openContextDrawer("activity");
              }}
              onDraftChange={(draft) => {
                setTaskDrafts((current) => {
                  const next = new Map(current);
                  if (draft) {
                    next.set(selectedTask.id, draft);
                  } else {
                    next.delete(selectedTask.id);
                  }
                  return next;
                });
              }}
              onSuggestContext={suggestContext}
              onSendRequest={sendChatMessage}
              chatLane={chatLane}
              onRetryHistory={() => retryChatHistoryRestore(selectedTask.id)}
              onAskMethodGuidance={sendBmadMessage}
              onSelectAgent={(agentCode) => {
                chatSessions.setActiveLane((lane) => ({ ...lane, agentCode }));
              }}
              onRenameTask={renameSelectedTask}
              onCloseTask={() => closeTaskById(selectedTask.id)}
              onDeleteTask={() => requestTaskDeletion(selectedTask.id)}
              sessionTitle={selectedTask.title}
              workspaceName={workspaceName}
            />
          ) : (
            <main className="task-shell-empty">
              <NoWorkspaceState
                copy={explorerAvailabilityMessage}
                mode={hostRuntime.kind}
                onOpenWorkspace={() => void selectWorkspace()}
                onRetryHost={hostRuntime.kind === "unavailable" ? retryHostRuntime : undefined}
              />
            </main>
          )}
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
        />
      </div>
    </div>
  );
}
