import { Button } from "@sapphirus/ui";
import { ShieldCheck, Send, X } from "lucide-react";
import { useState } from "react";
import type {
  CapabilityApprovedProjection,
  CapabilityCompletedProjection,
  CapabilityReviewProjection,
} from "../lib/hostClient";

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
  readonly onPrepare: (contextPaths: readonly string[]) => void;
  readonly onSubmit: (manifestHash: string, decisionId: string) => void;
  readonly phase: CapabilityRunPhase;
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

function parseContextPaths(raw: string): string[] {
  return [
    ...new Set(
      raw
        .split("\n")
        .map((line) => line.trim())
        .filter((line) => line.length > 0),
    ),
  ].slice(0, MAX_CONTEXT_PATHS);
}

export function BmadCapabilityPanel({
  capabilityId,
  capabilityLabel,
  destinationLabel,
  onApprove,
  onCancel,
  onClose,
  onPrepare,
  onSubmit,
  phase,
}: BmadCapabilityPanelProps) {
  const [rawPaths, setRawPaths] = useState("");
  const selectedPaths = parseContextPaths(rawPaths);

  return (
    <section
      aria-label={`${capabilityLabel} capability run`}
      className="bmad-capability-panel"
      role="dialog"
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

      {phase.kind === "selecting" || phase.kind === "preparing" ? (
        <div className="bmad-capability-panel__body">
          <p>
            Select the exact workspace files to review for this run. Nothing
            is read or sent until you approve the reviewed context.
          </p>
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
          <Button
            isDisabled={selectedPaths.length === 0 || phase.kind === "preparing"}
            onPress={() => onPrepare(selectedPaths)}
            variant="secondary"
          >
            {phase.kind === "preparing" ? "Preparing review…" : "Prepare reviewed context"}
          </Button>
        </div>
      ) : phase.kind === "review" ? (
        <div className="bmad-capability-panel__body">
          <section aria-label="Exact reviewed context">
            <h3>Reviewed context</h3>
            <ul aria-label="Reviewed context files">
              {phase.contextPaths.map((path) => (
                <li key={path}>
                  <code>{path}</code>
                </li>
              ))}
            </ul>
            <p>
              Destination: <strong>{destinationLabel}</strong>
            </p>
            <p>
              Context manifest <code>{phase.review.manifestHash}</code>
            </p>
            <p role="note">{CONSENT_DISCLOSURE}</p>
          </section>
          <div className="bmad-capability-panel__actions">
            <Button
              onPress={() => onApprove(phase.review.manifestHash)}
              variant="secondary"
            >
              <ShieldCheck aria-hidden="true" size={15} /> Approve this exact context
            </Button>
            <Button onPress={onClose} variant="quiet">
              Discard review
            </Button>
          </div>
        </div>
      ) : phase.kind === "approved" ? (
        <div className="bmad-capability-panel__body">
          <p role="status">
            Consent recorded for manifest <code>{phase.approved.manifestHash}</code>.
            It can be used exactly once and expires shortly.
          </p>
          <div className="bmad-capability-panel__actions">
            <Button
              isDisabled={phase.sending}
              onPress={() =>
                onSubmit(phase.approved.manifestHash, phase.approved.decisionId)
              }
              variant="secondary"
            >
              <Send aria-hidden="true" size={15} />{" "}
              {phase.sending ? "Sending once…" : "Send once and run"}
            </Button>
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
      ) : phase.kind === "completed" ? (
        <CompletedResult
          completed={phase.completed}
          resultJson={phase.resultJson}
        />
      ) : (
        <div className="bmad-capability-panel__body">
          <p role="alert">{phase.message}</p>
          <Button onPress={onClose} variant="quiet">
            Close
          </Button>
        </div>
      )}
    </section>
  );
}

function CompletedResult({
  completed,
  resultJson,
}: {
  readonly completed: CapabilityCompletedProjection;
  readonly resultJson: string | null;
}) {
  const artifact =
    completed.resultKind === "document_artifact"
      ? documentArtifactView(resultJson)
      : null;
  return (
    <div className="bmad-capability-panel__body">
      {completed.resultKind === "inactive_builder_draft" ? (
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
          <p role="note">Open the Changes panel to review the candidate.</p>
        </section>
      ) : artifact === null ? (
        <section aria-label="Completed capability run">
          <h3>Run completed</h3>
          <p>
            The verified result (<code>{completed.resultKind}</code>) is stored
            locally under run <code>{completed.runId}</code>.
          </p>
        </section>
      ) : (
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
      )}
    </div>
  );
}
