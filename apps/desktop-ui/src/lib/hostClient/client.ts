import {
  type BmadHelpApprovedProjection,
  type BmadHelpCancelledProjection,
  type BmadHelpContextReviewProjection,
  type BmadHelpRunCompletedProjection,
  type ModelAuthStatusProjection,
} from "../bmadModelProjection";
import {
  type BmadHelpRunCreatedProjection,
  type BmadLibrarySnapshot,
} from "../bmadProjection";
import {
  asPositiveSafeInteger,
  parseBmadHelpApprovedReply,
  parseBmadHelpCancelledReply,
  parseBmadHelpCompletedReply,
  parseBmadHelpReviewReply,
  parseBmadHelpRunCreatedReply,
  parseBmadLibrarySnapshotReply,
  parseBmadPersonaPerspectiveReply,
  type BmadPersonaPerspective,
  parseLatestBmadHelpRunReply,
  parseModelAuthStatusReply,
} from "./bmadProtocol";
import {
  parseChangesDecisionReply,
  parseChangesHistoryReply,
  parseChangesRecoveryDecisionReply,
  parseChangesRecoveryPreparedReply,
  parseChangesReviewReply,
  parseRollbackRequestReply,
  parseWorkspaceEditsEnabledReply,
} from "./changesProtocol";
import {
  parseCapabilityApprovedReply,
  parseCapabilityCancelledReply,
  parseCapabilityCompletedReply,
  parseCapabilityReviewReply,
  parseCapabilityRunLatestReply,
} from "./capabilityProtocol";
import {
  parseAboutReply,
  parseOffboardingErasedReply,
  parsePreferencesReply,
  parseRetentionManifestReply,
} from "./appProtocol";
import {
  buildApprovalDecisionEnvelope,
  buildChangesRecoveryDecisionEnvelope,
  buildChangesRecoveryPrepareEnvelope,
  buildBmadHelpRunEnvelope,
  buildBmadModelEnvelope,
  buildCapabilityDecisionEnvelope,
  buildCapabilityLatestEnvelope,
  buildCapabilityProposeChangesEnvelope,
  buildCapabilityPrepareEnvelope,
  buildEmptyPayloadEnvelope,
  buildOffboardingEraseEnvelope,
  buildLatestBmadHelpRunEnvelope,
  buildPreferencesUpdateEnvelope,
  buildProposeChangesEnvelope,
  buildReadOnlyEnvelope,
  buildRollbackRequestEnvelope,
  buildWorkspaceEpochEnvelope,
  buildWorkspaceListEnvelope,
  buildWorkspaceRevocationEnvelope,
  buildWorkspaceFilePickEnvelope,
  buildWorkspaceSelectionEnvelope,
} from "./commandEnvelopes";
import {
  type AboutProjection,
  type ApprovalChoice,
  type CapabilityApprovedProjection,
  type CapabilityCancelledProjection,
  type CapabilityCompletedProjection,
  type CapabilityReviewProjection,
  type CapabilityRunLatestProjection,
  type OffboardingErasedProjection,
  type RetentionManifestProjection,
  type BmadScanProjection,
  type BootstrapReply,
  type DensityPreference,
  type PreferencesProjection,
  type ThemePreference,
  type ChangesDecisionProjection,
  type ChangesHistoryProjection,
  type ChangesRecoveryDecision,
  type ChangesRecoveryPrepared,
  type ChangesReviewEnvelopeProjection,
  type ContextPreviewProjection,
  HostCapabilityError,
  type LatestBmadHelpRunResult,
  localEditsLimits,
  PROJECTION_REQUEST_SCHEMA,
  type ProjectionEvent,
  type ProjectionScope,
  type ProjectionSnapshot,
  type ProposedChange,
  type RecoveryApprovalChoice,
  type RendererDispatchCommand,
  type RollbackRequestResult,
  type TauriInvoke,
  type WorkspaceEntriesProjection,
  type WorkspaceProjection,
  workspaceReadLimits,
  type WorkspaceRevocationResult,
  type WorkspaceSearchMatch,
  type WorkspaceFilePick,
  type WorkspaceSelection,
  type WorkspaceTextProjection,
} from "./contracts";
import { parseProjectionReply } from "./projectionProtocol";
import {
  asBmadCursor,
  asBmadHelpIntent,
  asContractId,
  asNullableOpaqueCursor,
  asRelativePath,
  assertUniqueRelativePaths,
  asSha256,
  asTextContent,
  asUnsignedInteger,
  fail,
  hasUnpairedSurrogate,
  utf8Length,
} from "./validation";
import {
  parseBmadScanReply,
  parseBootstrapReply,
  parseContextPreviewReply,
  parseSearchResultsReply,
  parseWorkspace,
  parseWorkspaceEntriesReply,
  parseWorkspaceListReply,
  parseWorkspaceRevocationReply,
  parseWorkspaceFilePickReply,
  parseWorkspaceSelectionReply,
  parseWorkspaceTextReply,
  sameWorkspaceIdentity,
} from "./workspaceProtocol";

export interface DesktopHostClientOptions {
  invoke: TauriInvoke;
  now?: () => number;
  requestId?: () => string;
}

