/**
 * The shell model, and the provider that carries it.
 *
 * FE-10's outcome names four things a composition root should be: providers,
 * routing, shell composition, and feature controllers. This is the first of
 * them, and it is what makes the others possible.
 *
 * The drawer, the modals and the render reference 122 of App's locals between
 * them. Passing those as props means a forty-parameter interface per surface
 * and a call site — `<AppDrawer {...forty} />` — that tells a reader nothing:
 * it would shorten App.tsx and make the application harder to follow. So the
 * assembled state travels as one value and each surface reads what it needs.
 *
 * Grouped, not flat
 * -----------------
 * The grouping is the point. A flat 122-member object is the same god
 * component with a different shape. Slices named for features mean a reader
 * sees what the shell is made of from the type alone, and a surface reaching
 * into a slice it has no business in is visible at the call site.
 *
 * Slices carry *state*, not pre-built props. Handing a surface its own props
 * from App would be the prop-drilling this replaces, moved one file along.
 *
 * Re-render cost: none beyond what already happens. Every consumer is inside
 * App and already re-renders whenever App does, so a value rebuilt each render
 * changes nothing about how often they run.
 */

import { createContext, useContext, type ComponentProps, type ReactNode } from "react";

import type { BmadLibraryUiState } from "../lib/bmadProjection";
import type { BmadRequestState } from "../lib/bmadModelProjection";
import type {
  ApprovalChoice,
  BmadWorkflowStateProjection,
  ChangesHistoryProjection,
  ContextPreviewProjection,
  RecoveryApprovalChoice,
  WorkspaceProjection,
} from "../lib/hostClient";
import type { BmadPersonaPerspectiveView } from "../components/BmadLibraryPanel";
import type { GitReviewSource } from "../components/GitReviewPanel";
import type { GovernedChangesUiState } from "../components/GovernedChangesPanel";
import type { SessionContextMessage } from "../components/SessionContextPanel";
import type { SettingsDialog, SettingsSection } from "../components/settings/SettingsDialog";
import type { DeleteTaskDialog } from "../components/DeleteTaskDialog";
import type { ContextDrawerKind } from "../components/redesign/ContextDrawer";
import type { ReadonlyWorkspaceSource } from "../lib/workspaceReadSource";
import type { ActiveCapabilityRunState } from "../state/taskSessions";
import type { RecoveryReview } from "./recovery/recoveryReview";

export type RuntimeKind =
  | "loading" | "browser_demo" | "ready" | "read_only_recovery" | "unavailable";

/** The host binding, as the surfaces need to describe it. */
export interface HostSlice {
  readonly runtimeKind: RuntimeKind;
}

export interface WorkspaceSlice {
  readonly active: WorkspaceProjection | null;
  readonly name: string;
  readonly explorerAvailabilityMessage: string;
  readonly source: ReadonlyWorkspaceSource | null;
  readonly manager: WorkspaceManagerSlice;
}

/** The workspace manager modal: the list, its capabilities, and its actions. */
export interface WorkspaceManagerSlice {
  readonly workspaces: readonly WorkspaceProjection[];
  readonly canActivate: boolean;
  readonly canRemove: boolean;
  readonly canSelect: boolean;
  readonly selectionBusy: boolean;
  readonly removalBusyId: string | null;
  readonly error: string | null;
  readonly activate: (workspaceId: string) => void;
  readonly close: () => void;
  readonly remove: (workspaceId: string) => void;
  readonly select: () => void;
}

/** Governed changes: proposing, deciding, undoing, and the journal. */
export interface ChangesSlice {
  readonly state: GovernedChangesUiState;
  readonly error: string | null;
  readonly history: ChangesHistoryProjection | null;
  readonly historyStatus: "unread" | "loading" | "loaded" | "unavailable";
  readonly historyBusy: boolean;
  readonly historyAvailable: boolean;
  readonly canEnableEdits: boolean;
  readonly enableEditsBusy: boolean;
  readonly gitReviewAvailabilityMessage: string;
  readonly gitReviewSource: GitReviewSource | null;
  readonly decide: (choice: ApprovalChoice) => void;
  readonly enableEdits: () => void;
  readonly refreshHistory: () => void;
  readonly startNewProposal: () => void;
  readonly undo: (executionId: string) => void;
}

/** Journal recovery, the only path that can act on a quarantined journal. */
export interface RecoverySlice {
  readonly review: RecoveryReview | null;
  readonly busy: boolean;
  readonly returnFocusTarget: HTMLElement | null;
  readonly decide: (choice: RecoveryApprovalChoice) => void;
  readonly prepare: (journalId: string, trigger: HTMLElement) => void;
}

