/**
 * The words the shell uses to describe what it is connected to.
 *
 * FE-10 step 3. These were four nested ternaries and two IIFEs in the middle of
 * App's body, and they were the only place several distinctions were drawn --
 * "checking" versus "not connected", "local only" versus "demo only". A wrong
 * label here is not cosmetic: it is the app telling the user something untrue
 * about what it can do, and it is what sends them to Settings to fix a problem
 * that is somewhere else.
 *
 * Pure functions of the runtime and the projections, so each distinction can be
 * stated once and checked.
 */

import type { ModelAuthStatusProjection } from "../lib/hostClient";

/** The runtime kinds these labels distinguish between. */
export type RuntimeKind =
  | "loading"
  | "browser_demo"
  | "ready"
  | "read_only_recovery"
  | "unavailable";

export function hostStatusLabel(kind: RuntimeKind): string {
  switch (kind) {
    case "loading": return "Verifying local host";
    case "browser_demo": return "Browser preview";
    case "ready": return "Local host ready";
    case "read_only_recovery": return "Read-only recovery";
    case "unavailable": return "Host unavailable";
  }
}

export interface WorkspaceDescriptionInput {
  readonly runtimeKind: RuntimeKind;
  /** Null when no workspace is bound. */
  readonly permissions: "read_only" | "governed_edits" | null;
}

export function workspaceDescription({
  permissions,
  runtimeKind,
}: WorkspaceDescriptionInput): string {
  // The preview reads as sample data whatever the fixture's permissions say,
  // because nothing here touches a real folder.
  if (runtimeKind === "browser_demo") return "Sample data · Read only";
  if (permissions === null) return "Choose a workspace";
  return permissions === "governed_edits" ? "Governed edits" : "Read only";
}

export interface ModelAccess {
  readonly label: string;
  readonly detail: string;
}

export interface ModelAccessInput {
  readonly runtimeKind: RuntimeKind;
  readonly modelAuth: ModelAuthStatusProjection | null;
  /** True once the host has projected the model commands. */
  readonly modelCommandsAvailable: boolean;
  readonly libraryLoaded: boolean;
}

/** Which provider a status names, in the words the user chose it by. */
export function providerLabel(providerId: string | undefined): string {
  switch (providerId) {
    case "azure": return "Azure OpenAI";
    case "openai": return "OpenAI";
    case "forge-local": return "Forge local";
    default: return "Forge BMAD";
  }
}

export function modelAccess({
  libraryLoaded,
  modelAuth,
  modelCommandsAvailable,
  runtimeKind,
}: ModelAccessInput): ModelAccess {
  if (runtimeKind === "browser_demo") {
    return {
      detail: "No local folder, consent grant, or model request is active.",
      label: "No model access",
    };
  }
  if (modelAuth?.status === "development_ready") {
    const mode = modelAuth.mode === "deterministic_development"
      ? "Deterministic development"
      : modelAuth.providerId === "azure"
        ? "Azure OpenAI"
        : modelAuth.providerId === "openai"
          ? "OpenAI"
          : "Direct provider";
    return {
      detail: `${mode} · ready`,
      // The host's own label wins; the fallback is for a status that arrived
      // ready without naming a destination.
      label: modelAuth.destinationLabel || "Development model",
    };
  }
  if (runtimeKind === "loading") {
    return { detail: "Waiting for the native desktop host.", label: "Checking access" };
  }
  /*
   * "Checking" rather than "connect a provider".
   *
   * The host can answer model commands but has not yet said what the model
   * connection is. Telling the user to connect one while the answer is still
   * in flight sends them to Settings to fix something that may not be broken.
   */
  if (modelCommandsAvailable && modelAuth === null) {
    return {
      detail: "The desktop host is checking the current model connection.",
      label: "Checking model access",
    };
  }
  return {
    detail: libraryLoaded
      ? "Choose OpenAI or Azure OpenAI in Settings to send messages."
      : "No model provider or skills catalog is active.",
    label: "Connect a provider",
  };
}

export interface MethodStatusInput {
  readonly runtimeKind: RuntimeKind;
  readonly chatAvailable: boolean;
  /** True when the host projects the guidance commands, model aside. */
  readonly guidanceCommandsAvailable: boolean;
}

/**
 * The one-word state shown beside the agent selector.
 *
 * "Local only" and "Demo only" are different claims: the first is a real host
 * that can do local work but cannot reach a model, the second is a preview with
 * no host at all. Collapsing them would tell a preview user their local host is
 * fine.
 */
export function methodStatusLabel({
  chatAvailable,
  guidanceCommandsAvailable,
  runtimeKind,
}: MethodStatusInput): string {
  if (chatAvailable) return "Model ready";
  if (guidanceCommandsAvailable) return "Local only";
  return runtimeKind === "browser_demo" ? "Demo only" : "Not connected";
}

export interface ExplorerMessageInput {
  readonly runtimeKind: RuntimeKind;
  readonly hasWorkspace: boolean;
  /** The host's own message, which is the only accurate one when unavailable. */
  readonly unavailableMessage: string | null;
}

export function explorerAvailabilityMessage({
  hasWorkspace,
  runtimeKind,
  unavailableMessage,
}: ExplorerMessageInput): string {
  switch (runtimeKind) {
    case "loading":
      return "Forge is still connecting to the Windows host.";
    case "read_only_recovery":
      return "The local authority store is in recovery. Existing workspace names remain visible, but file projections are blocked.";
    case "unavailable":
      // The host said why. Substituting a generic sentence would discard the
      // only specific information available.
      return unavailableMessage ?? "Forge could not reach the Windows host.";
    case "ready":
      return hasWorkspace
        ? "The host did not project the complete D1 read capability set."
        : "Choose a local workspace before opening Explorer.";
    case "browser_demo":
      return "Browser demo data is unavailable.";
  }
}
