import { Button } from "@sapphirus/ui";
import { ShieldCheck, Send, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { capabilityCompatibility } from "../features/skills/skillAvailability";
import type {
  CapabilityApprovedProjection,
  CapabilityCompletedProjection,
  CapabilityDocumentArtifact,
  CapabilityReviewProjection,
} from "../lib/hostClient";
import { CapabilityCoverageDisclosure } from "./skills/SkillCompatibility";

/// One reviewed capability run's renderer phase. The renderer never holds
/// authority: every transition is a host decision reflected back here.
export type CapabilityRunPhase =
  | { readonly kind: "selecting" }
  | { readonly kind: "preparing" }
  | {
      readonly kind: "review";
      readonly review: CapabilityReviewProjection;
      readonly contextPaths: readonly string[];
    }
  | {
      readonly kind: "approved";
      readonly review: CapabilityReviewProjection;
      readonly approved: CapabilityApprovedProjection;
      readonly contextPaths: readonly string[];
      readonly sending: boolean;
    }
  | {
      readonly kind: "completed";
      readonly completed: CapabilityCompletedProjection;
      readonly resultJson: string | null;
    }
  | { readonly kind: "error"; readonly message: string };

export interface BmadCapabilityPanelProps {
  readonly capabilityId: string;
  readonly capabilityLabel: string;
  readonly destinationLabel: string;
  readonly onApprove: (manifestHash: string) => void;
  readonly onCancel: (manifestHash: string, decisionId: string) => void;
  readonly onClose: () => void;
  readonly onPrepare: (
    contextPaths: readonly string[],
    action: "create" | "update" | "validate" | null,
    mode: "interactive" | "headless" | null,
  ) => void;
  readonly onReviewChangeSet?: () => void;
  readonly onSubmit: (manifestHash: string, decisionId: string) => void;
  readonly phase: CapabilityRunPhase;
  readonly suggestedContextPaths?: readonly string[];
}

const CONSENT_DISCLOSURE =
  "Only the exact reviewed context shown here will be sent once. Redaction reduces risk but cannot prove that every secret was detected.";
const MAX_CONTEXT_PATHS = 100;

interface DocumentArtifactView {
  readonly title: string;
  readonly sections: readonly { readonly heading: string; readonly body: string }[];
  readonly openQuestions: readonly string[];
}

/// Bounded, display-only decoding of a stored document-artifact result.
/// Anything outside the expected shape renders as "not displayable".
function documentArtifactView(resultJson: string | null): DocumentArtifactView | null {
  if (resultJson === null) return null;
  try {
    const parsed: unknown = JSON.parse(resultJson);
    if (typeof parsed !== "object" || parsed === null) return null;
    const record = parsed as Record<string, unknown>;
    if (record.resultKind !== "document_artifact") return null;
    const artifact = record.documentArtifact as Record<string, unknown> | undefined;
    if (artifact === undefined || typeof artifact.title !== "string") return null;
    const sections = Array.isArray(artifact.sections) ? artifact.sections : [];
    const openQuestions = Array.isArray(artifact.openQuestions)
      ? artifact.openQuestions
      : [];
    return {
      title: artifact.title,
      sections: sections.flatMap((section) => {
        const item = section as Record<string, unknown>;
        return typeof item.heading === "string" && typeof item.body === "string"
          ? [{ heading: item.heading, body: item.body }]
          : [];
      }),
      openQuestions: openQuestions.filter(
        (question): question is string => typeof question === "string",
      ),
    };
  } catch {
    return null;
  }
}

interface ContextSelection {
  readonly paths: readonly string[];
  /// How many distinct paths were typed past `MAX_CONTEXT_PATHS`. The cap used
  /// to be applied by a bare `.slice()` with nothing on screen to say so, which
  /// is the one thing this panel must never do: silently disagree with the user
  /// about what "the exact reviewed context" is.
  readonly droppedOverCap: number;
}

function parseContextPaths(raw: string): ContextSelection {
  const unique = uniqueContextPaths(
    raw
      .split("\n"),
  );
  return {
    paths: unique.slice(0, MAX_CONTEXT_PATHS),
    droppedOverCap: Math.max(0, unique.length - MAX_CONTEXT_PATHS),
  };
}

function uniqueContextPaths(paths: readonly string[]): string[] {
  return [
    ...new Set(
      paths
        .map((path) => path.trim())
        .filter((path) => path.length > 0),
    ),
  ];
}

/// The longest deadline this panel will arm a timer for.
///
/// `setTimeout` truncates its delay to a signed 32-bit integer, so a deadline
/// further out than ~24 days fires *immediately* rather than never — which
/// would report a perfectly live consent as expired. Anything past this bound
/// cannot lapse inside a session, so no timer is the correct answer.
const MAX_DEADLINE_TIMER_MS = 24 * 60 * 60 * 1000;

/// Re-renders once when a host-issued deadline passes.
///
/// Both the reviewed manifest and the approved consent carry a short
/// `expiresAt`, and both were rendered as if they never lapsed: the panel said
/// "expires shortly" and left Approve and Send enabled indefinitely, so the
/// first thing a user saw after leaving the panel open was a rejected send with
/// no way back. Reading the deadline the host already sends is enough to say so
/// before the request goes out.
function useDeadlinePassed(expiresAt: number | null): boolean {
  // The *deadline* that lapsed, not a bare flag. Keying it this way means a
  // later phase carrying a fresh deadline is live again without the effect
  // having to write state on the way in — which is both a cascading render and
  // the thing that would silently mark a new consent expired.
  const [lapsedDeadline, setLapsedDeadline] = useState<number | null>(null);

  useEffect(() => {
    if (expiresAt === null) return;
    const remaining = expiresAt - Date.now();
    if (remaining > MAX_DEADLINE_TIMER_MS) return;
    const timer = setTimeout(() => setLapsedDeadline(expiresAt), Math.max(0, remaining));
    return () => clearTimeout(timer);
  }, [expiresAt]);

  return expiresAt !== null && lapsedDeadline === expiresAt;
}

function expiryClock(expiresAt: number): string {
  return new Date(expiresAt).toLocaleTimeString();
}

/// Moves focus to the run panel when it opens and hands focus back to whatever
/// opened it when it closes.
///
/// Without this the panel was effectively invisible: it renders at the top of
/// the scrolling skills drawer while the "Start reviewed run" controls sit far
/// below it in the agent list, so starting a run scrolled nothing, moved no
/// focus, and announced nothing. The button stayed focused and on screen and
/// the run appeared off-screen above — indistinguishable from a dead button.
///
/// Deliberately *not* a focus trap. This panel is not modal — the drawer around
/// it owns that layer — and a second hand-rolled trap is exactly the bug this
/// repo has shipped before.
function useRunPanelFocus() {
  const panelRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    const panel = panelRef.current;
    const opener = document.activeElement instanceof HTMLElement
      && document.activeElement !== document.body
      ? document.activeElement
      : null;
    panel?.focus();
    // jsdom does not implement scrolling, and neither does any non-visual
    // host; the guard keeps this a progressive enhancement rather than a throw.
    if (typeof panel?.scrollIntoView === "function") {
      panel.scrollIntoView({ block: "nearest" });
    }
    return () => {
      // Only reclaim focus if it is still inside the panel being removed.
      // Yanking it back from wherever the user has since moved would be worse
      // than leaving it on the body.
      const active = document.activeElement;
      const focusStillInside = active === null
        || active === document.body
        || panel?.contains(active) === true;
      if (focusStillInside && opener?.isConnected === true) {
        opener.focus();
      }
    };
  }, []);

  return panelRef;
}

