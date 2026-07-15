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
  type DesktopHostClient,
  HostCommandError,
  type ContextPreviewProjection,
  type HostRuntime,
  type WorkspaceProjection,
} from "./lib/hostClient";
import type {
  BmadHelpUiState,
  BmadLibrarySnapshot,
  BmadLibraryUiState,
} from "./lib/bmadProjection";
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
  const inspectorIsOverlay = useMediaQuery("(max-width: 1050px)");
  const sessionsIsOverlay = useMediaQuery("(max-width: 820px)");
  const workspaceActionBusyRef = useRef(false);
  const workspaceReturnFocusRef = useRef<HTMLElement | null>(null);
  const utilityReturnFocusRef = useRef<HTMLElement | null>(null);
  const hostBindingGenerationRef = useRef(0);
  const bmadProjectionGenerationRef = useRef(0);
  const bmadLibraryRequestedRef = useRef(false);

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
      ? "Local workspace · Read only"
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
  const bmadHelpState: BmadHelpUiState = { kind: "no_evidence" };
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

  function markReadOnlyRecovery(client: Extract<HostRuntime, { kind: "ready" | "read_only_recovery" }>["client"], sequence: number) {
    hostBindingGenerationRef.current += 1;
    setContextPreview(null);
    setContextProvenance(null);
    clearMethodLibraryProjection();
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

  function submitTask() {
    // Connected task submission is intentionally absent from this internal desktop build.
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
              key={selectedSessionId}
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
              onTaskSubmitted={submitTask}
              proposalState={proposalState}
              sessionTitle={selectedSession.title}
              workspaceName={workspaceName}
            />
          )}
          <Inspector
            bmadHelpState={bmadHelpState}
            bmadLibraryState={bmadLibraryState}
            contextPreview={contextPreview}
            contextProvenance={contextProvenance}
            interactionDisabled
            isInert={sessionsIsModal}
            isOpen={inspectorOpen}
            isOverlay={inspectorIsOverlay}
            methodLibraryAvailable={methodLibraryAvailable}
            onApply={() => undefined}
            onClose={() => setInspectorOpen(false)}
            onDiscard={() => undefined}
            onRevise={() => undefined}
            onReloadMethodLibrary={() => {
              if (methodLibraryClient) {
                void loadMethodLibrary(methodLibraryClient);
              }
            }}
            onTabChange={setInspectorTab}
            proposalState={proposalState}
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
