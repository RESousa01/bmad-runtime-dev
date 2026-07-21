import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { BmadHelpCard } from "./components/BmadHelpCard";
import {
  BmadLibraryPanel,
  type BmadPersonaPerspectiveView,
} from "./components/BmadLibraryPanel";
import { GovernedChangesPanel } from "./components/GovernedChangesPanel";
import { TaskWorkspace } from "./components/TaskWorkspace";
import { TitleBar } from "./components/TitleBar";
import {
  BmadCapabilityPanel,
  type CapabilityRunPhase,
} from "./components/BmadCapabilityPanel";
import {
  type OffboardingStatus,
  SettingsDialog,
  type SettingsSection,
} from "./components/settings/SettingsDialog";
import "./components/settings/settings.css";
import { WorkspacePanel } from "./components/WorkspacePanel";
import { WorkspaceExplorer } from "./components/WorkspaceExplorer";
import { CommandPalette, type PaletteAction } from "./components/CommandPalette";
import {
  AppShellLayout,
  DRAWER_OVERLAY_QUERY,
  SIDEBAR_OVERLAY_QUERY,
} from "./components/redesign/AppShellLayout";
import { AppSidebar } from "./components/redesign/AppSidebar";
import {
  ContextDrawer,
  type ContextDrawerKind,
} from "./components/redesign/ContextDrawer";
import { NoWorkspaceState } from "./components/redesign/NoWorkspaceState";
import { ActivityPanel } from "./components/panels/ActivityPanel";
import "./components/panels/activity-panel.css";
import "./components/redesign/app-shell.css";
import "./components/redesign/context-drawer.css";
import "./components/redesign/sidebar.css";
import "./components/redesign/task-surface.css";
import "./components/redesign/utility-panel.css";
import "./components/redesign/design-polish.css";
import "./components/redesign/design-refinement.css";
import {
  type DensityPreference,
  type SessionSummary,
  type ThemePreference,
} from "./data/demo";
import {
  getDefaultHostRuntime,
  getSafeHostMessage,
  type AboutProjection,
  type RetentionManifestProjection,
  type ApprovalChoice,
  type DesktopHostClient,
  HostCapabilityError,
  HostCommandError,
  type ContextPreviewProjection,
  type ChangesHistoryProjection,
  type ChangesRecoveryPrepared,
  type HostRuntime,
  type RecoveryApprovalChoice,
  type WorkspaceProjection,
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
} from "./lib/bmadModelProjection";
import {
  browserDemoWorkspaceSource,
  createHostWorkspaceSource,
  type WorkspaceProjectionProvenance,
} from "./lib/workspaceReadSource";
import { useMediaQuery } from "./lib/useMediaQuery";

const initialProductSession: SessionSummary = {
  id: "new-0",
  title: "New task",
  updatedAt: "Now",
};