export function BmadCapabilityPanel({
  capabilityId,
  capabilityLabel,
  destinationLabel,
  onApprove,
  onCancel,
  onClose,
  onPrepare,
  onReviewChangeSet,
  onSubmit,
  phase,
  suggestedContextPaths = [],
}: BmadCapabilityPanelProps) {
  const isCanonicalPrd = capabilityId === "bmm:bmad-prd";
  const [prdAction, setPrdAction] = useState<"create" | "update" | "validate">("create");
  const [prdMode, setPrdMode] = useState<"interactive" | "headless">("interactive");
  const [rawPaths, setRawPaths] = useState("");
  const { paths: selectedPaths, droppedOverCap } = parseContextPaths(rawPaths);
  const attachedPaths = uniqueContextPaths(suggestedContextPaths);
  const panelRef = useRunPanelFocus();
  const reviewExpired = useDeadlinePassed(
    phase.kind === "review" ? phase.review.expiresAt : null,
  );
  const consentExpired = useDeadlinePassed(
    phase.kind === "approved" ? phase.approved.expiresAt : null,
  );

  return (
    <section
      aria-label={`${capabilityLabel} capability run`}
      className="bmad-capability-panel"
      ref={panelRef}
      role="dialog"
      tabIndex={-1}
    >
      <header className="bmad-capability-panel__header">
        <div>
          <h2>{capabilityLabel}</h2>
          <code>{capabilityId}</code>
        </div>
        <Button aria-label="Close capability run" onPress={onClose} size="icon" variant="quiet">
          <X aria-hidden="true" size={17} />
        </Button>
      </header>

      <StepProgress phase={phase} />

      {/*
        * What the recorded coverage matrix says about the capability this run
        * spends its consent on -- including, on every capability, that nobody
        * has reviewed how it compares with upstream.
        *
        * Keyed by capability, not by `skillVerdict`. A run reaches this panel
        * because the catalogue's verdict already permitted it, and asking a
        * second time from a surface that holds only an id would answer
        * "unknown" for every Builder capability and every Forge-native one --
        * manufacturing a doubt about a run the host has already accepted.
        * Deciding availability twice, in two places, is the drift this model
        * was built to end.
        */}
      <CapabilityCoverageDisclosure compatibility={capabilityCompatibility(capabilityId)} />

      {phase.kind === "selecting" || phase.kind === "preparing" || phase.kind === "error" ? (
        // The failure path shares the selection body on purpose. Recovering
        // from any failure in this lane means choosing context and preparing
        // again, and the previous shape — a bare message with a Close button —
        // threw away everything the user had typed and made a failed run
        // indistinguishable from an abandoned one.
        <div aria-busy={phase.kind === "preparing" || undefined} className="bmad-capability-panel__body">
          {phase.kind === "error" ? (
            <section aria-label="Run failed">
              <p role="alert">{phase.message}</p>
              <p>
                Nothing was applied and no file in your workspace changed. Any
                consent this run had spent is gone, so continuing means
                preparing and approving the context again.
              </p>
            </section>
          ) : null}
          <p>
            A reviewed run never sends your workspace. You name the exact files
            below, read them back, and approve them — only then is that exact
            context sent once to <strong>{destinationLabel}</strong>. The reply
            is checked against a fixed shape, and any file change it proposes
            goes to Changes for you to approve before anything is written.
          </p>
          {isCanonicalPrd ? (
            <fieldset>
              <legend>PRD execution branch</legend>
              <label>
                Action
                <select
                  aria-label="PRD action"
                  disabled={phase.kind === "preparing"}
                  onChange={(event) =>
                    setPrdAction(event.target.value as "create" | "update" | "validate")
                  }
                  value={prdAction}
                >
                  <option value="create">Create</option>
                  <option value="update">Update</option>
                  <option value="validate">Validate</option>
                </select>
              </label>
              <label>
                Mode
                <select
                  aria-label="PRD mode"
                  disabled={phase.kind === "preparing"}
                  onChange={(event) =>
                    setPrdMode(event.target.value as "interactive" | "headless")
                  }
                  value={prdMode}
                >
                  <option value="interactive">Interactive</option>
                  <option value="headless">Headless</option>
                </select>
              </label>
            </fieldset>
          ) : null}
          <label>
            Context file paths (one relative path per line)
            <textarea
              aria-label="Context file paths"
              disabled={phase.kind === "preparing"}
              onChange={(event) => setRawPaths(event.target.value)}
              rows={6}
              value={rawPaths}
            />
          </label>
          {isCanonicalPrd && attachedPaths.length > 0 ? (
            <Button
              isDisabled={phase.kind === "preparing"}
              onPress={() => {
                const manualPaths = uniqueContextPaths(rawPaths.split("\n"));
                setRawPaths(uniqueContextPaths([...manualPaths, ...attachedPaths]).join("\n"));
              }}
              variant="quiet"
            >
              Add attached files ({attachedPaths.length})
            </Button>
          ) : null}
          <p role="status">
            {selectedPaths.length === 0
              ? "No files selected yet. Name at least one to prepare a review."
              : selectedPaths.length === 1
                ? "1 file selected."
                : `${selectedPaths.length} files selected.`}
          </p>
          {droppedOverCap > 0 ? (
            <p role="alert">
              Only the first {MAX_CONTEXT_PATHS} paths can go into one reviewed
              context. The remaining {droppedOverCap}{" "}
              {droppedOverCap === 1 ? "path is" : "paths are"} not included and
              will not be sent. Remove them, or run this again for the rest.
            </p>
          ) : null}
          <Button
            isDisabled={selectedPaths.length === 0 || phase.kind === "preparing"}
            onPress={() =>
              onPrepare(
                selectedPaths,
                isCanonicalPrd ? prdAction : null,
                isCanonicalPrd ? prdMode : null,
              )
            }
            variant="secondary"
          >
            {phase.kind === "preparing"
              ? "Preparing review…"
              : phase.kind === "error"
                ? "Prepare reviewed context again"
                : "Prepare reviewed context"}
          </Button>
          {phase.kind === "error" ? (
            <Button onPress={onClose} variant="quiet">
              Close
            </Button>
          ) : null}
        </div>
      ) : phase.kind === "review" ? (
        <div className="bmad-capability-panel__body">
          <section aria-label="Exact reviewed context">
            <h3>Reviewed context</h3>
            {phase.contextPaths.length === 0 ? (
              // An empty list next to "the exact reviewed context" reads as a
              // rendering failure. Say which it is.
              <p role="alert">
                This review covers no files. Nothing would be sent, so prepare
                the context again with at least one file.
              </p>
            ) : (
              <ul aria-label="Reviewed context files">
                {phase.contextPaths.map((path) => (
                  <li key={path}>
                    <code>{path}</code>
                  </li>
                ))}
              </ul>
            )}
            <p>
              Destination: <strong>{destinationLabel}</strong>
            </p>
            <p>
              Context manifest <code>{phase.review.manifestHash}</code>
            </p>
            <p>Approvable until {expiryClock(phase.review.expiresAt)}.</p>
            <p role="note">{CONSENT_DISCLOSURE}</p>
          </section>
          {reviewExpired ? (
            <p role="alert">
              This review expired before it was approved, so approving it now
              would be refused. Prepare the same context again to get a fresh
              review.
            </p>
          ) : null}
          <div className="bmad-capability-panel__actions">
            <Button
              isDisabled={reviewExpired || phase.contextPaths.length === 0}
              onPress={() => onApprove(phase.review.manifestHash)}
              variant="secondary"
            >
              <ShieldCheck aria-hidden="true" size={15} /> Approve this exact context
            </Button>
            {reviewExpired ? (
              <Button
                onPress={() =>
                  onPrepare(
                    phase.contextPaths,
                    isCanonicalPrd ? prdAction : null,
                    isCanonicalPrd ? prdMode : null,
                  )
                }
                variant="secondary"
              >
                Prepare this context again
              </Button>
            ) : null}
            <Button onPress={onClose} variant="quiet">
              Discard review
            </Button>
          </div>
        </div>
      ) : phase.kind === "approved" ? (
        <div className="bmad-capability-panel__body">
          <p role="status">
            Consent recorded for manifest <code>{phase.approved.manifestHash}</code>.
            It can be used exactly once, and lapses at{" "}
            {expiryClock(phase.approved.expiresAt)}.
          </p>
          {consentExpired ? (
            <p role="alert">
              That consent has lapsed, so sending now would be refused. Prepare
              the same context again and approve it once more — nothing has been
              sent and nothing in your workspace changed.
            </p>
          ) : null}
          <div className="bmad-capability-panel__actions">
            <Button
              isDisabled={phase.sending || consentExpired}
              onPress={() =>
                onSubmit(phase.approved.manifestHash, phase.approved.decisionId)
              }
              variant="secondary"
            >
              <Send aria-hidden="true" size={15} />{" "}
              {phase.sending ? "Sending once…" : "Send once and run"}
            </Button>
            {consentExpired ? (
              <Button
                onPress={() =>
                  onPrepare(
                    phase.contextPaths,
                    isCanonicalPrd ? prdAction : null,
                    isCanonicalPrd ? prdMode : null,
                  )
                }
                variant="secondary"
              >
                Prepare this context again
              </Button>
            ) : null}
            <Button
              isDisabled={phase.sending}
              onPress={() =>
                onCancel(phase.approved.manifestHash, phase.approved.decisionId)
              }
              variant="quiet"
            >
              Cancel consent
            </Button>
          </div>
        </div>
      ) : (
        <CompletedResult
          completed={phase.completed}
          onReviewChangeSet={onReviewChangeSet}
          resultJson={phase.resultJson}
        />
      )}
    </section>
  );
}

