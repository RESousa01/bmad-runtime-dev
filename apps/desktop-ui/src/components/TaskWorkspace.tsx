import { Button } from "@sapphirus/ui";
import {
  ArrowRight,
  BookmarkCheck,
  ChevronDown,
  FileText,
  Library,
  ListChecks,
  Menu,
  MoreVertical,
  PanelRightOpen,
  Paperclip,
  Pin,
  Send,
  ShieldAlert,
  ShieldCheck,
  WifiOff,
} from "lucide-react";
import { useState, type FormEvent } from "react";
import type { ProposalState } from "../data/demo";
import { BrandMark } from "./BrandMark";
import { StageRail, type TaskStage } from "./StageRail";

export interface TaskWorkspaceProps {
  hostStatusLabel: string;
  interactionDisabled: boolean;
  isInert?: boolean;
  isNewSession: boolean;
  isReadOnlyRecovery: boolean;
  methodGuidanceAvailable: boolean;
  methodGuidanceBusy: boolean;
  methodLibraryAvailable: boolean;
  onOpenMethodLibrary: () => void;
  onOpenInspector: () => void;
  onOpenSessions: () => void;
  onReviewContext: () => void;
  onReviewChanges: () => void;
  onTaskSubmitted: (intent: string) => Promise<void>;
  proposalState: ProposalState;
  sessionTitle: string;
  workspaceName: string;
}

function getCurrentStage(proposalState: ProposalState, isNewSession: boolean): TaskStage {
  if (isNewSession) {
    return "Context";
  }
  if (proposalState === "discarded") {
    return "Plan";
  }
  return "Review";
}

