/**
 * The method-guidance request: create, prepare, approve, submit.
 *
 * FE-10 step 3, the last large cluster. Two functions of ~90 lines each, sharing
 * three locks and one generation, all held as refs closed over from App's body.
 * Reading them told you what they did; it did not tell you what they depended
 * on, and the dependency set is the interesting part -- this flow spends the
 * user's money and writes to their repository, and every await in it is a place
 * the authority behind the request can change.
 *
 * The three guards, named
 * -----------------------
 * `creation` — one request at a time, and a second caller gets *the same
 * promise* rather than a second request. Returning a fresh rejected promise
 * would be simpler and wrong: the composer would report a failure while the
 * first request was still succeeding.
 *
 * `operation` — no approve or submit while one is already running. Distinct
 * from `creation` because approval is reachable on its own, from the review UI,
 * without going through `send`.
 *
 * `generation` — bumped by anything that invalidates the request's authority: a
 * new task, a workspace switch, a model change, a host rebind. Compared after
 * every await, because a result that arrives under a different authority is not
 * a result for this request. Approving one manifest and submitting under
 * another is the failure this prevents, and it is one the host would accept.
 */

import { getSafeHostMessage, HostCapabilityError } from "../../lib/hostClient";
import { isRecoveryFailure } from "../hostFailure";
import {
  bmadRequestAuthorityIsCurrent,
  type BmadAuthoritySnapshot,
  type BmadRequestEvent,
  type BmadRequestState,
} from "../../lib/bmadModelProjection";
import type { BmadHelpClient } from "../../lib/hostClient/clients";
import type { ModelAuthStatusProjection } from "../../lib/hostClient";

/** The workspace a request is bound to. The epoch is what the host checks. */
export interface GuidanceWorkspace {
  readonly workspaceId: string;
  readonly grantEpoch: number;
}

/** A mutable slot, so the caller keeps owning its refs. */
export interface Slot<T> {
  get: () => T;
  set: (value: T) => void;
}

export interface GuidanceRequestPorts {
  /** Null unless the whole guidance closure is available. */
  readonly client: BmadHelpClient | null;
  readonly workspace: GuidanceWorkspace | null;
  readonly modelAuth: ModelAuthStatusProjection | null;
  /** For the recovery transition, which needs the sequence it was seen at. */
  readonly enterReadOnlyRecovery: () => void;
  readonly nextGeneration: () => number;
  readonly currentGeneration: () => number;
  readonly applyEvent: (event: BmadRequestEvent) => BmadRequestState;
  readonly currentState: () => BmadRequestState;
  readonly currentAuthority: (runId: string) => BmadAuthoritySnapshot | null;
  /// `cancel` is the operation the review UI sets; the runner never sets it,
  /// but it must be able to see it in order to refuse while one is running.
  readonly operation: Slot<"approve" | "cancel" | "submit" | null>;
  readonly creation: Slot<Promise<void> | null>;
  /** Names the task from its opening message and records the intent. */
  readonly recordIntent: (intent: string) => void;
  readonly contextPaths: readonly string[];
  readonly now: () => number;
}

export interface GuidanceRequest {
  readonly send: (intent: string) => Promise<void>;
  readonly approveAndSubmit: (
    prepared: Extract<BmadRequestState, { readonly kind: "review_required" }>,
  ) => Promise<void>;
}

/** The states in which an approval or a submission is still outstanding. */
export function isInFlight(state: BmadRequestState): boolean {
  return state.kind === "creating"
    || state.kind === "approving"
    || state.kind === "approved"
    || state.kind === "submitting";
}

