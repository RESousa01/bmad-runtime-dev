/**
 * The host client, partitioned into domain interfaces.
 *
 * FE-11 step 1. `DesktopHostClient` is a 2,100-line facade with 48 methods, and
 * every feature that needed one of them took the whole thing. That is the
 * problem FE-10's own invariant names -- "feature controllers receive typed
 * ports, not the entire host client" -- and it is not enforceable while the
 * only type available is the facade: a controller handed the client can reach
 * anything, and a later change that reaches further is invisible in review.
 *
 * Each interface below is the authority one feature actually holds. A chat
 * controller takes `ChatClient` and can send a message and read a transcript;
 * it cannot revoke a workspace, and that is now a compile error rather than a
 * convention.
 *
 * Why `Pick` rather than hand-written signatures
 * ----------------------------------------------
 * A hand-written interface is a second declaration of every parameter and
 * return type, and second declarations drift. `Pick` makes the domain
 * interfaces *views* of the class: a signature change lands in both at once,
 * and a method that is renamed or removed fails to compile here rather than
 * silently leaving a domain pointing at nothing.
 *
 * The transport stays where it is. Only `DesktopHostClient` and the envelope
 * builders know about Tauri dispatch, request ids, or bootstrap generations;
 * nothing in this file adds a second place that does.
 */

import type { DesktopHostClient } from "../client";

/** Boot, preferences, About, and the local-data lifecycle. */
export type AppClient = Pick<DesktopHostClient,
  | "bootstrap"
  | "getAbout"
  | "getPreferences"
  | "setPreferences"
  | "inspectOffboarding"
  | "eraseOffboarding"
>;

/**
 * Workspace grants and reads inside a granted workspace.
 *
 * `enableWorkspaceEdits` lives here rather than with changes: it re-grants the
 * workspace, so it is a change to the grant, not to a file.
 */
export type WorkspaceClient = Pick<DesktopHostClient,
  | "selectWorkspace"
  | "listWorkspaces"
  | "revokeWorkspace"
  | "enableWorkspaceEdits"
  | "pickWorkspaceFiles"
  | "pickWorkspaceFolder"
  | "listWorkspaceEntries"
  | "readWorkspaceText"
  | "searchWorkspace"
  | "previewContext"
>;

/** Ordinary conversation. No BMAD concept is required to say something. */
export type ChatClient = Pick<DesktopHostClient,
  | "sendChatMessage"
  | "chatHistory"
>;

/** Proposing, deciding and rolling back governed file changes. */
export type ChangesClient = Pick<DesktopHostClient,
  | "proposeChanges"
  | "proposeCapabilityChanges"
  | "decideApproval"
  | "requestRollback"
  | "changesHistory"
>;

/** Read-only Git projections. Separate because it grants no write at all. */
export type GitClient = Pick<DesktopHostClient,
  | "readGitStatus"
  | "readGitDiff"
>;

/** The reviewed capability run: prepare, approve, cancel, submit, read back. */
export type RunsClient = Pick<DesktopHostClient,
  | "prepareCapabilityRun"
  | "approveCapabilityRun"
  | "cancelCapabilityRun"
  | "submitCapabilityRun"
  | "latestCapabilityRun"
>;

/** Which provider and model the host sends to, and whether it is authenticated. */
export type ModelClient = Pick<DesktopHostClient,
  | "modelAuthStatus"
  | "modelAuthSignIn"
  | "modelAuthSignOut"
  | "setModelSelection"
>;

/** The Foundry catalog, the resolved selection, and the response lifecycle. */
export type ModelCatalogClient = Pick<DesktopHostClient,
  | "getModelCatalog"
  | "refreshModelCatalog"
  | "getModelSelection"
  | "sendModelResponse"
  | "stopModelResponse"
>;

/** Method guidance: the reviewed BMAD request lifecycle. */
export type BmadHelpClient = Pick<DesktopHostClient,
  | "prepareBmadHelp"
  | "approveBmadHelp"
  | "cancelBmadHelp"
  | "submitBmadHelp"
  | "createBmadHelpRun"
  | "latestBmadHelpRun"
  | "bmadWorkflowState"
>;

/** Reading the method library. Projections only; grants nothing. */
export type BmadCapabilityClient = Pick<DesktopHostClient,
  | "scanBmad"
  | "bmadLibrarySnapshot"
  | "viewBmadPersona"
>;

/** Journal recovery, which is the only path that can act on a quarantined journal. */
export type RecoveryClient = Pick<DesktopHostClient,
  | "prepareChangesRecovery"
  | "decideChangesRecovery"
>;

/** The projection stream the shell polls to stay in step with the host. */
export type ProjectionClient = Pick<DesktopHostClient,
  | "projectionSnapshot"
  | "projectionEvents"
>;

/** Every method reachable through some domain interface. */
export type DomainClientMethod =
  | keyof AppClient
  | keyof WorkspaceClient
  | keyof ChatClient
  | keyof ChangesClient
  | keyof GitClient
  | keyof RunsClient
  | keyof ModelClient
  | keyof ModelCatalogClient
  | keyof BmadHelpClient
  | keyof BmadCapabilityClient
  | keyof RecoveryClient
  | keyof ProjectionClient;

/**
 * Methods on the client that no domain claims.
 *
 * Should always be `never`. A method left out is one no feature can reach
 * except by taking the whole facade again -- which is the thing this file
 * exists to stop.
 */
export type UnclaimedClientMethod = Exclude<keyof DesktopHostClient, DomainClientMethod>;

/**
 * Compile-time proof that the partition covers the client.
 *
 * If a method is added to `DesktopHostClient` without being filed into a
 * domain, `UnclaimedClientMethod` stops being `never` and this fails to
 * compile. A constraint rather than an assignment, because the constraint's
 * error *names the method*:
 *
 *   Type '"chatHistory"' does not satisfy the constraint 'never'.
 *
 * where an assignment to `never` only says `true` is not assignable, leaving
 * whoever hit it to work out which of 48 methods they forgot.
 *
 * A runtime test cannot catch this: the omission is a type-level fact with no
 * trace at runtime.
 */
type AssertNever<T extends never> = T;
export type DomainPartitionIsTotal = AssertNever<UnclaimedClientMethod>;
