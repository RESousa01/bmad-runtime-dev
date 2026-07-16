import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { GlobalRail } from "./components/GlobalRail";
import { Inspector } from "./components/Inspector";
import { SessionRail } from "./components/SessionRail";
import { TaskWorkspace } from "./components/TaskWorkspace";
import { TitleBar } from "./components/TitleBar";
import { UtilityPanel } from "./components/UtilityPanel";
import { WorkspacePanel } from "./components/WorkspacePanel";
import { WorkspaceExplorer } from "./components/WorkspaceExplorer";
import {
  initialSessions,
  type DensityPreference,
  type InspectorTab,
  type PrimaryView,
  type ProposalState,
  type SessionSummary,
  type ThemePreference,
} from "./data/demo";
import {
  getDefaultHostRuntime,
  getSafeHostMessage,
  type ApprovalChoice,
  type DesktopHostClient,
  HostCapabilityError,
  HostCommandError,
  type ContextPreviewProjection,
  type HostRuntime,
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

const fallbackSession: SessionSummary = {
  id: "scan",
  title: "Add a safe workspace scan",
  updatedAt: "10:42 AM",
};

const browserDemoWorkspace: WorkspaceProjection = {
  workspaceId: "workspace_browser_demo",
  projectId: "project_browser_demo",
  displayName: "bmad-runtime-dev",
  grantEpoch: 0,
  permissions: "read_only",
};

const retainedHelpProjectionUnavailableMessage =
  "A retained Method session from an earlier version exists, but its authenticated projection is unavailable. You can create a new local Method session for this workspace grant.";

type HostUiRuntime = HostRuntime | { kind: "loading" };

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
  const [activeView, setActiveView] = useState<PrimaryView>("agent");
  const [density, setDensity] = useState<DensityPreference>("comfortable");
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [inspectorTab, setInspectorTab] = useState<InspectorTab>("changes");
  const [proposalState, setProposalState] = useState<ProposalState>("ready");
  const [selectedSessionId, setSelectedSessionId] = useState("scan");
  const sessions = initialSessions;
  const [sessionRailOpen, setSessionRailOpen] = useState(false);
  const [theme, setTheme] = useState<ThemePreference>("dark");
  const [utilityPanel, setUtilityPanel] = useState<"account" | "settings" | null>(null);
  const [hostRuntime, setHostRuntime] = useState<HostUiRuntime>({ kind: "loading" });
  const [hostWorkspaces, setHostWorkspaces] = useState<WorkspaceProjection[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string | null>(null);
  const [workspacePanelOpen, setWorkspacePanelOpen] = useState(false);
  const [workspaceSelectionBusy, setWorkspaceSelectionBusy] = useState(false);
  const [workspaceRemovalBusyId, setWorkspaceRemovalBusyId] = useState<string | null>(null);
  const [workspaceActionError, setWorkspaceActionError] = useState<string | null>(null);
  const [contextPreview, setContextPreview] = useState<ContextPreviewProjection | null>(null);
  const [contextProvenance, setContextProvenance] = useState<WorkspaceProjectionProvenance | null>(null);
  const [bmadLibraryState, setBmadLibraryState] = useState<BmadLibraryUiState>({ kind: "idle" });
  const [bmadHelpState, setBmadHelpState] = useState<BmadRequestState>(initialBmadRequestState);
  const [modelAuthStatus, setModelAuthStatus] = useState<ModelAuthStatusProjection | null>(null);
  const [changesFlow, setChangesFlow] = useState<GovernedChangesUiState | null>(null);
  const [changesError, setChangesError] = useState<string | null>(null);
  const [enableEditsBusy, setEnableEditsBusy] = useState(false);
  const changesGenerationRef = useRef(0);
  const inspectorIsOverlay = useMediaQuery("(max-width: 1050px)");
  const sessionsIsOverlay = useMediaQuery("(max-width: 820px)");
  const workspaceActionBusyRef = useRef(false);
  const workspaceReturnFocusRef = useRef<HTMLElement | null>(null);
  const utilityReturnFocusRef = useRef<HTMLElement | null>(null);
  const hostBindingGenerationRef = useRef(0);
  const bmadProjectionGenerationRef = useRef(0);
  const bmadLibraryRequestedRef = useRef(false);
  const bmadHelpGenerationRef = useRef(0);
  const bmadHelpCreationRef = useRef<Promise<void> | null>(null);
  const bmadHelpOperationRef = useRef<"approve" | "cancel" | "submit" | null>(null);
  const bmadHelpStateRef = useRef<BmadRequestState>(initialBmadRequestState);

  const selectedSession = useMemo(
    () => sessions.find((session) => session.id === selectedSessionId) ?? fallbackSession,
    [selectedSessionId, sessions],
  );
  const isNewSession = selectedSession.id.startsWith("new-");
  const activeWorkspace = hostWorkspaces.find(({ workspaceId }) => workspaceId === activeWorkspaceId)
    ?? hostWorkspaces[0]
    ?? null;
  const workspaceName = activeWorkspace?.displayName ?? "No local workspace selected";
  const workspaceDescription = hostRuntime.kind === "browser_demo"
    ? "Preview workspace · no local access"
    : activeWorkspace
      ? activeWorkspace.permissions === "governed_edits"
        ? "Local workspace · Governed edits"
        : "Local workspace · Read only"
      : "Choose one from Workspaces";
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
  const methodGuidanceBindingKey = hostRuntime.kind === "ready"
    || hostRuntime.kind === "read_only_recovery"
    ? `${hostRuntime.bootstrap.rendererSessionId}:${activeWorkspace?.workspaceId ?? "none"}:${activeWorkspace?.grantEpoch ?? 0}`
    : hostRuntime.kind;
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

  function markReadOnlyRecovery(client: Extract<HostRuntime, { kind: "ready" | "read_only_recovery" }>["client"], sequence: number) {
    hostBindingGenerationRef.current += 1;
    setContextPreview(null);
    setContextProvenance(null);
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
    setContextPreview(null);
    setContextProvenance(null);
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
    setWorkspacePanelOpen(false);
    const returnFocus = workspaceReturnFocusRef.current;
    workspaceReturnFocusRef.current = null;
    window.requestAnimationFrame(() => returnFocus?.focus());
  }

  function openUtilityPanel(mode: "account" | "settings") {
    utilityReturnFocusRef.current = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    setWorkspacePanelOpen(false);
    setUtilityPanel(mode);
  }

  function dismissUtilityPanel(restoreFocus = true) {
    setUtilityPanel(null);
    const returnFocus = utilityReturnFocusRef.current;
    utilityReturnFocusRef.current = null;
    if (restoreFocus) {
      window.requestAnimationFrame(() => returnFocus?.isConnected && returnFocus.focus());
    }
  }

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
    setContextPreview(null);
    setContextProvenance(null);
  }, [activeWorkspaceId]);

  useEffect(() => {
    clearMethodLibraryProjection();
    if (!methodLibraryClient) {
      setInspectorTab((current) => current === "method" ? "changes" : current);
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

  useEffect(() => {
    const closeTransientPanels = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      setInspectorOpen(false);
      setSessionRailOpen(false);
      dismissUtilityPanel();
      dismissWorkspacePanel();
    };
    window.addEventListener("keydown", closeTransientPanels);
    return () => window.removeEventListener("keydown", closeTransientPanels);
  }, []);

  function selectNavigation(view: PrimaryView) {
    setActiveView(view);
    dismissUtilityPanel(false);
    if (view === "workspaces") {
      workspaceReturnFocusRef.current = document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
      setWorkspacePanelOpen(true);
      setInspectorOpen(false);
      return;
    }
    setWorkspacePanelOpen(false);
    if (view === "explorer") {
      setInspectorOpen(false);
      setSessionRailOpen(false);
      return;
    }
    if (view === "agent") {
      return;
    }
    const mappedTab: Partial<Record<PrimaryView, InspectorTab>> = {
      changes: "changes",
      activity: "evidence",
    };
    const nextTab = mappedTab[view];
    if (nextTab) {
      setInspectorTab(nextTab);
      setSessionRailOpen(false);
      setInspectorOpen(true);
    }
  }

  function selectSession(id: string) {
    setSelectedSessionId(id);
    setActiveView("agent");
    setProposalState(id.startsWith("new-") ? "discarded" : "ready");
    setSessionRailOpen(false);
  }

  function reviewChanges() {
    setInspectorTab("changes");
    setSessionRailOpen(false);
    setInspectorOpen(true);
  }

  function openMethodLibrary() {
    if (!methodLibraryClient) {
      return;
    }
    setInspectorTab("method");
    setSessionRailOpen(false);
    setInspectorOpen(true);
    if (!bmadLibraryRequestedRef.current) {
      void loadMethodLibrary(methodLibraryClient);
    }
  }

  function reviewBmadRequest(currentIntent: string): Promise<void> {
    if (bmadHelpCreationRef.current) {
      return bmadHelpCreationRef.current;
    }
    if (
      !methodGuidanceAvailable
      || !methodGuidanceClient
      || !activeWorkspace
      || !modelAuthStatus
      || bmadHelpOperationRef.current !== null
    ) {
      return Promise.reject(
        new HostCapabilityError("Method guidance is unavailable for the active workspace grant."),
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
            "The Method guidance result no longer belongs to the active workspace grant.",
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
            "The Method request authority changed before review opened.",
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
          throw new HostCapabilityError("The prepared Method request failed closed.");
        }
        setInspectorTab("method");
        setSessionRailOpen(false);
        setInspectorOpen(true);
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
    && (["workspace.enable_edits", "changes.propose", "approval.decide", "rollback.request"] as const)
      .every((command) => hostRuntime.bootstrap.supportedCommands.includes(command));
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

  async function proposeGovernedChange(relativePath: string, content: string) {
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
      const review = await client.proposeChanges(workspace.workspaceId, workspace.grantEpoch, [
        { change: "set_content", relativePath, content },
      ]);
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
    if (hostRuntime.kind !== "ready" || changesState.kind !== "applied") {
      return;
    }
    const client = hostRuntime.client;
    const sequence = hostRuntime.bootstrap.projectionSequence;
    const generation = ++changesGenerationRef.current;
    setChangesFlow({ kind: "applied", busy: true, execution: changesState.execution });
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

  const inspectorIsModal = inspectorIsOverlay && inspectorOpen;
  const sessionsIsModal = sessionsIsOverlay && sessionRailOpen;
  const workbenchIsInert = inspectorIsModal || sessionsIsModal;
  const hasOverlayScrim = workbenchIsInert || workspacePanelOpen;

  return (
    <div className="app-shell">
      <div className="app-surface" inert={workspacePanelOpen}>
        <TitleBar isInert={workbenchIsInert} />
        <div className="workbench">
          <div aria-label="Workspace navigation" className="desktop-sidebar" role="group">
            <GlobalRail
              activeView={activeView}
              isInert={workbenchIsInert}
              onAccount={() => openUtilityPanel("account")}
              onNavigate={selectNavigation}
              onSettings={() => openUtilityPanel("settings")}
            />
            <SessionRail
              isInert={inspectorIsModal}
              isOpen={sessionRailOpen}
              isOverlay={sessionsIsOverlay}
              isSessionCreationEnabled={false}
              onClose={() => setSessionRailOpen(false)}
              onNewSession={() => undefined}
              onSelect={selectSession}
              selectedId={selectedSessionId}
              sessions={sessions}
              workspaceDescription={workspaceDescription}
              workspaceName={workspaceName}
            />
          </div>
          {activeView === "explorer" ? (
            <WorkspaceExplorer
              availabilityMessage={explorerAvailabilityMessage}
              isInert={workbenchIsInert}
              key={`${workspaceSource?.provenance ?? "unavailable"}:${activeWorkspaceId ?? "none"}`}
              onContextReview={(projection, provenance) => {
                setContextPreview(projection);
                setContextProvenance(provenance);
                setInspectorTab("context");
                setSessionRailOpen(false);
                setInspectorOpen(true);
              }}
              source={workspaceSource}
              workspaceName={workspaceName}
            />
          ) : (
            <TaskWorkspace
              hostStatusLabel={hostStatusLabel}
              interactionDisabled
              isInert={workbenchIsInert}
              isNewSession={isNewSession}
              isReadOnlyRecovery={hostRuntime.kind === "read_only_recovery"}
              key={`${selectedSessionId}:${methodGuidanceBindingKey}`}
              methodGuidanceAvailable={methodGuidanceAvailable}
              methodGuidanceState={bmadHelpState}
              methodLibraryAvailable={methodLibraryAvailable}
              onOpenMethodLibrary={openMethodLibrary}
              onOpenInspector={() => {
                setSessionRailOpen(false);
                setInspectorOpen(true);
              }}
              onOpenSessions={() => {
                setInspectorOpen(false);
                setSessionRailOpen(true);
              }}
              onReviewContext={() => {
                setInspectorTab("context");
                setSessionRailOpen(false);
                setInspectorOpen(true);
              }}
              onReviewChanges={reviewChanges}
              onReviewRequest={reviewBmadRequest}
              proposalState={proposalState}
              sessionTitle={selectedSession.title}
              workspaceName={workspaceName}
            />
          )}
          <Inspector
            bmadHelpState={bmadHelpState}
            bmadModelDevelopmentOnly={modelAuthStatus?.developmentOnly ?? false}
            bmadLibraryState={bmadLibraryState}
            changesPanel={{
              canEnableEdits,
              enableEditsBusy,
              errorMessage: changesError,
              onDecide: (choice) => void decideGovernedChange(choice),
              onEnableEdits: () => void enableGovernedEdits(),
              onPropose: (relativePath, content) => void proposeGovernedChange(relativePath, content),
              onStartNewProposal: resetChangesFlow,
              onUndo: (executionId) => void undoGovernedChange(executionId),
              state: changesState,
            }}
            contextPreview={contextPreview}
            contextProvenance={contextProvenance}
            isInert={sessionsIsModal}
            isOpen={inspectorOpen}
            isOverlay={inspectorIsOverlay}
            methodLibraryAvailable={methodLibraryAvailable}
            onBmadApprove={approveBmadContext}
            onBmadCancel={cancelBmadContext}
            onBmadSend={sendBmadRequest}
            onClose={() => setInspectorOpen(false)}
            onReloadMethodLibrary={() => {
              if (methodLibraryClient) {
                void loadMethodLibrary(methodLibraryClient);
              }
            }}
            onTabChange={setInspectorTab}
            selectedTab={inspectorTab}
          />
        </div>
      </div>
      {hasOverlayScrim ? (
        <button
          aria-label="Close open panel"
          className={`panel-scrim ${workspacePanelOpen ? "workspace-scrim" : ""}`}
          onClick={() => {
            if (workspacePanelOpen) {
              dismissWorkspacePanel();
            } else {
              setInspectorOpen(false);
              setSessionRailOpen(false);
            }
          }}
          type="button"
        />
      ) : null}
      {utilityPanel ? (
        <UtilityPanel
          density={density}
          key={utilityPanel}
          mode={utilityPanel}
          onClose={() => dismissUtilityPanel()}
          onDensityChange={setDensity}
          onThemeChange={setTheme}
          theme={theme}
        />
      ) : null}
      {workspacePanelOpen ? (
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
      ) : null}
    </div>
  );
}