/** The reviewed capability run, the method library, and workflow state. */
export interface BmadSlice {
  readonly library: BmadLibraryUiState;
  readonly helpState: BmadRequestState;
  readonly personaPerspectives: ReadonlyMap<string, BmadPersonaPerspectiveView>;
  readonly run: ActiveCapabilityRunState | null;
  readonly workflow: {
    readonly state: BmadWorkflowStateProjection | null;
    readonly loading: boolean;
    readonly error: string | null;
  };
  readonly approveRun: (manifestHash: string) => void;
  readonly cancelRun: (manifestHash: string, decisionId: string) => void;
  readonly closeRun: () => void;
  readonly openRun: (capabilityId: string, label: string) => void;
  readonly prepareRun: (contextPaths: readonly string[]) => void;
  readonly proposeChangeSet: (capabilityId: string) => void;
  readonly submitRun: (manifestHash: string, decisionId: string) => void;
  readonly reloadLibrary: () => void;
  readonly refreshWorkflow: () => void;
  readonly viewPersona: (agentCode: string) => void;
}

/** Attached context, and the model-usage figures the panel shows. */
export interface ContextSlice {
  readonly adoptReviewed: (projection: ContextPreviewProjection) => void;
  readonly session: {
    readonly createdAt: number | undefined;
    readonly inputTokens: number | undefined;
    readonly messages: readonly SessionContextMessage[];
  };
}

export interface ModelSlice {
  readonly destinationLabel: string;
  readonly providerLabel: string;
  readonly modelId: string | undefined;
}

/** Which overlay owns the page, and how to move between or close them. */
export interface OverlaySlice {
  /**
   * Which modal owns the page. Only one can, so this is a discriminant.
   *
   * `"account"` is a state the shell can be in with no dialog behind it yet.
   * It is listed rather than filtered out so that adding the dialog later is a
   * new case here, not a change to what the shell may hold.
   */
  readonly modalKind: "workspace-manager" | "settings" | "account" | "delete-task" | null;
  readonly drawerKind: Exclude<ContextDrawerKind, null> | null;
  readonly drawerIsOverlay: boolean;
  readonly closeDrawer: () => void;
  readonly selectDrawerTab: (tab: Exclude<ContextDrawerKind, null>) => void;
}

/**
 * Settings, as a typed pass-through rather than a re-typed slice.
 *
 * The other slices exist because several surfaces read the same state, so a
 * shared shape earns its keep. Settings has exactly one consumer and its props
 * *are* its state -- re-listing thirty fields here would be an isomorphic copy
 * that can only drift. `ComponentProps` cannot drift: a prop renamed in the
 * dialog fails to compile here.
 */
export interface SettingsSlice {
  readonly props: ComponentProps<typeof SettingsDialog>;
  /** Which section to open on, which also keys the dialog. */
  readonly section: SettingsSection;
}

/** The delete confirmation, or null when no task is pending deletion. */
export type DeletionSlice =
  | (Pick<
    ComponentProps<typeof DeleteTaskDialog>,
    "historyStatus" | "mayHaveDurableCopy" | "messageCount" | "taskTitle"
  > & {
    readonly cancel: () => void;
    readonly confirm: () => void;
  })
  | null;

export interface ShellModel {
  readonly bmad: BmadSlice;
  readonly changes: ChangesSlice;
  readonly context: ContextSlice;
  readonly deletion: DeletionSlice;
  readonly host: HostSlice;
  readonly settings: SettingsSlice;
  readonly model: ModelSlice;
  readonly overlays: OverlaySlice;
  readonly recovery: RecoverySlice;
  readonly workspace: WorkspaceSlice;
  /** The task on screen, or null on Home. */
  readonly selectedTaskTitle: string | null;
}

const ShellContext = createContext<ShellModel | null>(null);

export function ShellProvider({
  children,
  model,
}: {
  readonly children: ReactNode;
  readonly model: ShellModel;
}) {
  return <ShellContext.Provider value={model}>{children}</ShellContext.Provider>;
}

/**
 * Reads the shell model.
 *
 * Throws rather than returning null. A surface rendered outside the provider is
 * a wiring mistake, and the alternative — optional chaining at every use —
 * turns that mistake into a silently empty panel.
 */
export function useShell(): ShellModel {
  const model = useContext(ShellContext);
  if (model === null) {
    throw new Error("useShell must be used inside <ShellProvider>.");
  }
  return model;
}