/// The one stepwise capability announces which host-selected step this run
/// answers. The host owns progression: this is a readout, never a control.
function StepProgress({ phase }: { readonly phase: CapabilityRunPhase }) {
  const progress =
    phase.kind === "review"
      ? phase.review.architecture
      : phase.kind === "approved"
        ? phase.review.architecture
        : phase.kind === "completed"
          ? phase.completed.architecture
          : null;
  if (progress === null) return null;
  return (
    <p className="bmad-capability-panel__progress" role="status">
      {progress.finalized
        ? `All ${progress.stepCount} steps answered — spine distilled.`
        : `Step ${progress.stepNumber} of ${progress.stepCount} — ${progress.stepTitle}`}
    </p>
  );
}

/// The distilled spine arrives inline on the final step rather than as a
/// stored run result, because it is a local distillation of the steps already
/// stored — not a fresh model claim.
function ArchitectureSpine({ spine }: { readonly spine: CapabilityDocumentArtifact }) {
  return (
    <article aria-label={spine.title}>
      <h3>{spine.title}</h3>
      <p role="note">
        Distilled locally from the answered steps. No model was contacted for
        this artifact, and nothing in your workspace changed.
      </p>
      {spine.sections.map((section) => (
        <section key={section.heading}>
          <h4>{section.heading}</h4>
          <p>{section.body}</p>
        </section>
      ))}
      {spine.openQuestions.length > 0 ? (
        <section aria-label="Open questions">
          <h4>Open questions</h4>
          <ul>
            {spine.openQuestions.map((question) => (
              <li key={question}>{question}</li>
            ))}
          </ul>
        </section>
      ) : null}
    </article>
  );
}