export class DesktopHostClient {
  readonly #invoke: TauriInvoke;
  readonly #now: () => number;
  readonly #requestId: () => string;
  readonly #directoryCursors = new Map<
    string,
    { workspaceId: string; relativeDirectory: string }
  >();
  readonly #directoryEntryPaths = new Map<string, Set<string>>();
  readonly #pendingDirectoryCursors = new Set<string>();
  #bootstrap: BootstrapReply | null = null;
  #bootstrapAttempt = 0;
  #bootstrapGeneration = 0;
  #projectionSequence: number | null = null;
  #pendingRecovery: {
    review: Extract<ChangesRecoveryPrepared, { status: "review_required" }>;
    workspaceId: string;
    workspaceGrantEpoch: number;
  } | null = null;

  constructor({
    invoke,
    now = Date.now,
    requestId = () => crypto.randomUUID(),
  }: DesktopHostClientOptions) {
    this.#invoke = invoke;
    this.#now = now;
    this.#requestId = requestId;
  }

  async bootstrap(): Promise<BootstrapReply> {
    const attempt = this.#bootstrapAttempt + 1;
    this.#bootstrapAttempt = attempt;
    this.#bootstrapGeneration += 1;
    this.#bootstrap = null;
    this.#projectionSequence = null;
    this.#directoryCursors.clear();
    this.#directoryEntryPaths.clear();
    this.#pendingDirectoryCursors.clear();
    this.#pendingRecovery = null;
    const reply = parseBootstrapReply(await this.#invoke("host_bootstrap"));
    if (attempt !== this.#bootstrapAttempt) {
      return fail();
    }
    this.#bootstrap = reply;
    this.#projectionSequence = reply.projectionSequence;
    return reply;
  }

  async selectWorkspace(): Promise<WorkspaceSelection> {
    const bootstrap = this.requireBootstrap();
    const bootstrapGeneration = this.#bootstrapGeneration;
    if (
      bootstrap.bootMode !== "ready" ||
      !bootstrap.supportedCommands.includes("workspace.select_folder")
    ) {
      throw new HostCapabilityError(
        "Folder selection is unavailable in the current host mode.",
      );
    }
    const requestId = this.#requestId();
    const envelope = buildWorkspaceSelectionEnvelope(
      bootstrap,
      requestId,
      this.#now(),
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseWorkspaceSelectionReply(reply, requestId);
    const currentBootstrap =
      this.requireBootstrapGeneration(bootstrapGeneration);
    if (
      currentBootstrap.bootMode !== "ready" ||
      !currentBootstrap.supportedCommands.includes("workspace.select_folder")
    ) {
      throw new HostCapabilityError(
        "Folder selection is unavailable in the current host mode.",
      );
    }
    this.advanceProjectionSequence(parsed.sequence);
    if (parsed.selection.kind === "workspace_selected") {
      const selectedWorkspace = parsed.selection.value;
      this.replaceWorkspaces([
        selectedWorkspace,
        ...currentBootstrap.workspaces.filter(
          ({ workspaceId }) => workspaceId !== selectedWorkspace.workspaceId,
        ),
      ]);
    }
    return parsed.selection;
  }

  async pickWorkspaceFiles(workspaceId: string): Promise<WorkspaceFilePick> {
    const bootstrap = this.requireCommand("workspace.pick_files");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildWorkspaceFilePickEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      workspaceId,
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseWorkspaceFilePickReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireCommand("workspace.pick_files");
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.pick;
  }

  async listWorkspaces(): Promise<WorkspaceProjection[]> {
    const bootstrap = this.requireBootstrap();
    const bootstrapGeneration = this.#bootstrapGeneration;
    if (!bootstrap.supportedCommands.includes("workspace.list")) {
      throw new HostCapabilityError(
        "Local workspace status is unavailable in the current host mode.",
      );
    }
    const requestId = this.#requestId();
    const envelope = buildWorkspaceListEnvelope(
      bootstrap,
      requestId,
      this.#now(),
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseWorkspaceListReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(parsed.sequence);
    this.replaceWorkspaces(parsed.workspaces);
    return parsed.workspaces;
  }

  async revokeWorkspace(
    expectedWorkspaceValue: WorkspaceProjection,
  ): Promise<WorkspaceRevocationResult> {
    const bootstrap = this.requireBootstrap();
    const bootstrapGeneration = this.#bootstrapGeneration;
    const expectedWorkspace = parseWorkspace(expectedWorkspaceValue);
    const currentWorkspace = bootstrap.workspaces.find(
      ({ workspaceId }) => workspaceId === expectedWorkspace.workspaceId,
    );
    if (
      bootstrap.bootMode !== "ready" ||
      !bootstrap.supportedCommands.includes("workspace.revoke") ||
      !currentWorkspace ||
      currentWorkspace.grantEpoch !== expectedWorkspace.grantEpoch ||
      !sameWorkspaceIdentity(currentWorkspace, expectedWorkspace)
    ) {
      throw new HostCapabilityError(
        "That workspace access is no longer available.",
      );
    }

    const requestId = this.#requestId();
    const envelope = buildWorkspaceRevocationEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      expectedWorkspace.workspaceId,
    );
    const issuedAfterSequence = this.#projectionSequence;
    if (issuedAfterSequence === null) {
      return fail();
    }
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseWorkspaceRevocationReply(
      reply,
      requestId,
      expectedWorkspace,
    );

    const currentBootstrap =
      this.requireBootstrapGeneration(bootstrapGeneration);
    const stillCurrent = currentBootstrap.workspaces.find(
      ({ workspaceId }) => workspaceId === expectedWorkspace.workspaceId,
    );
    if (
      stillCurrent &&
      (stillCurrent.grantEpoch !== expectedWorkspace.grantEpoch ||
        !sameWorkspaceIdentity(stillCurrent, expectedWorkspace))
    ) {
      return fail();
    }

    // Validation and replay checks must complete before local authority state changes.
    this.advanceMutationSequence(parsed.sequence, issuedAfterSequence);
    this.clearWorkspaceTraversal(expectedWorkspace.workspaceId);
    const workspaces = currentBootstrap.workspaces.filter(
      ({ workspaceId }) => workspaceId !== expectedWorkspace.workspaceId,
    );
    this.replaceWorkspaces(workspaces);
    return { revoked: parsed.revoked, workspaces: [...workspaces] };
  }

  async listWorkspaceEntries(
    workspaceId: string,
    cursor: string | null = null,
    limit = 100,
  ): Promise<WorkspaceEntriesProjection> {
    const bootstrap = this.requireWorkspaceCommand(
      workspaceId,
      "workspace.list_entries",
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    if (
      !Number.isInteger(limit) ||
      limit < 1 ||
      limit > workspaceReadLimits.entryPage
    ) {
      throw new HostCapabilityError(
        "The requested Explorer page size is outside the desktop limit.",
      );
    }
    const cursorBinding =
      cursor === null
        ? { workspaceId, relativeDirectory: "." }
        : this.requireDirectoryCursor(cursor, workspaceId);
    if (cursor !== null) {
      if (this.#pendingDirectoryCursors.has(cursor)) {
        throw new HostCapabilityError(
          "That Explorer page is already being requested.",
        );
      }
      this.#pendingDirectoryCursors.add(cursor);
    }
    const requestId = this.#requestId();
    const envelope = buildReadOnlyEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "workspace.list_entries",
      { workspaceId, cursor, limit },
    );
    let reply: unknown;
    try {
      reply = await this.#invoke("host_dispatch", {
        body: JSON.stringify(envelope),
      });
    } finally {
      if (cursor !== null) {
        this.#pendingDirectoryCursors.delete(cursor);
        this.#directoryCursors.delete(cursor);
      }
    }
    const parsed = parseWorkspaceEntriesReply(reply, requestId, {
      workspaceId,
      relativeDirectory: cursorBinding.relativeDirectory,
      limit,
    });
    // A concurrent revocation must prevent an in-flight page from restoring stale cursors.
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireWorkspaceCommand(workspaceId, "workspace.list_entries");
    const isFreshRootPage = cursor === null;
    const directoryKey = this.directoryKey(
      workspaceId,
      cursorBinding.relativeDirectory,
    );
    const observedPaths = isFreshRootPage
      ? new Set<string>()
      : new Set(this.#directoryEntryPaths.get(directoryKey) ?? []);
    for (const entry of parsed.projection.entries) {
      const foldedPath = entry.relativePath.toLocaleLowerCase("en-US");
      if (observedPaths.has(foldedPath)) {
        fail();
      }
      observedPaths.add(foldedPath);
    }
    if (
      (cursor !== null && parsed.projection.nextCursor === cursor) ||
      (parsed.projection.entries.length === 0 &&
        parsed.projection.nextCursor !== null)
    ) {
      fail();
    }

    const projectedCursorBindings = new Map<
      string,
      { workspaceId: string; relativeDirectory: string }
    >();
    if (parsed.projection.nextCursor) {
      projectedCursorBindings.set(parsed.projection.nextCursor, cursorBinding);
    }
    for (const entry of parsed.projection.entries) {
      if (!entry.childCursor) {
        continue;
      }
      const childBinding = {
        workspaceId,
        relativeDirectory: entry.relativePath,
      };
      const projectedBinding = projectedCursorBindings.get(entry.childCursor);
      if (
        projectedBinding &&
        (projectedBinding.workspaceId !== childBinding.workspaceId ||
          projectedBinding.relativeDirectory !== childBinding.relativeDirectory)
      ) {
        fail();
      }
      projectedCursorBindings.set(entry.childCursor, childBinding);
    }
    for (const [projectedCursor, projectedBinding] of projectedCursorBindings) {
      const existingBinding = this.#directoryCursors.get(projectedCursor);
      if (
        existingBinding &&
        !(isFreshRootPage && existingBinding.workspaceId === workspaceId) &&
        (existingBinding.workspaceId !== projectedBinding.workspaceId ||
          existingBinding.relativeDirectory !==
            projectedBinding.relativeDirectory)
      ) {
        fail();
      }
    }

    this.advanceProjectionSequence(parsed.sequence);
    if (isFreshRootPage) {
      this.clearWorkspaceTraversal(workspaceId);
    }
    this.#directoryEntryPaths.set(directoryKey, observedPaths);
    for (const [projectedCursor, projectedBinding] of projectedCursorBindings) {
      this.#directoryCursors.set(projectedCursor, projectedBinding);
    }
    return parsed.projection;
  }

  async readWorkspaceText(
    workspaceId: string,
    relativePathValue: string,
    maxBytes = 128 * 1024,
  ): Promise<WorkspaceTextProjection> {
    const bootstrap = this.requireWorkspaceCommand(
      workspaceId,
      "workspace.read_text",
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const relativePath = asRelativePath(relativePathValue);
    if (
      !Number.isInteger(maxBytes) ||
      maxBytes < 1 ||
      maxBytes > workspaceReadLimits.readBytes
    ) {
      throw new HostCapabilityError(
        "The requested text preview size is outside the desktop limit.",
      );
    }
    const requestId = this.#requestId();
    const envelope = buildReadOnlyEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "workspace.read_text",
      { workspaceId, relativePath, maxBytes },
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseWorkspaceTextReply(
      reply,
      requestId,
      relativePath,
      maxBytes,
    );
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireWorkspaceCommand(workspaceId, "workspace.read_text");
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async searchWorkspace(
    workspaceId: string,
    queryValue: string,
    maxResults = 100,
  ): Promise<WorkspaceSearchMatch[]> {
    const bootstrap = this.requireWorkspaceCommand(
      workspaceId,
      "workspace.search",
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const query = queryValue.trim();
    if (
      query.length === 0 ||
      query.includes("\0") ||
      hasUnpairedSurrogate(query) ||
      utf8Length(query) > workspaceReadLimits.searchQueryBytes ||
      !Number.isInteger(maxResults) ||
      maxResults < 1 ||
      maxResults > workspaceReadLimits.searchResults
    ) {
      throw new HostCapabilityError(
        "The search request is outside the desktop read limits.",
      );
    }
    const requestId = this.#requestId();
    const envelope = buildReadOnlyEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "workspace.search",
      { workspaceId, query, maxResults },
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseSearchResultsReply(reply, requestId, maxResults);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireWorkspaceCommand(workspaceId, "workspace.search");
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.matches;
  }

  async scanBmad(workspaceId: string): Promise<BmadScanProjection> {
    const bootstrap = this.requireWorkspaceCommand(workspaceId, "bmad.scan");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildReadOnlyEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "bmad.scan",
      { workspaceId },
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseBmadScanReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireWorkspaceCommand(workspaceId, "bmad.scan");
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async bmadLibrarySnapshot(
    cursor?: string | null,
  ): Promise<BmadLibrarySnapshot> {
    const bootstrap = this.requireBmadLibraryCommand();
    const bootstrapGeneration = this.#bootstrapGeneration;
    const normalizedCursor = asBmadCursor(cursor ?? null);
    const requestId = this.#requestId();
    const envelope = buildReadOnlyEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "bmad.library.snapshot",
      { scope: "installed_method", cursor: normalizedCursor },
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseBmadLibrarySnapshotReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireBmadLibraryCommand();
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async viewBmadPersona(agentCode: string): Promise<BmadPersonaPerspective> {
    const bootstrap = this.requireCommand("bmad.persona.view");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildReadOnlyEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "bmad.persona.view",
      { agentCode },
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseBmadPersonaPerspectiveReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async getPreferences(): Promise<PreferencesProjection> {
    const bootstrap = this.requireCommand("app.preferences.get");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildEmptyPayloadEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "app.preferences.get",
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parsePreferencesReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async setPreferences(
    theme: ThemePreference,
    density: DensityPreference,
  ): Promise<PreferencesProjection> {
    const bootstrap = this.requireCommand("app.preferences.set");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildPreferencesUpdateEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      theme,
      density,
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parsePreferencesReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async getAbout(): Promise<AboutProjection> {
    const bootstrap = this.requireCommand("app.about");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildEmptyPayloadEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "app.about",
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseAboutReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async inspectOffboarding(): Promise<RetentionManifestProjection> {
    const bootstrap = this.requireCommand("app.offboarding.inspect");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildEmptyPayloadEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "app.offboarding.inspect",
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseRetentionManifestReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  // Irreversible: the host validates the exact confirmation phrase and
  // drops to read-only recovery after the erase, so no bootstrap or
  // sequence expectations survive the reply.
  async eraseOffboarding(confirm: string): Promise<OffboardingErasedProjection> {
    const bootstrap = this.requireCommand("app.offboarding.erase");
    const requestId = this.#requestId();
    const envelope = buildOffboardingEraseEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      confirm,
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseOffboardingErasedReply(reply, requestId);
    return parsed.projection;
  }

  async prepareCapabilityRun(
    workspaceId: string,
    workspaceGrantEpoch: number,
    capabilityId: string,
    contextPaths: string[],
  ): Promise<CapabilityReviewProjection> {
    const bootstrap = this.requireCommand("bmad.capability.prepare");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildCapabilityPrepareEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      workspaceId,
      workspaceGrantEpoch,
      capabilityId,
      contextPaths,
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseCapabilityReviewReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async approveCapabilityRun(
    workspaceId: string,
    workspaceGrantEpoch: number,
    capabilityId: string,
    manifestHash: string,
  ): Promise<CapabilityApprovedProjection> {
    const bootstrap = this.requireCommand("bmad.capability.approve");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildCapabilityDecisionEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "bmad.capability.approve",
      { workspaceId, workspaceGrantEpoch, capabilityId, manifestHash },
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseCapabilityApprovedReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async cancelCapabilityRun(
    workspaceId: string,
    workspaceGrantEpoch: number,
    capabilityId: string,
    manifestHash: string,
    decisionId: string,
  ): Promise<CapabilityCancelledProjection> {
    const bootstrap = this.requireCommand("bmad.capability.cancel");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildCapabilityDecisionEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "bmad.capability.cancel",
      { workspaceId, workspaceGrantEpoch, capabilityId, manifestHash, decisionId },
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseCapabilityCancelledReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async submitCapabilityRun(
    workspaceId: string,
    workspaceGrantEpoch: number,
    capabilityId: string,
    manifestHash: string,
    decisionId: string,
  ): Promise<CapabilityCompletedProjection> {
    const bootstrap = this.requireCommand("bmad.capability.submit");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildCapabilityDecisionEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "bmad.capability.submit",
      { workspaceId, workspaceGrantEpoch, capabilityId, manifestHash, decisionId },
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseCapabilityCompletedReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async latestCapabilityRun(
    workspaceId: string,
    workspaceGrantEpoch: number,
    capabilityId: string,
  ): Promise<CapabilityRunLatestProjection> {
    const bootstrap = this.requireCommand("bmad.capability.latest");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildCapabilityLatestEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      workspaceId,
      workspaceGrantEpoch,
      capabilityId,
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseCapabilityRunLatestReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  private requireCommand(command: RendererDispatchCommand): BootstrapReply {
    const bootstrap = this.requireBootstrap();
    if (
      bootstrap.bootMode !== "ready" ||
      !(bootstrap.supportedCommands as readonly string[]).includes(command)
    ) {
      throw new HostCapabilityError(
        "The command is unavailable in the current host mode.",
      );
    }
    return bootstrap;
  }

  async modelAuthStatus(): Promise<ModelAuthStatusProjection> {
    return this.dispatchModelAuthCommand("model.auth.status");
  }

  async modelAuthSignIn(): Promise<ModelAuthStatusProjection> {
    return this.dispatchModelAuthCommand("model.auth.sign_in");
  }

  async modelAuthSignOut(): Promise<ModelAuthStatusProjection> {
    return this.dispatchModelAuthCommand("model.auth.sign_out");
  }

  async prepareBmadHelp(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
  ): Promise<BmadHelpContextReviewProjection> {
    const command = "bmad.help.prepare" as const;
    const bootstrap = this.requireBmadHelpWorkspaceCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      command,
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    const requestId = this.#requestId();
    const envelope = buildBmadModelEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      command,
      {
        workspaceId,
        workspaceGrantEpoch: asPositiveSafeInteger(workspaceGrantEpoch),
      },
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseBmadHelpReviewReply(reply, requestId, workspaceId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireBmadHelpWorkspaceCommand(
      workspaceId,
      workspaceGrantEpoch,
      command,
    );
    if (parsed.projection.workspaceGrantEpoch !== workspaceGrantEpoch)
      return fail();
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async approveBmadHelp(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
    manifestHashValue: string,
  ): Promise<BmadHelpApprovedProjection> {
    const command = "bmad.help.approve" as const;
    const bootstrap = this.requireBmadHelpWorkspaceCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      command,
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    const manifestHash = asSha256(manifestHashValue);
    const requestId = this.#requestId();
    const envelope = buildBmadModelEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      command,
      {
        workspaceId,
        workspaceGrantEpoch: asPositiveSafeInteger(workspaceGrantEpoch),
        manifestHash,
      },
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseBmadHelpApprovedReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireBmadHelpWorkspaceCommand(
      workspaceId,
      workspaceGrantEpoch,
      command,
    );
    if (parsed.projection.manifestHash !== manifestHash) return fail();
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async cancelBmadHelp(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
    manifestHashValue: string,
    decisionIdValue: string,
  ): Promise<BmadHelpCancelledProjection> {
    const command = "bmad.help.cancel" as const;
    const bootstrap = this.requireBmadHelpWorkspaceCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      command,
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    const manifestHash = asSha256(manifestHashValue);
    const decisionId = asContractId(decisionIdValue);
    const requestId = this.#requestId();
    const envelope = buildBmadModelEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      command,
      {
        workspaceId,
        workspaceGrantEpoch: asPositiveSafeInteger(workspaceGrantEpoch),
        manifestHash,
        decisionId,
      },
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseBmadHelpCancelledReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireBmadHelpWorkspaceCommand(
      workspaceId,
      workspaceGrantEpoch,
      command,
    );
    if (
      parsed.projection.manifestHash !== manifestHash ||
      parsed.projection.decisionId !== decisionId
    )
      return fail();
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async submitBmadHelp(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
    manifestHashValue: string,
    decisionIdValue: string,
  ): Promise<BmadHelpRunCompletedProjection> {
    const command = "bmad.help.submit" as const;
    const bootstrap = this.requireBmadHelpWorkspaceCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      command,
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    const requestId = this.#requestId();
    const envelope = buildBmadModelEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      command,
      {
        workspaceId,
        workspaceGrantEpoch: asPositiveSafeInteger(workspaceGrantEpoch),
        manifestHash: asSha256(manifestHashValue),
        decisionId: asContractId(decisionIdValue),
      },
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseBmadHelpCompletedReply(reply, requestId, workspaceId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireBmadHelpWorkspaceCommand(
      workspaceId,
      workspaceGrantEpoch,
      command,
    );
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async createBmadHelpRun(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
    currentIntentValue: string,
  ): Promise<BmadHelpRunCreatedProjection> {
    const bootstrap = this.requireBmadHelpWorkspaceCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      "run.create",
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    const currentIntent = asBmadHelpIntent(currentIntentValue);
    const requestId = this.#requestId();
    const envelope = buildBmadHelpRunEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      workspaceId,
      workspaceGrantEpoch,
      currentIntent,
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseBmadHelpRunCreatedReply(reply, requestId, workspaceId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireBmadHelpWorkspaceCommand(
      workspaceId,
      workspaceGrantEpoch,
      "run.create",
    );
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async latestBmadHelpRun(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
  ): Promise<LatestBmadHelpRunResult> {
    const bootstrap = this.requireBmadHelpWorkspaceCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      "bmad.help.latest",
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    const requestId = this.#requestId();
    const envelope = buildLatestBmadHelpRunEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      workspaceId,
      workspaceGrantEpoch,
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseLatestBmadHelpRunReply(
      reply,
      requestId,
      workspaceId,
      workspaceGrantEpoch,
    );
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireBmadHelpWorkspaceCommand(
      workspaceId,
      workspaceGrantEpoch,
      "bmad.help.latest",
    );
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.result;
  }

  async enableWorkspaceEdits(
    expectedWorkspaceValue: WorkspaceProjection,
  ): Promise<WorkspaceProjection> {
    const bootstrap = this.requireBootstrap();
    const bootstrapGeneration = this.#bootstrapGeneration;
    const expectedWorkspace = parseWorkspace(expectedWorkspaceValue);
    const currentWorkspace = bootstrap.workspaces.find(
      ({ workspaceId }) => workspaceId === expectedWorkspace.workspaceId,
    );
    if (
      bootstrap.bootMode !== "ready" ||
      !bootstrap.supportedCommands.includes("workspace.enable_edits") ||
      !currentWorkspace ||
      currentWorkspace.grantEpoch !== expectedWorkspace.grantEpoch ||
      currentWorkspace.permissions !== "read_only" ||
      !sameWorkspaceIdentity(currentWorkspace, expectedWorkspace)
    ) {
      throw new HostCapabilityError(
        "Governed edits cannot be enabled for that workspace.",
      );
    }
    const requestId = this.#requestId();
    const envelope = buildWorkspaceEpochEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "workspace.enable_edits",
      expectedWorkspace.workspaceId,
      expectedWorkspace.grantEpoch,
    );
    const issuedAfterSequence = this.#projectionSequence;
    if (issuedAfterSequence === null) {
      return fail();
    }
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseWorkspaceEditsEnabledReply(
      reply,
      requestId,
      expectedWorkspace,
    );
    const currentBootstrap =
      this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceMutationSequence(parsed.sequence, issuedAfterSequence);
    this.clearWorkspaceTraversal(expectedWorkspace.workspaceId);
    this.replaceWorkspaces([
      parsed.projection,
      ...currentBootstrap.workspaces.filter(
        ({ workspaceId }) => workspaceId !== parsed.projection.workspaceId,
      ),
    ]);
    return parsed.projection;
  }

  async proposeChanges(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
    changesValue: readonly ProposedChange[],
  ): Promise<ChangesReviewEnvelopeProjection> {
    const bootstrap = this.requireGovernedEditsCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      "changes.propose",
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    if (
      changesValue.length === 0 ||
      changesValue.length > localEditsLimits.changesPerProposal
    ) {
      throw new HostCapabilityError(
        "Propose between 1 and 20 file changes in a single review.",
      );
    }
    const changes = changesValue.map((change): ProposedChange => {
      const relativePath = asRelativePath(change.relativePath);
      if (change.change === "set_content") {
        return {
          change: "set_content",
          relativePath,
          content: asTextContent(
            change.content,
            localEditsLimits.changeContentBytes,
          ),
        };
      }
      return { change: "delete", relativePath };
    });
    assertUniqueRelativePaths(changes.map(({ relativePath }) => relativePath));
    const requestId = this.#requestId();
    const envelope = buildProposeChangesEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      workspaceId,
      workspaceGrantEpoch,
      changes,
    );
    const issuedAfterSequence = this.#projectionSequence;
    if (issuedAfterSequence === null) {
      return fail();
    }
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseChangesReviewReply(reply, requestId, {
      workspaceId,
      workspaceGrantEpoch,
      proposalKind: "edit",
      sourceExecutionId: null,
    });
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceMutationSequence(parsed.sequence, issuedAfterSequence);
    return parsed.projection;
  }

  async proposeCapabilityChanges(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
    capabilityId: string,
  ): Promise<ChangesReviewEnvelopeProjection> {
    const bootstrap = this.requireGovernedEditsCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      "bmad.capability.propose_changes",
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    const requestId = this.#requestId();
    const envelope = buildCapabilityProposeChangesEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      workspaceId,
      workspaceGrantEpoch,
      capabilityId,
    );
    const issuedAfterSequence = this.#projectionSequence;
    if (issuedAfterSequence === null) {
      return fail();
    }
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseChangesReviewReply(reply, requestId, {
      workspaceId,
      workspaceGrantEpoch,
      proposalKind: "edit",
      sourceExecutionId: null,
    });
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceMutationSequence(parsed.sequence, issuedAfterSequence);
    return parsed.projection;
  }

  async decideApproval(
    review: ChangesReviewEnvelopeProjection,
    choice: ApprovalChoice,
  ): Promise<ChangesDecisionProjection> {
    const bootstrap = this.requireBootstrap();
    const bootstrapGeneration = this.#bootstrapGeneration;
    if (
      bootstrap.bootMode !== "ready" ||
      !bootstrap.supportedCommands.includes("approval.decide")
    ) {
      throw new HostCapabilityError(
        "Change decisions are unavailable in the current host mode.",
      );
    }
    const requestId = this.#requestId();
    const envelope = buildApprovalDecisionEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      review.approvalId,
      review.review.candidateHash,
      review.displayedDiffHash,
      choice,
    );
    const issuedAfterSequence = this.#projectionSequence;
    if (issuedAfterSequence === null) {
      return fail();
    }
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseChangesDecisionReply(reply, requestId, {
      approvalId: review.approvalId,
      choice,
    });
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceMutationSequence(parsed.sequence, issuedAfterSequence);
    return parsed.projection;
  }

  async requestRollback(
    executionIdValue: string,
  ): Promise<RollbackRequestResult> {
    const bootstrap = this.requireBootstrap();
    const bootstrapGeneration = this.#bootstrapGeneration;
    const executionId = asContractId(executionIdValue);
    if (
      bootstrap.bootMode !== "ready" ||
      !bootstrap.supportedCommands.includes("rollback.request")
    ) {
      throw new HostCapabilityError(
        "Undo changes is unavailable in the current host mode.",
      );
    }
    const requestId = this.#requestId();
    const envelope = buildRollbackRequestEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      executionId,
    );
    const issuedAfterSequence = this.#projectionSequence;
    if (issuedAfterSequence === null) {
      return fail();
    }
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseRollbackRequestReply(reply, requestId, executionId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceMutationSequence(parsed.sequence, issuedAfterSequence);
    return parsed.result;
  }

  async changesHistory(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
  ): Promise<ChangesHistoryProjection> {
    this.#pendingRecovery = null;
    const bootstrap = this.requireGovernedEditsCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      "changes.history",
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    const requestId = this.#requestId();
    const envelope = buildWorkspaceEpochEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "changes.history",
      workspaceId,
      workspaceGrantEpoch,
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseChangesHistoryReply(reply, requestId, workspaceId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async prepareChangesRecovery(input: {
    workspaceId: string;
    workspaceGrantEpoch: number;
    journalId: string;
  }): Promise<ChangesRecoveryPrepared> {
    this.#pendingRecovery = null;
    const bootstrap = this.requireGovernedEditsCommand(
      input.workspaceId,
      input.workspaceGrantEpoch,
      "changes.recovery.prepare",
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(input.workspaceId);
    const journalId = asContractId(input.journalId);
    const requestId = this.#requestId();
    const issuedAt = this.#now();
    const envelope = buildChangesRecoveryPrepareEnvelope(
      bootstrap,
      requestId,
      issuedAt,
      workspaceId,
      input.workspaceGrantEpoch,
      journalId,
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseChangesRecoveryPreparedReply(reply, requestId, journalId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireGovernedEditsCommand(
      workspaceId,
      input.workspaceGrantEpoch,
      "changes.recovery.prepare",
    );
    this.advanceProjectionSequence(parsed.sequence);
    if (parsed.projection.status === "review_required") {
      if (parsed.projection.expiresAt <= issuedAt) {
        return fail();
      }
      this.#pendingRecovery = {
        review: parsed.projection,
        workspaceId,
        workspaceGrantEpoch: input.workspaceGrantEpoch,
      };
    }
    return parsed.projection;
  }

  async decideChangesRecovery(input: {
    recoveryApprovalId: string;
    displayedRecoveryHash: string;
    choice: RecoveryApprovalChoice;
  }): Promise<ChangesRecoveryDecision> {
    const pending = this.#pendingRecovery;
    this.#pendingRecovery = null;
    const decisionAt = this.#now();
    if (
      pending === null
      || decisionAt >= pending.review.expiresAt
      || input.recoveryApprovalId !== pending.review.recoveryApprovalId
      || input.displayedRecoveryHash !== pending.review.displayedRecoveryHash
      || (input.choice !== "restore" && input.choice !== "cancel")
    ) {
      throw new HostCapabilityError(
        "That recovery review is no longer current; prepare recovery again.",
      );
    }
    const bootstrap = this.requireGovernedEditsCommand(
      pending.workspaceId,
      pending.workspaceGrantEpoch,
      "changes.recovery.decide",
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const issuedAfterSequence = this.#projectionSequence;
    if (issuedAfterSequence === null) {
      return fail();
    }
    const requestId = this.#requestId();
    const envelope = buildChangesRecoveryDecisionEnvelope(
      bootstrap,
      requestId,
      decisionAt,
      input.recoveryApprovalId,
      input.displayedRecoveryHash,
      input.choice,
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseChangesRecoveryDecisionReply(reply, requestId, {
      recoveryApprovalId: pending.review.recoveryApprovalId,
      journalId: pending.review.journalId,
      executionId: pending.review.executionId,
      choice: input.choice,
      operationCount: pending.review.operations.length,
    });
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireGovernedEditsCommand(
      pending.workspaceId,
      pending.workspaceGrantEpoch,
      "changes.recovery.decide",
    );
    if (input.choice === "restore") {
      this.advanceMutationSequence(parsed.sequence, issuedAfterSequence);
    } else {
      this.advanceProjectionSequence(parsed.sequence);
    }
    return parsed.projection;
  }

  async previewContext(
    workspaceId: string,
    relativePathValues: readonly string[],
  ): Promise<ContextPreviewProjection> {
    const bootstrap = this.requireWorkspaceCommand(
      workspaceId,
      "context.preview",
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    if (
      relativePathValues.length === 0 ||
      relativePathValues.length > workspaceReadLimits.contextPaths
    ) {
      throw new HostCapabilityError(
        "Select between 1 and 100 text files for context review.",
      );
    }
    const relativePaths = relativePathValues.map(asRelativePath);
    assertUniqueRelativePaths(relativePaths);
    const requestId = this.#requestId();
    const envelope = buildReadOnlyEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "context.preview",
      { workspaceId, relativePaths },
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseContextPreviewReply(
      reply,
      requestId,
      workspaceId,
      relativePaths,
    );
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireWorkspaceCommand(workspaceId, "context.preview");
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async projectionSnapshot(
    scope: ProjectionScope = {},
  ): Promise<ProjectionSnapshot> {
    const bootstrap = this.requireBootstrap();
    const bootstrapGeneration = this.#bootstrapGeneration;
    const request = this.buildProjectionRequest(scope, null);
    const reply = await this.#invoke("host_projection_snapshot", {
      body: JSON.stringify(request),
    });
    const snapshot = parseProjectionReply(
      reply,
      bootstrap.rendererSessionId,
      "snapshot",
    ) as ProjectionSnapshot;
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(snapshot.sequence);
    if (snapshot.bootMode === "read_only_recovery") {
      this.adoptReadOnlyRecovery(snapshot.sequence);
    }
    return snapshot;
  }

  async projectionEvents(
    afterSequence: number,
    scope: ProjectionScope = {},
  ): Promise<ProjectionEvent[]> {
    const bootstrap = this.requireBootstrap();
    const bootstrapGeneration = this.#bootstrapGeneration;
    const request = this.buildProjectionRequest(
      scope,
      asUnsignedInteger(afterSequence),
    );
    const reply = await this.#invoke("host_projection_events", {
      body: JSON.stringify(request),
    });
    const events = parseProjectionReply(
      reply,
      bootstrap.rendererSessionId,
      "events",
    ) as ProjectionEvent[];
    this.requireBootstrapGeneration(bootstrapGeneration);
    if (events.some(({ sequence }) => sequence <= afterSequence)) {
      return fail();
    }
    if (events.length > 0) {
      this.advanceProjectionSequence(events.at(-1)!.sequence);
    }
    const recoveryEvent = events.find(
      ({ event }) =>
        event.type === "boot_state_changed" &&
        event.projection.mode === "read_only_recovery",
    );
    if (recoveryEvent) {
      this.adoptReadOnlyRecovery(recoveryEvent.sequence);
    }
    return events;
  }

  private requireWorkspaceCommand(
    workspaceIdValue: string,
    command: Extract<
      RendererDispatchCommand,
      | "workspace.list_entries"
      | "workspace.read_text"
      | "workspace.search"
      | "bmad.scan"
      | "context.preview"
    >,
  ): BootstrapReply {
    const bootstrap = this.requireBootstrap();
    const workspaceId = asContractId(workspaceIdValue);
    if (
      bootstrap.bootMode !== "ready" ||
      !bootstrap.supportedCommands.includes(command) ||
      !bootstrap.workspaces.some(
        (workspace) => workspace.workspaceId === workspaceId,
      )
    ) {
      throw new HostCapabilityError(
        "That read-only workspace capability is unavailable.",
      );
    }
    return bootstrap;
  }

  private requireGovernedEditsCommand(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
    command: Extract<
      RendererDispatchCommand,
      | "changes.propose"
      | "changes.history"
      | "changes.recovery.prepare"
      | "changes.recovery.decide"
      | "bmad.capability.propose_changes"
    >,
  ): BootstrapReply {
    const bootstrap = this.requireBootstrap();
    const workspaceId = asContractId(workspaceIdValue);
    if (
      bootstrap.bootMode !== "ready" ||
      !bootstrap.supportedCommands.includes(command) ||
      !Number.isSafeInteger(workspaceGrantEpoch) ||
      workspaceGrantEpoch < 1 ||
      !bootstrap.workspaces.some(
        (workspace) =>
          workspace.workspaceId === workspaceId &&
          workspace.grantEpoch === workspaceGrantEpoch &&
          workspace.permissions === "governed_edits",
      )
    ) {
      throw new HostCapabilityError(
        "Governed edits are not enabled for that workspace at the current grant.",
      );
    }
    return bootstrap;
  }

  private requireBmadLibraryCommand(): BootstrapReply {
    const bootstrap = this.requireBootstrap();
    if (
      bootstrap.bootMode !== "ready" ||
      !bootstrap.supportedCommands.includes("bmad.library.snapshot")
    ) {
      throw new HostCapabilityError(
        "Skills and agents are unavailable in the current host mode.",
      );
    }
    return bootstrap;
  }

  private async dispatchModelAuthCommand(
    command: "model.auth.status" | "model.auth.sign_in" | "model.auth.sign_out",
  ): Promise<ModelAuthStatusProjection> {
    const bootstrap = this.requireModelAuthCommand(command);
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildBmadModelEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      command,
      {},
    );
    const reply = await this.#invoke("host_dispatch", {
      body: JSON.stringify(envelope),
    });
    const parsed = parseModelAuthStatusReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireModelAuthCommand(command);
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  private requireModelAuthCommand(
    command: "model.auth.status" | "model.auth.sign_in" | "model.auth.sign_out",
  ): BootstrapReply {
    const bootstrap = this.requireBootstrap();
    if (
      bootstrap.bootMode !== "ready" ||
      !bootstrap.supportedCommands.includes(command)
    ) {
      throw new HostCapabilityError(
        "Model identity is unavailable in the current host mode.",
      );
    }
    return bootstrap;
  }

  private requireBmadHelpWorkspaceCommand(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
    command:
      | "bmad.help.prepare"
      | "bmad.help.approve"
      | "bmad.help.cancel"
      | "bmad.help.submit"
      | "bmad.help.latest"
      | "run.create",
  ): BootstrapReply {
    const bootstrap = this.requireBootstrap();
    const workspaceId = asContractId(workspaceIdValue);
    if (
      !Number.isSafeInteger(workspaceGrantEpoch) ||
      workspaceGrantEpoch < 1 ||
      bootstrap.bootMode !== "ready" ||
      !bootstrap.supportedCommands.includes(command) ||
      !bootstrap.workspaces.some(
        (workspace) =>
          workspace.workspaceId === workspaceId &&
          workspace.grantEpoch === workspaceGrantEpoch &&
          workspace.permissions === "read_only",
      )
    ) {
      throw new HostCapabilityError(
        "Method guidance is unavailable for that workspace grant.",
      );
    }
    return bootstrap;
  }

  private requireDirectoryCursor(
    cursorValue: string,
    workspaceId: string,
  ): { workspaceId: string; relativeDirectory: string } {
    const cursor = asNullableOpaqueCursor(cursorValue);
    if (cursor === null) {
      return fail();
    }
    const binding = this.#directoryCursors.get(cursor);
    if (!binding || binding.workspaceId !== workspaceId) {
      throw new HostCapabilityError(
        "The Explorer page cursor is unavailable; refresh the workspace.",
      );
    }
    return binding;
  }

  private directoryKey(workspaceId: string, relativeDirectory: string): string {
    return `${workspaceId}\u001f${relativeDirectory}`;
  }

  private clearWorkspaceTraversal(workspaceId: string): void {
    for (const [cursor, binding] of this.#directoryCursors) {
      if (binding.workspaceId === workspaceId) {
        this.#pendingDirectoryCursors.delete(cursor);
        this.#directoryCursors.delete(cursor);
      }
    }
    const keyPrefix = `${workspaceId}\u001f`;
    for (const key of this.#directoryEntryPaths.keys()) {
      if (key.startsWith(keyPrefix)) {
        this.#directoryEntryPaths.delete(key);
      }
    }
  }

  private replaceWorkspaces(workspaces: WorkspaceProjection[]): void {
    this.#pendingRecovery = null;
    const bootstrap = this.requireBootstrap();
    const visibleWorkspaceIds = new Set(
      workspaces.map(({ workspaceId }) => workspaceId),
    );
    for (const [cursor, binding] of this.#directoryCursors) {
      if (!visibleWorkspaceIds.has(binding.workspaceId)) {
        this.#pendingDirectoryCursors.delete(cursor);
        this.#directoryCursors.delete(cursor);
      }
    }
    for (const key of this.#directoryEntryPaths.keys()) {
      const separatorIndex = key.indexOf("\u001f");
      if (
        separatorIndex < 0 ||
        !visibleWorkspaceIds.has(key.slice(0, separatorIndex))
      ) {
        this.#directoryEntryPaths.delete(key);
      }
    }
    this.#bootstrap = { ...bootstrap, workspaces };
  }

  private requireBootstrap(): BootstrapReply {
    if (!this.#bootstrap) {
      throw new HostCapabilityError(
        "The Windows host has not completed bootstrap.",
      );
    }
    return this.#bootstrap;
  }

  private requireBootstrapGeneration(
    expectedGeneration: number,
  ): BootstrapReply {
    if (this.#bootstrapGeneration !== expectedGeneration) {
      return fail();
    }
    return this.requireBootstrap();
  }

  private advanceProjectionSequence(nextSequence: number): void {
    if (
      this.#projectionSequence !== null &&
      nextSequence < this.#projectionSequence
    ) {
      fail();
    }
    this.#projectionSequence = nextSequence;
  }

  private advanceMutationSequence(
    nextSequence: number,
    issuedAfterSequence: number,
  ): void {
    if (
      nextSequence <= issuedAfterSequence ||
      this.#projectionSequence === null
    ) {
      fail();
    }
    this.#projectionSequence = Math.max(this.#projectionSequence, nextSequence);
  }

  private adoptReadOnlyRecovery(sequence: number): void {
    if (!this.#bootstrap) {
      return fail();
    }
    this.#directoryCursors.clear();
    this.#directoryEntryPaths.clear();
    this.#pendingDirectoryCursors.clear();
    this.#pendingRecovery = null;
    this.#bootstrap = {
      ...this.#bootstrap,
      bootMode: "read_only_recovery",
      supportedCommands: this.#bootstrap.supportedCommands.filter(
        (command) =>
          command === "app.get_boot_state" || command === "workspace.list",
      ),
      projectionSequence: Math.max(
        this.#bootstrap.projectionSequence,
        sequence,
      ),
    };
  }

  private buildProjectionRequest(
    scope: ProjectionScope,
    afterSequence: number | null,
  ) {
    const bootstrap = this.requireBootstrap();
    return {
      schemaVersion: PROJECTION_REQUEST_SCHEMA,
      rendererSessionId: bootstrap.rendererSessionId,
      installationId: bootstrap.installationId,
      workspaceId:
        scope.workspaceId === undefined
          ? null
          : asContractId(scope.workspaceId),
      sessionId: null,
      afterSequence,
    };
  }
}
