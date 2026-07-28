/**
 * Reviewing and deciding a journal recovery.
 *
 * FE-10 step 3. A quarantined journal is work the host could not finish
 * cleanly, and recovering it replays writes into the user's repository. So the
 * decision is consent, and consent is bound to what was actually displayed:
 * `decide` sends back `displayedRecoveryHash`, the hash of the review the user
 * read. If the host's idea of the recovery has moved since, that binding
 * mismatches and the host refuses — which is the point. Approving "the
 * recovery" in the abstract would let the thing approved differ from the thing
 * shown.
 *
 * Two locks, not one
 * ------------------
 * `preparing` and `deciding` are separate because they guard different things.
 * Preparing is entered from the Activity journal list, where a second click on
 * a different journal must not start a second preparation. Deciding is entered
 * from the review dialog, which only exists once a preparation has succeeded --
 * so a single lock would either block a legitimate decision or allow a second
 * preparation underneath an open review.
 */

import { useCallback, useRef, useState } from "react";

import type {
  ChangesRecoveryPrepared,
  RecoveryApprovalChoice,
} from "../../lib/hostClient";

/** A prepared recovery the host says a human must look at. */
export type ManualReview = Extract<
  ChangesRecoveryPrepared,
  { readonly status: "manual_review" }
>;

/** A prepared recovery awaiting the user's decision. */
export type RecoveryReview = Extract<
  ChangesRecoveryPrepared,
  { readonly status: "review_required" }
>;

export interface RecoveryReviewState {
  readonly review: RecoveryReview | null;
  readonly busy: boolean;
  /** Where focus goes when the review closes: the row that opened it. */
  readonly returnFocusTarget: HTMLElement | null;
  readonly setReturnFocusTarget: (target: HTMLElement | null) => void;
  /** True while a decision is in flight, which suppresses history refreshes. */
  readonly decisionInFlight: () => boolean;
  /** Clears everything. Called when the workspace binding changes. */
  readonly clear: () => void;
  /** @internal For the actions built from this state. */
  readonly internals: RecoveryInternals;
}

interface RecoveryInternals {
  readonly preparingRef: { current: boolean };
  readonly decidingRef: { current: boolean };
  readonly generationRef: { current: number };
  readonly setReview: (review: RecoveryReview | null) => void;
  readonly setBusy: (busy: boolean) => void;
}

export function useRecoveryReview(): RecoveryReviewState {
  const [review, setReview] = useState<RecoveryReview | null>(null);
  const [busy, setBusy] = useState(false);
  const [returnFocusTarget, setReturnFocusTarget] = useState<HTMLElement | null>(null);
  const preparingRef = useRef(false);
  const decidingRef = useRef(false);
  const generationRef = useRef(0);

  const clear = useCallback(() => {
    // The generation bump is what makes this safe: an in-flight preparation or
    // decision for the old binding lands stale and writes nothing.
    generationRef.current += 1;
    preparingRef.current = false;
    decidingRef.current = false;
    setReview(null);
    setBusy(false);
    setReturnFocusTarget(null);
  }, []);

  return {
    busy,
    clear,
    decisionInFlight: () => decidingRef.current,
    internals: { decidingRef, generationRef, preparingRef, setBusy, setReview },
    returnFocusTarget,
    review,
    setReturnFocusTarget,
  };
}

/** The whole authority this feature holds. */
export interface RecoveryHostPort {
  readonly prepare: (input: {
    workspaceId: string;
    workspaceGrantEpoch: number;
    journalId: string;
  }) => Promise<ChangesRecoveryPrepared>;
  readonly decide: (input: {
    recoveryApprovalId: string;
    displayedRecoveryHash: string;
    choice: RecoveryApprovalChoice;
  }) => Promise<unknown>;
}