const browserDemoWorkspace: WorkspaceProjection = {
  workspaceId: "workspace_browser_demo",
  projectId: "project_browser_demo",
  displayName: "bmad-runtime-dev",
  grantEpoch: 0,
  permissions: "read_only",
  contextReadEpoch: 1,
  governedEditEpoch: 1,
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

export type PrimaryRoute = { kind: "task"; taskId: string | null };
export type AppModalKind = "workspace-manager" | "settings" | "account" | null;

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


// The reviewed 26 roster menu paths (ADR-0005 closure ledger): every
// (agentCode, menuCode) pair binds exactly one capability identity.
const MENU_CAPABILITIES: Readonly<Record<string, string>> = {
  "bmad-agent-analyst/BP": "bmm:bmad-brainstorming",
  "bmad-agent-analyst/MR": "bmm:bmad-market-research",
  "bmad-agent-analyst/DR": "bmm:bmad-domain-research",
  "bmad-agent-analyst/TR": "bmm:bmad-technical-research",
  "bmad-agent-analyst/CB": "bmm:bmad-product-brief",
  "bmad-agent-analyst/WB": "bmm:bmad-prfaq",
  "bmad-agent-analyst/DP": "bmm:bmad-document-project",
  "bmad-agent-tech-writer/DP": "bmm:bmad-document-project",
  "bmad-agent-tech-writer/WD": "bmm:tech-writer-write-document",
  "bmad-agent-tech-writer/MG": "bmm:tech-writer-mermaid-gen",
  "bmad-agent-tech-writer/VD": "bmm:tech-writer-validate-doc",
  "bmad-agent-tech-writer/EC": "bmm:tech-writer-explain-concept",
  "bmad-agent-pm/PRD": "bmm:bmad-prd",
  "bmad-agent-pm/CE": "bmm:bmad-create-epics-and-stories",
  "bmad-agent-pm/IR": "bmm:bmad-check-implementation-readiness",
  "bmad-agent-pm/CC": "bmm:bmad-correct-course",
  "bmad-agent-ux-designer/CU": "bmm:bmad-ux",
  "bmad-agent-architect/CA": "bmm:bmad-architecture",
  "bmad-agent-architect/IR": "bmm:bmad-check-implementation-readiness",
  "bmad-agent-dev/DS": "bmm:bmad-dev-story",
  "bmad-agent-dev/QD": "bmm:bmad-quick-dev",
  "bmad-agent-dev/QA": "bmm:bmad-qa-generate-e2e-tests",
  "bmad-agent-dev/CR": "bmm:bmad-code-review",
  "bmad-agent-dev/SP": "bmm:bmad-sprint-planning",
  "bmad-agent-dev/CS": "bmm:bmad-create-story",
  "bmad-agent-dev/ER": "bmm:bmad-retrospective",
};

interface ActiveCapabilityRunState {
  readonly capabilityId: string;
  readonly label: string;
  readonly phase: CapabilityRunPhase;
}

export function App({
  hostRuntimeLoader = getDefaultHostRuntime,
  projectionPollIntervalMs = 1_500,
}: AppProps = {}) {
  const [primaryRoute, setPrimaryRoute] = useState<PrimaryRoute>({
    kind: "task",
    taskId: initialProductSession.id,
  });
  const [density, setDensity] = useState<DensityPreference>("comfortable");
  const [sessions, setSessions] = useState<SessionSummary[]>([initialProductSession]);
  const [contextDrawer, setContextDrawer] = useState<ContextDrawerKind | null>(null);
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false);
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
  const [capabilityRun, setCapabilityRun] = useState<ActiveCapabilityRunState | null>(null);
  const [attachNotice, setAttachNotice] = useState<string | null>(null);
  const [hostRuntime, setHostRuntime] = useState<HostUiRuntime>({ kind: "loading" });
  const [hostWorkspaces, setHostWorkspaces] = useState<WorkspaceProjection[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string | null>(null);
  const [workspaceSelectionBusy, setWorkspaceSelectionBusy] = useState(false);
  const [workspaceRemovalBusyId, setWorkspaceRemovalBusyId] = useState<string | null>(null);
  const [workspaceActionError, setWorkspaceActionError] = useState<string | null>(null);
  const [contextPreview, setContextPreview] = useState<ContextPreviewProjection | null>(null);
  const [, setContextProvenance] = useState<WorkspaceProjectionProvenance | null>(null);
  const [bmadLibraryState, setBmadLibraryState] = useState<BmadLibraryUiState>({ kind: "idle" });
  const [personaPerspectives, setPersonaPerspectives] = useState<
    ReadonlyMap<string, BmadPersonaPerspectiveView>
  >(new Map());
  const [bmadHelpState, setBmadHelpState] = useState<BmadRequestState>(initialBmadRequestState);
  const [modelAuthStatus, setModelAuthStatus] = useState<ModelAuthStatusProjection | null>(null);
  const [changesFlow, setChangesFlow] = useState<GovernedChangesUiState | null>(null);
  const [changesError, setChangesError] = useState<string | null>(null);
  const [changesHistory, setChangesHistory] = useState<ChangesHistoryProjection | null>(null);
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
  const sidebarIsOverlay = useMediaQuery(SIDEBAR_OVERLAY_QUERY);
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
  const sessionSequenceRef = useRef(0);

  const selectedSessionId = primaryRoute.taskId ?? initialProductSession.id;
  const selectedSession = useMemo(
    () => sessions.find((session) => session.id === selectedSessionId) ?? initialProductSession,
    [selectedSessionId, sessions],
  );
  const isNewSession = selectedSession.id.startsWith("new-");
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
  const methodRequestAvailable = methodGuidanceClient !== null
    && modelAuthStatus?.status === "development_ready";
  const methodRequestInFlight = bmadHelpState.kind === "creating"
    || bmadHelpState.kind === "review_required"
    || bmadHelpState.kind === "approving"
    || bmadHelpState.kind === "approved"
    || bmadHelpState.kind === "submitting";
  const modelAccess = (() => {
    if (hostRuntime.kind === "browser_demo") {
      return {
        detail: "No local folder, consent grant, or model request is active.",
        label: "No model access",
      };
    }
    if (modelAuthStatus?.status === "development_ready") {
      return {
        detail: `${modelAuthStatus.mode === "deterministic_development" ? "Deterministic development" : "Offline"} · review required`,
        label: modelAuthStatus.destinationLabel || "Development model",
      };
    }
    if (hostRuntime.kind === "loading") {
      return { detail: "Waiting for the native desktop host.", label: "Checking access" };
    }
    if (methodGuidanceClient !== null && modelAuthStatus === null) {
      return { detail: "The desktop host is verifying the current model authorization.", label: "Checking model access" };
    }
    return {
      detail: methodLibraryAvailable
        ? "The host catalog can be opened without creating a model request."
        : "No model request or skills catalog capability is active.",
      label: "Model access unavailable",
    };
  })();
  const methodStatusLabel = methodRequestAvailable
    ? "Model ready"
    : methodGuidanceAvailable
      ? "Local only"
    : hostRuntime.kind === "browser_demo"
      ? "Demo only"
      : "Unavailable";
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
  const canBrowseFiles = hostRuntime.kind === "ready"
    && activeWorkspace !== null
    && workspaceSource !== null
    && hostRuntime.bootstrap.supportedCommands.includes("workspace.pick_files");

  async function browseAndAttachFiles() {
    if (hostRuntime.kind !== "ready" || !activeWorkspace || !workspaceSource) {
      return;
    }
    setAttachNotice(null);
    try {
      const pick = await hostRuntime.client.pickWorkspaceFiles(
        activeWorkspace.workspaceId,
      );
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
      const projection = await workspaceSource.previewContext(
        pick.value.relativePaths,
      );
      setContextPreview(projection);
      setContextProvenance(workspaceSource.provenance);
      const skipped = pick.value.rejectedOutsideRoot + pick.value.rejectedUnreadable;
      setAttachNotice(
        skipped > 0
          ? `Attached ${pick.value.selectedCount} files. ${skipped} were outside the workspace and skipped.`
          : null,
      );
    } catch (error) {
      setAttachNotice(getSafeHostMessage(error));
    }
  }
  const explorerAvailabilityMessage = (() => {
    switch (hostRuntime.kind) {
      case "loading":
        return "The signed Windows host is still being verified.";
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
        setContextPreview(null);
        setContextProvenance(null);
        setHostRuntime((current) => current.kind === "ready" && current.client === client
          ? {
            kind: "unavailable",
            client: null,
            bootstrap: null,
            message: "The signed Windows host could not be verified. Local actions remain unavailable.",
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
  }, []);

  const invalidateWorkspaceBoundUi = useCallback(() => {
    changesGenerationRef.current += 1;
    setChangesFlow(null);
    setChangesError(null);
    setChangesHistory(null);
    setChangesHistoryBusy(false);
    setEnableEditsBusy(false);
    recoveryGenerationRef.current += 1;
    recoveryDecisionRef.current = false;
    recoveryPreparationRef.current = false;
    setRecoveryReview(null);
    setRecoveryBusy(false);
    setRecoveryReturnFocusTarget(null);
    setContextPreview(null);
    setContextProvenance(null);
    setContextDrawer(null);
  }, []);

  function markReadOnlyRecovery(client: Extract<HostRuntime, { kind: "ready" | "read_only_recovery" }>["client"], sequence: number) {
    hostBindingGenerationRef.current += 1;
    invalidateWorkspaceBoundUi();
    clearMethodLibraryProjection();
    clearBmadHelpProjection();
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
      "Sapphirus entered read-only recovery. Workspace access was not changed.",
    );
  }

  function markHostUnavailable(client: Extract<HostRuntime, { kind: "ready" | "read_only_recovery" }>["client"]) {
    hostBindingGenerationRef.current += 1;
    invalidateWorkspaceBoundUi();
    clearMethodLibraryProjection();
    clearBmadHelpProjection();
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
        message: "The signed Windows host could not be verified. Local actions remain unavailable.",
      };
    });
  }

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
    setMobileSidebarOpen(false);
    setUtilitySettingsPage(mode === "settings" ? settingsPage : "general");
    setAppModal(mode);
    void loadAboutProjection();
  }

  async function loadAboutProjection() {
    if (
      hostRuntime.kind !== "ready"
      || !hostRuntime.bootstrap.supportedCommands.includes("app.about")
    ) {
      setAboutStatus("unavailable");
      return;
    }
    setAboutStatus("loading");
    try {
      const about = await hostRuntime.client.getAbout();
      setAboutProjection(about);
      setAboutStatus("ready");
    } catch {
      setAboutStatus("unavailable");
    }
    await loadRetentionManifest();
  }


  function openCapabilityRun(agentCode: string, menuCode: string, label: string) {
    const capabilityId = MENU_CAPABILITIES[`${agentCode}/${menuCode}`];
    if (capabilityId === undefined) {
      setCapabilityRun({
        capabilityId: `${agentCode}/${menuCode}`,
        label,
        phase: {
          kind: "error",
          message: "This menu target is not in the reviewed capability ledger.",
        },
      });
      return;
    }
    setCapabilityRun({ capabilityId, label, phase: { kind: "selecting" } });
  }

  function capabilityWorkspace(): { workspaceId: string; grantEpoch: number } | null {
    const active = hostWorkspaces[0];
    if (hostRuntime.kind !== "ready" || active === undefined) return null;
    return { workspaceId: active.workspaceId, grantEpoch: active.grantEpoch };
  }

  function failCapabilityRun(error: unknown) {
    setCapabilityRun((current) =>
      current === null
        ? null
        : { ...current, phase: { kind: "error", message: getSafeHostMessage(error) } },
    );
  }

  async function prepareCapabilityRun(contextPaths: readonly string[]) {
    const run = capabilityRun;
    const workspace = capabilityWorkspace();
    if (run === null || workspace === null || hostRuntime.kind !== "ready") return;
    setCapabilityRun({ ...run, phase: { kind: "preparing" } });
    try {
      const review = await hostRuntime.client.prepareCapabilityRun(
        workspace.workspaceId,
        workspace.grantEpoch,
        run.capabilityId,
        [...contextPaths],
      );
      setCapabilityRun({
        ...run,
        phase: { kind: "review", review, contextPaths },
      });
    } catch (error) {
      failCapabilityRun(error);
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
    try {
      const approved = await hostRuntime.client.approveCapabilityRun(
        workspace.workspaceId,
        workspace.grantEpoch,
        run.capabilityId,
        manifestHash,
      );
      setCapabilityRun({
        ...run,
        phase: {
          kind: "approved",
          review: run.phase.review,
          approved,
          contextPaths: run.phase.contextPaths,
          sending: false,
        },
      });
    } catch (error) {
      failCapabilityRun(error);
    }
  }

  async function cancelCapabilityRun(manifestHash: string, decisionId: string) {
    const run = capabilityRun;
    const workspace = capabilityWorkspace();
    if (run === null || workspace === null || hostRuntime.kind !== "ready") return;
    try {
      await hostRuntime.client.cancelCapabilityRun(
        workspace.workspaceId,
        workspace.grantEpoch,
        run.capabilityId,
        manifestHash,
        decisionId,
      );
      setCapabilityRun(null);
    } catch (error) {
      failCapabilityRun(error);
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
    setCapabilityRun({ ...run, phase: { ...run.phase, sending: true } });
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
        resultJson = latest.found ? latest.resultJson : null;
      } catch {
        resultJson = null;
      }
      setCapabilityRun({
        ...run,
        phase: { kind: "completed", completed, resultJson },
      });
    } catch (error) {
      failCapabilityRun(error);
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
    setOffboardingStatus("loading");
    try {
      const manifest = await hostRuntime.client.inspectOffboarding();
      setOffboardingManifest(manifest);
      setOffboardingStatus("ready");
    } catch {
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
        hostBindingGenerationRef.current += 1;
        setHostRuntime(runtime);
        const workspaces = runtime.kind === "browser_demo"
          ? [browserDemoWorkspace]
          : runtime.kind === "ready" || runtime.kind === "read_only_recovery"
            ? runtime.bootstrap.workspaces
            : [];
        setHostWorkspaces(workspaces);
        setActiveWorkspaceId(workspaces[0]?.workspaceId ?? null);
      })
      .catch(() => {
        if (!isCurrent) {
          return;
        }
        hostBindingGenerationRef.current += 1;
        setHostRuntime({
          kind: "unavailable",
          client: null,
          bootstrap: null,
          message: "The signed Windows host could not be verified. Local actions remain unavailable.",
        });
        setHostWorkspaces([]);
        setActiveWorkspaceId(null);
      });
    return () => {
      isCurrent = false;
    };
  }, [hostRuntimeLoader]);

  useEffect(() => {
    invalidateWorkspaceBoundUi();
    setMobileSidebarOpen(false);
  }, [invalidateWorkspaceBoundUi, workspaceAuthorityKey]);

  useEffect(() => {
    clearMethodLibraryProjection();
    if (!methodLibraryClient) {
      setContextDrawer((current) => current === "skills" ? null : current);
    }
  }, [activeWorkspaceId, clearMethodLibraryProjection, methodLibraryClient]);

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
          applyBmadEvent({ type: "recovered", run: null });
          return;
        }
        if (result.kind === "retained") {
          applyBmadEvent({ type: "recovered", run: result.run });
          return;
        }
        if (result.kind === "interrupted") {
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
  }, [applyBmadEvent, methodGuidanceBindingKey, methodGuidanceClient]);

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
          const workspaces = await client.listWorkspaces();
          if (isStale()) {
            return;
          }
          setHostWorkspaces(workspaces);
          setActiveWorkspaceId((current) => current !== null
            && workspaces.some(({ workspaceId }) => workspaceId === current)
            ? current
            : workspaces[0]?.workspaceId ?? null);
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
          && (error.details.code === "recovery_required" || error.details.code === "integrity_failure")
        ) {
          markReadOnlyRecovery(client, cursor);
        } else if (!(error instanceof HostCommandError && error.details.retryable)) {
          markHostUnavailable(client);
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
  }, [hostRuntime, loadMethodLibrary, projectionPollIntervalMs]);

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
    if (!sessions.some((session) => session.id === id)) {
      return;
    }
    setPrimaryRoute({ kind: "task", taskId: id });
    setContextDrawer(null);
    setMobileSidebarOpen(false);
  }

  function startNewSession() {
    if (methodRequestInFlight) {
      return;
    }
    sessionSequenceRef.current += 1;
    const session: SessionSummary = {
      id: `new-${sessionSequenceRef.current}`,
      title: "New task",
      updatedAt: "Now",
    };
    bmadHelpGenerationRef.current += 1;
    bmadHelpCreationRef.current = null;
    bmadHelpOperationRef.current = null;
    bmadHelpStateRef.current = initialBmadRequestState;
    setBmadHelpState(initialBmadRequestState);
    setContextPreview(null);
    setContextProvenance(null);
    setSessions([session]);
    setPrimaryRoute({ kind: "task", taskId: session.id });
    setContextDrawer(null);
    setMobileSidebarOpen(false);
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
    setMobileSidebarOpen(false);
    if (!bmadLibraryRequestedRef.current) {
      void loadMethodLibrary(methodLibraryClient);
    }
  }

  function reviewBmadRequest(currentIntent: string): Promise<void> {
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
    const projectionSequence = hostRuntime.kind === "ready"
      ? hostRuntime.bootstrap.projectionSequence
      : 0;
    const generation = ++bmadHelpGenerationRef.current;
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
        );
        if (
          generation !== bmadHelpGenerationRef.current
          || modelAuthStatus.status !== "development_ready"
        ) {
          throw new HostCapabilityError(
            "The request authority changed before review opened.",
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
        openContextDrawer("skills");
        setMobileSidebarOpen(false);
        if (!bmadLibraryRequestedRef.current) {
          void loadMethodLibrary(client);
        }
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

  function approveBmadContext() {
    const state = bmadHelpStateRef.current;
    if (
      state.kind !== "review_required"
      || !methodGuidanceClient
      || !activeWorkspace
      || bmadHelpOperationRef.current !== null
    ) return;
    const authority = currentBmadAuthority(state.run.runId);
    if (!authority || !bmadRequestAuthorityIsCurrent(state, authority)) {
      applyBmadEvent({ type: "authority_invalidated", reason: "authority_changed" });
      return;
    }
    const generation = authority.rendererGeneration;
    bmadHelpOperationRef.current = "approve";
    applyBmadEvent({ type: "approve_started" });
    void methodGuidanceClient.approveBmadHelp(
      activeWorkspace.workspaceId,
      activeWorkspace.grantEpoch,
      state.authority.manifestHash,
    ).then((approval) => {
      const current = bmadHelpStateRef.current;
      const snapshot = currentBmadAuthority(state.run.runId);
      if (
        generation !== bmadHelpGenerationRef.current
        || current.kind !== "approving"
        || !snapshot
        || !bmadRequestAuthorityIsCurrent(current, snapshot)
      ) return;
      applyBmadEvent({ type: "approved", approval });
    }).catch((error: unknown) => {
      if (generation === bmadHelpGenerationRef.current) {
        applyBmadEvent({ type: "unavailable", message: getSafeHostMessage(error) });
      }
    }).finally(() => {
      if (bmadHelpOperationRef.current === "approve") bmadHelpOperationRef.current = null;
    });
  }

  function cancelBmadContext() {
    const state = bmadHelpStateRef.current;
    if (state.kind === "review_required") {
      applyBmadEvent({ type: "terminal", reason: "cancelled" });
      return;
    }
    if (
      state.kind !== "approved"
      || !methodGuidanceClient
      || !activeWorkspace
      || bmadHelpOperationRef.current !== null
    ) return;
    const authority = currentBmadAuthority(state.run.runId);
    if (!authority || !bmadRequestAuthorityIsCurrent(state, authority)) {
      applyBmadEvent({ type: "authority_invalidated", reason: "authority_changed" });
      return;
    }
    bmadHelpOperationRef.current = "cancel";
    applyBmadEvent({ type: "terminal", reason: "cancelled" });
    void methodGuidanceClient.cancelBmadHelp(
      activeWorkspace.workspaceId,
      activeWorkspace.grantEpoch,
      state.authority.manifestHash,
      state.approval.decisionId,
    ).catch(() => undefined).finally(() => {
      if (bmadHelpOperationRef.current === "cancel") bmadHelpOperationRef.current = null;
    });
  }

  function sendBmadRequest() {
    const state = bmadHelpStateRef.current;
    if (
      state.kind !== "approved"
      || !methodGuidanceClient
      || !activeWorkspace
      || bmadHelpOperationRef.current !== null
    ) return;
    const authority = currentBmadAuthority(state.run.runId);
    if (!authority || !bmadRequestAuthorityIsCurrent(state, authority)) {
      applyBmadEvent({ type: "authority_invalidated", reason: "authority_changed" });
      return;
    }
    const workspace = activeWorkspace;
    const generation = authority.rendererGeneration;
    const manifestHash = state.authority.manifestHash;
    const decisionId = state.approval.decisionId;
    bmadHelpOperationRef.current = "submit";
    applyBmadEvent({ type: "submit_started" });
    void methodGuidanceClient.submitBmadHelp(
      workspace.workspaceId,
      workspace.grantEpoch,
      manifestHash,
      decisionId,
    ).then((result) => {
      if (
        generation === bmadHelpGenerationRef.current
        && bmadHelpStateRef.current.kind === "submitting"
      ) applyBmadEvent({ type: "completed", result });
    }).catch(() => {
      if (generation === bmadHelpGenerationRef.current) {
        applyBmadEvent({ type: "terminal", reason: "failed" });
      }
    }).finally(() => {
      if (bmadHelpOperationRef.current === "submit") bmadHelpOperationRef.current = null;
    });
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
  const editsEnabled = activeWorkspace?.permissions === "governed_edits";
  const canEnableEdits = governedEditsCommandsAvailable
    && activeWorkspace !== null
    && activeWorkspace.permissions === "read_only"
    && !enableEditsBusy;
  const changesState: GovernedChangesUiState = (() => {
    if (hostRuntime.kind === "browser_demo") {
      return {
        kind: "unavailable",
        reason: "Governed edits require the signed Windows desktop host. This browser view is for visual QA only.",
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

  function resetChangesFlow() {
    changesGenerationRef.current += 1;
    setChangesFlow(null);
    setChangesError(null);
  }

  function clearRecoveryReview() {
    recoveryGenerationRef.current += 1;
    recoveryDecisionRef.current = false;
    recoveryPreparationRef.current = false;
    setRecoveryReview(null);
    setRecoveryBusy(false);
    setRecoveryReturnFocusTarget(null);
  }

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
  }, [recoveryReview?.expiresAt, recoveryReview?.recoveryApprovalId]);

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
    setChangesError(null);
    try {
      const history = await client.changesHistory(workspace.workspaceId, workspace.grantEpoch);
      if (
        generation === changesGenerationRef.current
        && workspaceAuthorityKeyRef.current === authorityKey
        && hostRuntime.kind === "ready"
      ) {
        setChangesHistory(history);
      }
    } catch (error) {
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
      setChangesFlow({ kind: "idle" });
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
    const generation = ++changesGenerationRef.current;
    setChangesFlow({ kind: "preparing" });
    setChangesError(null);
    try {
      const review = await client.proposeCapabilityChanges(
        workspace.workspaceId,
        workspace.grantEpoch,
        capabilityId,
      );
      if (generation === changesGenerationRef.current) {
        setChangesFlow({ kind: "review", busy: false, review });
      }
    } catch (error) {
      if (generation === changesGenerationRef.current) {
        setChangesFlow({ kind: "idle" });
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
    const generation = ++changesGenerationRef.current;
    setChangesFlow({ kind: "review", busy: true, review });
    setChangesError(null);
    try {
      const decision = await client.decideApproval(review, choice);
      if (generation !== changesGenerationRef.current) {
        return;
      }
      if (decision.disposition === "applied" && decision.execution) {
        setChangesFlow({ kind: "applied", busy: false, execution: decision.execution });
        setChangesHistory(null);
      } else if (decision.disposition === "discarded") {
        setChangesFlow({ kind: "discarded" });
      } else {
        setChangesFlow({ kind: "idle" });
      }
    } catch (error) {
      if (generation === changesGenerationRef.current) {
        // The host consumes a pending proposal on any decision attempt, so a
        // failed apply requires a fresh review.
        setChangesFlow({ kind: "idle" });
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
    const generation = ++changesGenerationRef.current;
    setChangesFlow(changesState.kind === "applied"
      ? { kind: "applied", busy: true, execution: changesState.execution }
      : { kind: "preparing" });
    setChangesError(null);
    try {
      const result = await client.requestRollback(executionId);
      if (generation !== changesGenerationRef.current) {
        return;
      }
      if (result.kind === "review") {
        setChangesFlow({ kind: "review", busy: false, review: result.value });
      } else {
        setChangesFlow({ kind: "undo_unavailable", value: result.value });
      }
    } catch (error) {
      if (generation === changesGenerationRef.current) {
        setChangesFlow({ kind: "idle" });
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
      resetChangesFlow();
      setChangesHistory(null);
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
    setMobileSidebarOpen(false);
    setAppModal("workspace-manager");
  }

  function openWorkspaceManagerFromUtilityPanel() {
    const returnFocus = utilityReturnFocusRef.current;
    utilityReturnFocusRef.current = null;
    workspaceReturnFocusRef.current = returnFocus;
    setContextDrawer(null);
    setMobileSidebarOpen(false);
    setAppModal("workspace-manager");
  }

  const shellOverlayOpen = appModal !== null
    || (drawerIsOverlay && contextDrawer !== null)
    || (sidebarIsOverlay && mobileSidebarOpen);

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
          onContextReview={(projection, provenance) => {
            setContextPreview(projection);
            setContextProvenance(provenance);
            dismissContextDrawer();
          }}
          source={workspaceSource}
          workspaceName={workspaceName}
        />
      ) : contextDrawer === "changes" ? (
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
      ) : contextDrawer === "skills" ? (
        <div className="method-library-panel">
          <BmadHelpCard
            developmentOnly={modelAuthStatus?.developmentOnly ?? false}
            onApprove={approveBmadContext}
            onCancel={cancelBmadContext}
            onSend={sendBmadRequest}
            state={bmadHelpState}
          />
          {capabilityRun === null ? null : (
            <BmadCapabilityPanel
              capabilityId={capabilityRun.capabilityId}
              capabilityLabel={capabilityRun.label}
              destinationLabel={modelAccess.label}
              onApprove={(manifestHash) => void approveCapabilityRun(manifestHash)}
              onCancel={(manifestHash, decisionId) =>
                void cancelCapabilityRun(manifestHash, decisionId)}
              onClose={() => setCapabilityRun(null)}
              onPrepare={(contextPaths) => void prepareCapabilityRun(contextPaths)}
              onReviewChangeSet={() =>
                void proposeCapabilityChangeSet(capabilityRun.capabilityId)}
              onSubmit={(manifestHash, decisionId) =>
                void submitCapabilityRun(manifestHash, decisionId)}
              phase={capabilityRun.phase}
            />
          )}
          <BmadLibraryPanel
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
  ) : appModal === "settings" || appModal === "account" ? (
    <SettingsDialog
      about={aboutProjection}
      aboutStatus={aboutStatus}
      agentStatusLabel={methodStatusLabel}
      density={density}
      initialSection={utilitySettingsPage}
      key={`${appModal}:${utilitySettingsPage}`}
      modelAccessDetail={modelAccess.detail}
      modelAccessLabel={modelAccess.label}
      offboardingManifest={offboardingManifest}
      offboardingNotice={offboardingNotice}
      offboardingStatus={offboardingStatus}
      onClose={() => dismissUtilityPanel()}
      onDensityChange={changeDensity}
      onEraseOffboarding={(confirm) => void eraseOffboardingData(confirm)}
      onManageWorkspaces={openWorkspaceManagerFromUtilityPanel}
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
  ) : undefined;

  const paletteActions: readonly PaletteAction[] = [
    { id: "new-task", label: "New task", hint: "chat", run: startNewSession },
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
      label: "Show proposed changes",
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
    {
      id: "account",
      label: "Open account",
      hint: "app",
      run: () => openUtilityPanel("account"),
    },
  ];

  return (
    <div
      className="app-shell"
      onKeyDown={(event) => {
        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
          event.preventDefault();
          setPaletteOpen((current) => !current);
        }
      }}
    >
      <CommandPalette
        actions={paletteActions}
        onClose={() => setPaletteOpen(false)}
        open={paletteOpen}
      />
      <TitleBar isInert={shellOverlayOpen} />
      <div className="workbench workbench--task-shell">
        <AppShellLayout
          drawer={drawer}
          main={activeWorkspace ? (
            <TaskWorkspace
              canAttachFiles={workspaceSource !== null}
              contextPreview={contextPreview}
              hostStatusLabel={hostStatusLabel}
              interactionDisabled={!methodRequestAvailable}
              isBrowserDemo={hostRuntime.kind === "browser_demo"}
              isNewSession={isNewSession}
              isReadOnlyRecovery={hostRuntime.kind === "read_only_recovery"}
              key={`${selectedSessionId}:${methodGuidanceBindingKey}`}
              methodGuidanceAvailable={methodRequestAvailable}
              methodGuidanceState={bmadHelpState}
              methodLibraryAvailable={methodLibraryAvailable}
              modelAccessDetail={modelAccess.detail}
              modelAccessLabel={modelAccess.label}
              onAttachFiles={() => {
                setMobileSidebarOpen(false);
                openContextDrawer("files");
              }}
              onBrowseFiles={canBrowseFiles ? browseAndAttachFiles : undefined}
              attachNotice={attachNotice}
              agentLibrary={bmadLibraryState}
              onOpenChanges={() => {
                setMobileSidebarOpen(false);
                openContextDrawer("changes");
              }}
              onOpenMethodLibrary={openMethodLibrary}
              onOpenRunDetails={() => {
                setMobileSidebarOpen(false);
                openContextDrawer("activity");
              }}
              onOpenSidebar={() => {
                setContextDrawer(null);
                setMobileSidebarOpen(true);
              }}
              onReviewRequest={reviewBmadRequest}
              sessionTitle={selectedSession.title}
              workspaceName={workspaceName}
            />
          ) : (
            <main className="task-shell-empty">
              <NoWorkspaceState
                copy={explorerAvailabilityMessage}
                mode={hostRuntime.kind}
                onOpenWorkspace={() => void selectWorkspace()}
              />
            </main>
          )}
          mobileSidebarOpen={mobileSidebarOpen}
          modal={modal}
          onCloseDrawer={dismissContextDrawer}
          onCloseModal={() => {
            if (appModal === "workspace-manager") {
              dismissWorkspacePanel();
            } else {
              dismissUtilityPanel();
            }
          }}
          onCloseSidebar={() => setMobileSidebarOpen(false)}
          sidebar={(
            <AppSidebar
              canCreateTask={hostRuntime.kind === "ready" && activeWorkspace !== null && !methodRequestInFlight}
              onNewTask={startNewSession}
              onOpenAccount={() => openUtilityPanel("account")}
              onOpenSettings={() => openUtilityPanel("settings")}
              onOpenWorkspaceManager={openWorkspaceManager}
              onSelectTask={selectSession}
              selectedTaskId={activeWorkspace ? selectedSessionId : null}
              tasks={activeWorkspace ? sessions : []}
              workspaceLabel={activeWorkspace?.displayName ?? "No workspace"}
              workspaceStatus={workspaceDescription}
            />
          )}
        />
      </div>
    </div>
  );
}
