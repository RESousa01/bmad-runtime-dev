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
import type { BmadRequestState } from "../lib/bmadModelProjection";
import { BrandMark } from "./BrandMark";
import { StageRail, type TaskStage } from "./StageRail";

export interface TaskWorkspaceProps {
  hostStatusLabel: string;
  interactionDisabled: boolean;
  isInert?: boolean;
  isNewSession: boolean;
  isReadOnlyRecovery: boolean;
  methodGuidanceAvailable: boolean;
  methodGuidanceState: BmadRequestState;
  methodLibraryAvailable: boolean;
  onOpenMethodLibrary: () => void;
  onOpenInspector: () => void;
  onOpenSessions: () => void;
  onReviewContext: () => void;
  onReviewChanges: () => void;
  onReviewRequest: (intent: string) => Promise<void>;
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
  methodGuidanceState,
  methodLibraryAvailable,
  onOpenMethodLibrary,
  onOpenInspector,
  onOpenSessions,
  onReviewContext,
  onReviewChanges,
  onReviewRequest,
  proposalState,
  sessionTitle,
  workspaceName,
}: TaskWorkspaceProps) {
  const [draft, setDraft] = useState("");
  const [submittedTask, setSubmittedTask] = useState<string | null>(null);
  const currentStage = getCurrentStage(proposalState, isNewSession && !submittedTask);
  const methodGuidanceView = methodGuidanceAvailable
    || submittedTask !== null
    || methodGuidanceState.kind !== "idle";
  const guidancePending = methodGuidanceState.kind === "creating"
    || methodGuidanceState.kind === "review_required"
    || methodGuidanceState.kind === "approving"
    || methodGuidanceState.kind === "approved"
    || methodGuidanceState.kind === "submitting";
  const checkingRetainedGuidance = methodGuidanceState.kind === "creating"
    && methodGuidanceState.activity === "recovering";
  const failedSubmission = submittedTask !== null && methodGuidanceState.kind === "unavailable";
  const composerDisabled = !methodGuidanceAvailable || guidancePending || failedSubmission;

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
    await onReviewRequest(value).catch(() => undefined);
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
              ? "Reviewing an intent creates an inert local Method run and prepares the exact outbound context. Nothing is sent until you approve context and choose Send request."
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
            {methodGuidanceState.kind === "creating" ? (
              <article className="message message--agent message--compact" role="status">
                <div className="message__avatar">
                  <BrandMark size={22} />
                </div>
                <div>
                  <span className="message__label">
                    {checkingRetainedGuidance ? "Checking · Local only" : "Preparing · Local only"}
                  </span>
                  <p>
                    {checkingRetainedGuidance
                      ? "Checking for a retained Method result without sending a model request or changing the workspace…"
                      : "Preparing the exact outbound review. Nothing is sent until you approve context and choose Send request…"}
                  </p>
                </div>
              </article>
            ) : methodGuidanceState.kind === "review_required"
              || methodGuidanceState.kind === "approving"
              || methodGuidanceState.kind === "approved" ? (
              <article className="message message--agent message--compact" role="status">
                <div className="message__avatar">
                  <BrandMark size={22} />
                </div>
                <div>
                  <span className="message__label">
                    {methodGuidanceState.kind === "review_required"
                      ? "Review required · Nothing sent"
                      : methodGuidanceState.kind === "approving"
                        ? "Approving · Nothing sent"
                        : "Approved · Ready to send"}
                  </span>
                  <p>
                    Inspect the exact context and continue in the Method inspector. Approval alone
                    does not contact the model.
                  </p>
                </div>
              </article>
            ) : methodGuidanceState.kind === "submitting" ? (
              <article className="message message--agent message--compact" role="status">
                <div className="message__avatar"><BrandMark size={22} /></div>
                <div><span className="message__label">Sending · One shot</span><p>Sending the approved exact context once…</p></div>
              </article>
            ) : methodGuidanceState.kind === "completed" ? (
              <article className="message message--agent message--compact" role="status">
                <div className="message__avatar"><BrandMark size={22} /></div>
                <div><span className="message__label">Completed · Verified</span><p>Review the canonical recommendation and safe receipt in the Method inspector.</p></div>
              </article>
            ) : methodGuidanceState.kind === "interrupted" ? (
              <article className="message message--agent message--compact" role="alert">
                <div className="message__avatar"><BrandMark size={22} /></div>
                <div><span className="message__label">Interrupted · Cannot resume</span><p>Start a fresh review; this request cannot be sent again.</p></div>
              </article>
            ) : methodGuidanceState.kind === "terminal" ? (
              <article className="message message--agent message--compact" role="status">
                <div className="message__avatar"><BrandMark size={22} /></div>
                <div><span className="message__label">Review ended</span><p>No request authority remains. Start a fresh review when ready.</p></div>
              </article>
            ) : methodGuidanceState.kind === "unavailable" ? (
              <article className="message message--agent message--compact" role="alert">
                <div className="message__avatar"><BrandMark size={22} /></div>
                <div><span className="message__label">Request unavailable</span><p>{methodGuidanceState.message}</p></div>
              </article>
            ) : methodGuidanceState.run ? (
              <article className="message message--agent message--compact" role="status">
                <div className="message__avatar"><BrandMark size={22} /></div>
                <div><span className="message__label">Created · Unbound</span><p>A retained local Method run is available in the Method inspector. No model request was made.</p></div>
              </article>
            ) : submittedTask ? null : (
              <section className="empty-session" aria-labelledby="empty-session-title">
                <div className="empty-session__mark">
                  <BrandMark size={31} />
                </div>
                <h2 id="empty-session-title">What do you want Method guidance for?</h2>
                <p>
                  Describe your intent to create an inert local run and review the exact context.
                  Nothing is sent until you separately approve and send the request.
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
              aria-label={methodGuidanceView ? "Review request" : "Send task"}
              isDisabled={composerDisabled || !draft.trim()}
              size={methodGuidanceView ? "small" : "icon"}
              type="submit"
              variant="secondary"
            >
              <Send aria-hidden="true" size={18} />
              {methodGuidanceView ? "Review request" : null}
            </Button>
          </div>
        </div>
        <p className="composer__availability" id="task-composer-availability">
          {methodGuidanceView
            ? methodGuidanceState.kind === "creating"
              ? checkingRetainedGuidance
                ? "Checking for a retained local Method result. No model request or workspace change is being made."
                : "Preparing the exact outbound review. Nothing is sent until you approve context and choose Send request."
              : "Creates an inert local run and prepares exact outbound context. Nothing is sent until you approve context and choose Send request."
            : "Preview only — submitting tasks and applying changes require a later governed capability build."}
        </p>
      </form>
    </main>
  );
}