export function TaskWorkspace({
  hostStatusLabel,
  interactionDisabled,
  isInert = false,
  isNewSession,
  isReadOnlyRecovery,
  methodGuidanceAvailable,
  methodGuidanceBusy,
  methodLibraryAvailable,
  onOpenMethodLibrary,
  onOpenInspector,
  onOpenSessions,
  onReviewContext,
  onReviewChanges,
  onTaskSubmitted,
  proposalState,
  sessionTitle,
  workspaceName,
}: TaskWorkspaceProps) {
  const [draft, setDraft] = useState("");
  const [submittedTask, setSubmittedTask] = useState<string | null>(null);
  const [hasMethodGuidanceSubmission, setHasMethodGuidanceSubmission] = useState(false);
  const [guidanceStatus, setGuidanceStatus] = useState<
    "idle" | "submitting" | "created"
  >("idle");
  const currentStage = getCurrentStage(proposalState, isNewSession && !submittedTask);
  const methodGuidanceView = methodGuidanceAvailable || hasMethodGuidanceSubmission;
  const guidanceSubmitting = methodGuidanceBusy || guidanceStatus === "submitting";
  const checkingRetainedGuidance = guidanceSubmitting
    && submittedTask === null
    && guidanceStatus === "idle";
  const composerDisabled = !methodGuidanceAvailable || guidanceSubmitting;

  async function submitTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (composerDisabled) {
      return;
    }
    const value = draft.trim();
    if (!value) {
      return;
    }
    setSubmittedTask(value);
    setDraft("");
    if (methodGuidanceAvailable) {
      setHasMethodGuidanceSubmission(true);
      setGuidanceStatus("submitting");
    }
    try {
      await onTaskSubmitted(value);
      if (methodGuidanceAvailable) {
        setGuidanceStatus("created");
      }
    } catch {
      if (methodGuidanceAvailable) {
        setGuidanceStatus("idle");
      }
    }
  }

  const showProposal = !methodGuidanceView && (!isNewSession || submittedTask !== null);

  return (
    <main className={`task-workspace ${isReadOnlyRecovery ? "has-recovery" : ""}`} inert={isInert}>
      <header className="task-header">
        <div className="task-header__workspace">
          <Button
            aria-label="Open sessions"
            className="mobile-panel-button"
            onPress={onOpenSessions}
            size="icon"
            variant="quiet"
          >
            <Menu aria-hidden="true" size={18} />
          </Button>
          <strong>{workspaceName}</strong>
          <span className="workspace-status">
            <span className="workspace-status__folder" aria-hidden="true" />
            {hostStatusLabel}
            <span aria-hidden="true" className={`status-dot ${isReadOnlyRecovery ? "status-dot--warning" : ""}`} />
          </span>
        </div>
        <div className="task-header__actions">
          {methodLibraryAvailable ? (
            <Button
              aria-label="Method library"
              className="method-library-trigger"
              onPress={onOpenMethodLibrary}
              size="small"
              variant="secondary"
            >
              <Library aria-hidden="true" size={16} />
              Method library
            </Button>
          ) : null}
          <Button aria-label="Pin session" isDisabled size="icon" variant="quiet">
            <Pin aria-hidden="true" size={17} />
          </Button>
          <Button aria-label="More session actions" isDisabled size="icon" variant="quiet">
            <MoreVertical aria-hidden="true" size={17} />
          </Button>
          <Button
            aria-label="Open inspector"
            className="mobile-inspector-button"
            onPress={onOpenInspector}
            size="icon"
            variant="quiet"
          >
            <PanelRightOpen aria-hidden="true" size={18} />
          </Button>
        </div>
        <div className="task-title-row">
          <h1>{isNewSession && !submittedTask ? "New session" : sessionTitle}</h1>
          <span className="preview-badge">
            {methodGuidanceView ? "Method guidance" : "Preview demo"}
          </span>
        </div>
      </header>

      {isReadOnlyRecovery ? (
        <div className="recovery-banner" role="status">
          <ShieldAlert aria-hidden="true" size={17} />
          <div>
            <strong>Read-only recovery</strong>
            <span>Sapphirus could not verify its local workspace data. Workspace changes remain blocked.</span>
          </div>
        </div>
      ) : null}

      <div className="task-scroll-region">
        <div className="preview-notice" role="note">
          <strong>{methodGuidanceView ? "Local Method guidance" : "Internal preview"}</strong>
          <span>
            {methodGuidanceView
              ? "Submitting an intent creates a local, unbound Method session. It does not contact a model or change workspace files."
              : "Agent tasks and local changes are not enabled in this build. The proposal below is demonstration content only."}
          </span>
        </div>
        {methodGuidanceView ? (
          <>
            {submittedTask ? (
              <article className="message message--user">
                <div className="message__avatar message__avatar--user">You</div>
                <div>
                  <p>{submittedTask}</p>
                  <time>Now</time>
                </div>
              </article>
            ) : null}
            {guidanceSubmitting ? (
              <article className="message message--agent message--compact" role="status">
                <div className="message__avatar">
                  <BrandMark size={22} />
                </div>
                <div>
                  <span className="message__label">
                    {checkingRetainedGuidance ? "Checking · Local only" : "Creating · Local only"}
                  </span>
                  <p>
                    {checkingRetainedGuidance
                      ? "Checking for a retained local Method session without contacting a model or changing the workspace…"
                      : "Creating the local Method session without a model request or workspace change…"}
                  </p>
                </div>
              </article>
            ) : guidanceStatus === "created" ? (
              <article className="message message--agent message--compact" role="status">
                <div className="message__avatar">
                  <BrandMark size={22} />
                </div>
                <div>
                  <span className="message__label">Created · Unbound</span>
                  <p>
                    A local Method session was created. It remains unbound: no model request was
                    made and no workspace change was proposed. Review the source-grounded
                    recommendation in the Method inspector.
                  </p>
                </div>
              </article>
            ) : submittedTask ? null : (
              <section className="empty-session" aria-labelledby="empty-session-title">
                <div className="empty-session__mark">
                  <BrandMark size={31} />
                </div>
                <h2 id="empty-session-title">What do you want Method guidance for?</h2>
                <p>
                  Describe your intent to create a local, unbound session and review a
                  source-grounded recommendation. No model or execution capability is attached.
                </p>
              </section>
            )}
          </>
        ) : showProposal ? (
          <>
            {submittedTask ? (
              <article className="message message--user">
                <div className="message__avatar message__avatar--user">You</div>
                <div>
                  <p>{submittedTask}</p>
                  <time>Now</time>
                </div>
              </article>
            ) : null}
            <article className="message message--agent">
              <div className="message__avatar">
                <BrandMark size={22} />
              </div>
              <div>
                <span className="message__label">Demo response</span>
                <p>
                  This preview shows how the Agent could present a safe workspace scan that
                  respects ignore rules and size limits, with tests and concise documentation.
                </p>
                <time>10:42 AM</time>
              </div>
            </article>

            <StageRail current={currentStage} />

            <section className="review-summary" aria-labelledby="review-heading">
              {proposalState === "discarded" ? (
                <>
                  <h2 id="review-heading">Proposal discarded</h2>
                  <p>No files were changed. Describe a new approach whenever you’re ready.</p>
                </>
              ) : (
                <>
                  <h2 id="review-heading">Ready for review</h2>
                  <p>
                    The scan is read-only, honors ignore rules, and enforces size limits. Tests
                    cover exclusions, boundaries, and invalid paths.
                  </p>
                </>
              )}

              {proposalState !== "discarded" ? (
                <>
                  <div className="impact-divider" />
                  <h3>Impact</h3>
                  <div className="impact-grid">
                    <div>
                      <FileText aria-hidden="true" size={23} />
                      <span>2 files</span>
                    </div>
                    <div>
                      <WifiOff aria-hidden="true" size={23} />
                      <span>No network</span>
                    </div>
                    <div>
                      <BookmarkCheck aria-hidden="true" size={23} />
                      <span>No files written</span>
                    </div>
                    <div>
                      <ShieldCheck aria-hidden="true" size={23} />
                      <span>Low risk</span>
                    </div>
                  </div>
                  <div className="review-summary__action">
                    <Button onPress={onReviewChanges} size="large" variant="primary">
                      Review changes
                      <ArrowRight aria-hidden="true" size={17} />
                    </Button>
                  </div>
                </>
              ) : null}
            </section>

            {proposalState === "ready" ? (
              <article className="message message--agent message--compact">
                <div className="message__avatar">
                  <BrandMark size={22} />
                </div>
                <div>
                  <p>
                    Review the demonstration changes in the inspector. Applying them is disabled in this build.
                  </p>
                  <time>10:42 AM</time>
                </div>
              </article>
            ) : null}
          </>
        ) : (
          <section className="empty-session" aria-labelledby="empty-session-title">
            <div className="empty-session__mark">
              <BrandMark size={31} />
            </div>
            <h2 id="empty-session-title">What would you like to work on?</h2>
            <p>
              Ask the Agent to inspect your local workspace, explain code, create a plan, or
              propose a focused change.
            </p>
          </section>
        )}
      </div>

      <form className="composer" onSubmit={submitTask}>
        <div className="composer__input-row">
          <Button
            aria-label="Attach context"
            isDisabled={interactionDisabled || methodGuidanceView}
            size="icon"
            variant="quiet"
          >
            <Paperclip aria-hidden="true" size={19} />
          </Button>
          <label className="sr-only" htmlFor="task-composer">
            {methodGuidanceView
              ? "Describe what you want Method guidance for"
              : "Describe a task"}
          </label>
          <textarea
            id="task-composer"
            aria-describedby="task-composer-availability"
            disabled={composerDisabled}
            onChange={(event) => setDraft(event.target.value)}
            placeholder={methodGuidanceView
              ? "Describe your intent for Method guidance…"
              : "Describe a task, ask a question, or request a review…"}
            rows={2}
            value={draft}
          />
        </div>
        <div className="composer__toolbar">
          <label>
            <span className="sr-only">Mode</span>
            <select
              defaultValue={methodGuidanceView ? "method" : "agent"}
              disabled={interactionDisabled || methodGuidanceView}
              key={methodGuidanceView ? "method" : "agent"}
            >
              <option value={methodGuidanceView ? "method" : "agent"}>
                {methodGuidanceView ? "Method guidance" : "Agent"}
              </option>
            </select>
            <ChevronDown aria-hidden="true" size={14} />
          </label>
          <div className="composer__right">
            <Button onPress={onReviewContext} size="small" variant="secondary">
              <ListChecks aria-hidden="true" size={15} />
              Review context
            </Button>
            <span className="connection-state">
              <span className="status-dot status-dot--preview" /> {hostStatusLabel}
            </span>
            <Button
              aria-label={methodGuidanceView ? "Request Method guidance" : "Send task"}
              isDisabled={composerDisabled || !draft.trim()}
              size="icon"
              type="submit"
              variant="secondary"
            >
              <Send aria-hidden="true" size={18} />
            </Button>
          </div>
        </div>
        <p className="composer__availability" id="task-composer-availability">
          {methodGuidanceView
            ? guidanceSubmitting
              ? checkingRetainedGuidance
                ? "Checking for a retained local Method session. No model request or workspace change is being made."
                : "Creating the local Method session. No model request or workspace change is being made."
              : "Creates a local, unbound Method session. No model request or workspace change is performed."
            : "Preview only — submitting tasks and applying changes require a later governed capability build."}
        </p>
      </form>
    </main>
  );
}