export function createGuidanceRequest(ports: GuidanceRequestPorts): GuidanceRequest {
  const {
    applyEvent,
    contextPaths,
    creation,
    currentAuthority,
    currentGeneration,
    currentState,
    enterReadOnlyRecovery,
    modelAuth,
    nextGeneration,
    now,
    operation,
    recordIntent,
    workspace,
  } = ports;

  async function approveAndSubmit(
    prepared: Extract<BmadRequestState, { readonly kind: "review_required" }>,
  ): Promise<void> {
    const client = ports.client;
    if (client === null || workspace === null || operation.get() !== null) {
      throw new HostCapabilityError("The message can no longer be sent.");
    }
    const authority = currentAuthority(prepared.run.runId);
    if (!authority || !bmadRequestAuthorityIsCurrent(prepared, authority)) {
      applyEvent({ type: "authority_invalidated", reason: "authority_changed" });
      throw new HostCapabilityError("The message authority changed. Send it again.");
    }

    const generation = authority.rendererGeneration;
    operation.set("approve");
    applyEvent({ type: "approve_started" });
    try {
      const approval = await client.approveBmadHelp(
        workspace.workspaceId,
        workspace.grantEpoch,
        prepared.authority.manifestHash,
      );
      /*
       * Re-checked, not assumed. Between asking for approval and receiving it
       * the user may have switched workspace or changed model, and submitting
       * an approval granted under the previous authority is a write the host
       * would accept and the user never sanctioned.
       */
      const approving = currentState();
      const approvingAuthority = currentAuthority(prepared.run.runId);
      if (
        generation !== currentGeneration()
        || approving.kind !== "approving"
        || !approvingAuthority
        || !bmadRequestAuthorityIsCurrent(approving, approvingAuthority)
      ) {
        throw new HostCapabilityError("The message authority changed before sending.");
      }
      const approved = applyEvent({ type: "approved", approval });
      if (approved.kind !== "approved") {
        throw new HostCapabilityError("The message approval did not match this request.");
      }

      operation.set("submit");
      applyEvent({ type: "submit_started" });
      const result = await client.submitBmadHelp(
        workspace.workspaceId,
        workspace.grantEpoch,
        approved.authority.manifestHash,
        approved.approval.decisionId,
      );
      if (generation !== currentGeneration() || currentState().kind !== "submitting") {
        throw new HostCapabilityError("The completed response no longer belongs to this task.");
      }
      applyEvent({ type: "completed", result });
    } catch (error: unknown) {
      // Only the request that is still current may report a failure. A
      // superseded one would overwrite the state of whatever replaced it.
      if (generation === currentGeneration() && isInFlight(currentState())) {
        applyEvent({ type: "unavailable", message: getSafeHostMessage(error) });
      }
      throw error;
    } finally {
      operation.set(null);
    }
  }

  function send(intent: string): Promise<void> {
    /*
     * The same promise, not a new rejected one. A second caller -- a
     * double-click, or a keyboard shortcut racing the button -- is asking about
     * the request already running, and handing them a rejection would report a
     * failure while the first request was still succeeding.
     */
    const existing = creation.get();
    if (existing !== null) return existing;

    const client = ports.client;
    if (
      client === null
      || workspace === null
      || modelAuth === null
      || modelAuth.status !== "development_ready"
      || operation.get() !== null
    ) {
      return Promise.reject(
        new HostCapabilityError("Skill guidance is unavailable for the active workspace grant."),
      );
    }

    const generation = nextGeneration();
    // Asking for guidance is still the task's opening message, so it names the
    // task the same way an ordinary chat turn does.
    recordIntent(intent);
    applyEvent({ type: "create_started" });

    const running = (async () => {
      try {
        const run = await client.createBmadHelpRun(
          workspace.workspaceId,
          workspace.grantEpoch,
          intent,
        );
        if (generation !== currentGeneration()) {
          throw new HostCapabilityError(
            "The skill-guidance result no longer belongs to the active workspace grant.",
          );
        }
        const review = await client.prepareBmadHelp(
          workspace.workspaceId,
          workspace.grantEpoch,
          [...contextPaths],
        );
        if (generation !== currentGeneration() || modelAuth.status !== "development_ready") {
          throw new HostCapabilityError(
            "The model connection changed before the message could be sent.",
          );
        }
        const next = applyEvent({
          type: "review_ready",
          run,
          review,
          authority: {
            workspaceId: workspace.workspaceId,
            workspaceGrantEpoch: workspace.grantEpoch,
            runId: run.runId,
            authEpoch: modelAuth.authEpoch,
            rendererGeneration: generation,
            now: now(),
          },
        });
        if (next.kind !== "review_required") {
          throw new HostCapabilityError("The prepared skill-guidance request failed closed.");
        }
        await approveAndSubmit(next);
      } catch (error: unknown) {
        if (generation === currentGeneration()) {
          // One definition of "recovery failure", shared with the workspace and
          // changes runners. This site had its own inline copy.
          if (isRecoveryFailure(error)) enterReadOnlyRecovery();
          else applyEvent({ type: "unavailable", message: getSafeHostMessage(error) });
        }
        throw error;
      }
    })().finally(() => {
      // Only if it is still ours. A later request that replaced this one owns
      // the slot, and clearing it unconditionally would unlock a request that
      // is still running.
      if (creation.get() === running) creation.set(null);
    });
    creation.set(running);
    return running;
  }

  return { approveAndSubmit, send };
}