export interface RecoveryActionPorts {
  /** Null unless a ready host, a granted workspace and governed edits are all present. */
  readonly host: RecoveryHostPort | null;
  readonly workspace: { workspaceId: string; grantEpoch: number } | null;
  /** False once the workspace binding this action started under has been replaced. */
  readonly authorityIsCurrent: () => boolean;
  /** Whether the journal is one the host said has a review available. */
  readonly journalHasReview: (journalId: string) => boolean;
  readonly refreshHistory: () => Promise<void>;
  readonly reportFailure: (error: unknown) => void;
  readonly setError: (message: string | null) => void;
  /**
   * The sentence explaining why a journal needs manual review.
   *
   * Typed to the host's closed set of reason codes rather than `string`: a
   * port that accepts any string accepts a code no message exists for, and the
   * user gets `undefined` where an explanation should be.
   */
  readonly manualReviewMessage: (reasonCode: ManualReview["reasonCode"]) => string;
}

export interface RecoveryActions {
  readonly prepare: (journalId: string, trigger: HTMLElement) => Promise<void>;
  readonly decide: (choice: RecoveryApprovalChoice) => Promise<void>;
}

export function createRecoveryActions(
  state: RecoveryReviewState,
  ports: RecoveryActionPorts,
): RecoveryActions {
  const { decidingRef, generationRef, preparingRef, setBusy, setReview } = state.internals;

  /** Puts the review back to a state the user can act from. */
  function reset(): void {
    setReview(null);
    setBusy(false);
    state.setReturnFocusTarget(null);
    preparingRef.current = false;
    decidingRef.current = false;
  }

  return {
    async decide(choice: RecoveryApprovalChoice) {
      const review = state.review;
      if (
        ports.host === null
        || review === null
        || state.busy
        || decidingRef.current
      ) {
        return;
      }
      const generation = generationRef.current;
      decidingRef.current = true;
      setBusy(true);
      ports.setError(null);
      try {
        await ports.host.decide({
          // The hash of what was displayed, not an id the host can reinterpret.
          // If the host's recovery has moved since the review was rendered,
          // this mismatches and the host refuses -- which is the guarantee
          // that the thing approved is the thing shown.
          displayedRecoveryHash: review.displayedRecoveryHash,
          recoveryApprovalId: review.recoveryApprovalId,
          choice,
        });
        if (generation !== generationRef.current) return;
        setReview(null);
        setBusy(false);
        decidingRef.current = false;
        await ports.refreshHistory();
      } catch (error) {
        if (generation !== generationRef.current) return;
        reset();
        ports.reportFailure(error);
      }
    },

    async prepare(journalId: string, trigger: HTMLElement) {
      if (
        ports.host === null
        || ports.workspace === null
        || state.busy
        // A review already open: preparing another underneath it would replace
        // what the user is reading with something they did not ask for.
        || state.review !== null
        || preparingRef.current
        // Only a journal the host said has a review. Asking for one it did not
        // offer is a request it will refuse, reported as an error the user
        // cannot act on.
        || !ports.journalHasReview(journalId)
      ) {
        return;
      }
      const workspace = ports.workspace;
      const generation = ++generationRef.current;
      preparingRef.current = true;
      decidingRef.current = false;
      setReview(null);
      state.setReturnFocusTarget(trigger);
      setBusy(true);
      ports.setError(null);
      try {
        const prepared = await ports.host.prepare({
          journalId,
          workspaceId: workspace.workspaceId,
          workspaceGrantEpoch: workspace.grantEpoch,
        });
        if (generation !== generationRef.current || !ports.authorityIsCurrent()) return;
        setBusy(false);
        preparingRef.current = false;

        if (prepared.status === "review_required") {
          setReview(prepared);
          return;
        }
        setReview(null);
        state.setReturnFocusTarget(null);
        if (prepared.status === "manual_review") {
          const message = ports.manualReviewMessage(prepared.reasonCode);
          // The refresh first, then the message: refreshing clears the error,
          // so setting it earlier would erase it before the user saw it.
          await ports.refreshHistory();
          if (ports.authorityIsCurrent()) ports.setError(message);
          return;
        }
        await ports.refreshHistory();
      } catch (error) {
        if (generation !== generationRef.current) return;
        reset();
        ports.reportFailure(error);
      }
    },
  };
}