function CompletedResult({
  completed,
  onReviewChangeSet,
  resultJson,
}: {
  readonly completed: CapabilityCompletedProjection;
  readonly onReviewChangeSet?: (() => void) | undefined;
  readonly resultJson: string | null;
}) {
  const artifact =
    completed.resultKind === "document_artifact"
      ? documentArtifactView(resultJson)
      : null;
  const spine = completed.architecture?.spine ?? null;
  return (
    <div className="bmad-capability-panel__body">
      {spine !== null ? (
        <ArchitectureSpine spine={spine} />
      ) : completed.architecture !== null ? (
        <section aria-label="Architecture step answered">
          <h3>Step answered</h3>
          <p>
            Step {completed.architecture.stepNumber} of{" "}
            {completed.architecture.stepCount} —{" "}
            {completed.architecture.stepTitle} is recorded. Run the next step to
            continue; the host will not let a step be skipped or repeated.
          </p>
        </section>
      ) : completed.resultKind === "inactive_builder_draft" ? (
        <section aria-label="Inactive builder draft">
          <h3>Inactive draft produced</h3>
          <p>
            This run produced a versioned Builder draft. It is stored as
            inactive data: it cannot install, register, execute, or appear
            in the capability catalog.
          </p>
          <p role="note">
            Draft <code>{completed.runId}</code> stays local and inert.
          </p>
        </section>
      ) : completed.resultKind === "governed_change_set" ? (
        <section aria-label="Candidate change set">
          <h3>Candidate change set produced</h3>
          <p>
            This run produced a candidate change set. It has no authority and
            changed nothing: review and approve it in Governed changes before
            any file is touched.
          </p>
          {onReviewChangeSet === undefined ? (
            <p role="note">Open the Changes panel to review the candidate.</p>
          ) : (
            <Button onPress={onReviewChangeSet} variant="primary">
              Review changes
            </Button>
          )}
        </section>
      ) : artifact !== null ? (
        <article aria-label={artifact.title}>
          <h3>{artifact.title}</h3>
          {artifact.sections.map((section) => (
            <section key={section.heading}>
              <h4>{section.heading}</h4>
              <p>{section.body}</p>
            </section>
          ))}
          {artifact.openQuestions.length > 0 ? (
            <section aria-label="Open questions">
              <h4>Open questions</h4>
              <ul>
                {artifact.openQuestions.map((question) => (
                  <li key={question}>{question}</li>
                ))}
              </ul>
            </section>
          ) : null}
        </article>
      ) : completed.resultKind === "document_artifact" ? (
        // A document result that could not be decoded is a failure, not a
        // success with a terse subtitle. The previous copy reported it in the
        // same words as an unknown-but-fine result kind, so a run whose output
        // never made it back on to this screen still read as "Run completed".
        <section aria-label="Result could not be displayed">
          <h3>Result stored, but not shown</h3>
          <p role="alert">
            This run finished and its result was stored, but the stored copy did
            not come back in the shape this panel can read, so there is nothing
            to show you. Nothing was applied and no file in your workspace
            changed.
          </p>
          <p>
            Run <code>{completed.runId}</code> stays local. Preparing and
            approving the context again is the way to get a document you can
            read.
          </p>
        </section>
      ) : (
        <section aria-label="Completed capability run">
          <h3>Run completed</h3>
          <p>
            This run produced a <code>{completed.resultKind}</code> result, which
            this panel has no view for. It is stored locally under run{" "}
            <code>{completed.runId}</code>, and nothing was applied.
          </p>
        </section>
      )}
    </div>
  );
}
