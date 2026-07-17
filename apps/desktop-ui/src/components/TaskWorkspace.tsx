import { Button } from "@sapphirus/ui";
import {
  Bot,
  ChevronDown,
  ChevronRight,
  FileCode2,
  History,
  Library,
  ListChecks,
  Menu,
  Paperclip,
  Send,
  ShieldAlert,
} from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import type { BmadRequestState } from "../lib/bmadModelProjection";
import type { ContextPreviewProjection } from "../lib/hostClient";
import { BrandMark } from "./BrandMark";

export interface TaskWorkspaceProps {
  canAttachFiles: boolean;
  contextPreview: ContextPreviewProjection | null;
  hostStatusLabel: string;
  interactionDisabled: boolean;
  isBrowserDemo: boolean;
  isInert?: boolean;
  isNewSession: boolean;
  isReadOnlyRecovery: boolean;
  methodGuidanceAvailable: boolean;
  methodGuidanceState: BmadRequestState;
  methodLibraryAvailable: boolean;
  modelAccessDetail: string;
  modelAccessLabel: string;
  onAttachFiles: () => void;
  onOpenAgentSettings: (returnFocusTarget: HTMLElement | null) => void;
  onOpenChanges: () => void;
  onOpenMethodLibrary: () => void;
  onOpenRunDetails: () => void;
  onOpenSidebar: () => void;
  onReviewRequest: (intent: string) => Promise<void>;
  sessionTitle: string;
  workspaceName: string;
}

export function TaskWorkspace({
  canAttachFiles,
  contextPreview,
  hostStatusLabel,
  interactionDisabled,
  isBrowserDemo,
  isInert = false,
  isNewSession,
  isReadOnlyRecovery,
  methodGuidanceAvailable,
  methodGuidanceState,
  methodLibraryAvailable,
  modelAccessDetail,
  modelAccessLabel,
  onAttachFiles,
  onOpenAgentSettings,
  onOpenChanges,
  onOpenMethodLibrary,
  onOpenRunDetails,
  onOpenSidebar,
  onReviewRequest,
  sessionTitle,
  workspaceName,
}: TaskWorkspaceProps) {
  const agentControlRef = useRef<HTMLDivElement>(null);
  const [agentControlOpen, setAgentControlOpen] = useState(false);
  const [draft, setDraft] = useState("");
  const [submittedTask, setSubmittedTask] = useState<string | null>(null);
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
  const composerDisabled = interactionDisabled || !methodGuidanceAvailable || guidancePending || failedSubmission;

  function closeAgentControl(restoreFocus = false) {
    const trigger = agentControlRef.current?.querySelector<HTMLButtonElement>(".agent-control__trigger");
    setAgentControlOpen(false);
    if (restoreFocus) {
      window.requestAnimationFrame(() => trigger?.isConnected && trigger.focus());
    }
  }

  useEffect(() => {
    if (!agentControlOpen) return undefined;
    function closeOnOutsidePointer(event: PointerEvent) {
      if (!agentControlRef.current?.contains(event.target as Node)) {
        setAgentControlOpen(false);
      }
    }
    document.addEventListener("pointerdown", closeOnOutsidePointer);
    return () => document.removeEventListener("pointerdown", closeOnOutsidePointer);
  }, [agentControlOpen]);

  async function submitTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (composerDisabled) {
      return;
    }
    const value = draft.trim();
    if (!value) {
      return;
    }
    try {
      await onReviewRequest(value);
      setSubmittedTask(value);
      setDraft("");
    } catch {
      setDraft(value);
    }
  }

  return (
    <main className={`task-workspace ${isReadOnlyRecovery ? "has-recovery" : ""}`} inert={isInert}>
      <header className="task-header">
        <div className="task-header__workspace">
          <Button
            aria-label="Open task navigation"
            className="mobile-panel-button"
            onPress={onOpenSidebar}
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
          <Button onPress={onOpenChanges} size="small" variant="quiet">
            <ListChecks aria-hidden="true" size={16} />
            Changes
          </Button>
          <Button onPress={onOpenRunDetails} size="small" variant="quiet">
            <History aria-hidden="true" size={16} />
            Run details
          </Button>
          {methodLibraryAvailable ? (
            <Button
              aria-label="Skills and agents"
              className="method-library-trigger"
              onPress={onOpenMethodLibrary}
              size="small"
              variant="secondary"
            >
              <Library aria-hidden="true" size={16} />
              Skills and agents
            </Button>
          ) : null}
        </div>
        <div className="task-title-row">
          <span className="task-kicker">Task</span>
          <h1>{isNewSession && !submittedTask ? "New task" : sessionTitle}</h1>
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
        {!isBrowserDemo ? (
          <div className="preview-notice" role="note">
            <strong>{methodGuidanceView ? "Local skill guidance" : "Current local product"}</strong>
            <span>
              {methodGuidanceView
                ? "Reviewing an intent creates an inert local BMAD Help run and prepares the exact outbound context. Nothing is sent until you approve context and choose Send request."
                : "This workspace is open locally. Files and governed changes remain available; model-backed skill guidance stays disabled until access is ready."}
            </span>
          </div>
        ) : null}
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
                      ? "Checking for retained skill guidance without sending a model request or changing the workspace…"
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
                    Inspect the exact context in Skills and agents. Approval alone
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
                <div><span className="message__label">Completed · Verified</span><p>Review the canonical recommendation and safe receipt in Skills and agents.</p></div>
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
                <div><span className="message__label">Created · Unbound</span><p>A retained local BMAD Help run is available in Skills and agents. No model request was made.</p></div>
              </article>
            ) : submittedTask ? null : (
              <section className="empty-session" aria-labelledby="empty-session-title">
                <div className="empty-session__mark">
                  <BrandMark size={31} />
                </div>
                <h2 id="empty-session-title">What do you want skill guidance for?</h2>
                <p>
                  Describe your intent to create an inert local run and review the exact context.
                  Nothing is sent until you separately approve and send the request.
                </p>
              </section>
            )}
          </>
        ) : (
          <section className="empty-session" aria-labelledby="empty-session-title">
            <div className="empty-session__mark">
              <BrandMark size={31} />
            </div>
            <h2 id="empty-session-title">
              {isBrowserDemo ? "Explore Sapphirus safely" : "What would you like to work on?"}
            </h2>
            <p>
              {isBrowserDemo
                ? "Browse the included sample project structure and explore the desktop task flow."
                : methodGuidanceAvailable
                  ? "Ask the Agent to inspect your local workspace, explain code, create a plan, or prepare skill-guided work."
                  : "Use Files to inspect local context or Changes to review governed workspace activity. Agent requests stay disabled until model access is ready."}
            </p>
          </section>
        )}
      </div>

      <form
        aria-label="Task composer"
        className="composer"
        onKeyDown={(event) => {
          if (event.key === "Escape" && agentControlOpen) {
            event.preventDefault();
            closeAgentControl(true);
          }
        }}
        onSubmit={submitTask}
      >
        {contextPreview && contextPreview.items.length > 0 ? (
          <section className="attached-context" aria-labelledby="attached-context-title">
            <div className="attached-context__heading" id="attached-context-title">
              <FileCode2 aria-hidden="true" size={15} />
              Local context preview
            </div>
            <ul className="attached-context__chips">
              {contextPreview.items.map((item) => (
                <li key={item.relativePath} title={item.relativePath}>
                  {item.relativePath}
                </li>
              ))}
            </ul>
            <p className="attached-context__note">
              Reviewed locally. Files are not included in a model request unless they appear in the exact request review.
            </p>
          </section>
        ) : null}
        <div className="composer__input-row">
          <Button
            aria-label="Attach files"
            isDisabled={!canAttachFiles}
            onPress={onAttachFiles}
            size="small"
            variant="quiet"
          >
            <Paperclip aria-hidden="true" size={19} />
            Attach files
          </Button>
          <label className="sr-only" htmlFor="task-composer">
            {methodGuidanceView
              ? "Describe what you want skill guidance for"
              : "Describe a task"}
          </label>
          <textarea
            id="task-composer"
            aria-describedby="task-composer-availability"
            disabled={composerDisabled}
            onChange={(event) => setDraft(event.target.value)}
            placeholder={methodGuidanceView
              ? "Describe your intent for skill guidance…"
              : "Describe a task, ask a question, or request a review…"}
            rows={2}
            value={draft}
          />
        </div>
        <div className="composer__toolbar">
          <div className="agent-control" ref={agentControlRef}>
            <Button
              aria-expanded={agentControlOpen}
              aria-controls="agent-model-access"
              aria-label="Agent and model settings"
              className="agent-control__trigger"
              onPress={() => setAgentControlOpen((open) => !open)}
              size="small"
              variant="quiet"
            >
              <Bot aria-hidden="true" size={15} />
              <span>Agent</span>
              <span className="agent-control__summary">
                {methodGuidanceView ? "BMAD Help" : modelAccessLabel}
              </span>
              <ChevronDown aria-hidden="true" size={14} />
            </Button>
            {agentControlOpen ? (
              <section
                aria-label="Agent and model"
                className="agent-control__popover"
                id="agent-model-access"
                role="region"
              >
                <header>
                  <h2>Agent configuration</h2>
                  <span className="agent-control__status">
                    <span
                      className={`status-dot ${methodGuidanceAvailable ? "" : "status-dot--warning"}`}
                    />
                    {methodGuidanceAvailable ? "Available" : isBrowserDemo ? "Read only" : "Unavailable"}
                  </span>
                </header>
                <dl className="agent-control__menu">
                  <div><dt>Agent capability</dt><dd>BMAD Help</dd></div>
                  <div><dt>Model access</dt><dd title={modelAccessDetail}>{modelAccessLabel}</dd></div>
                  <div><dt>Request policy</dt><dd>Review before send</dd></div>
                </dl>
                <Button
                  className="agent-control__settings"
                  onPress={() => {
                    const trigger = agentControlRef.current?.querySelector<HTMLButtonElement>(".agent-control__trigger");
                    closeAgentControl();
                    trigger?.focus();
                    onOpenAgentSettings(trigger ?? null);
                  }}
                  size="small"
                  variant="secondary"
                >
                  Open settings
                  <ChevronRight aria-hidden="true" size={15} />
                </Button>
              </section>
            ) : null}
          </div>
          <div className="composer__right">
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
          {isBrowserDemo
            ? "Read only: sample data only. No access to your device or a model."
            : methodGuidanceView
            ? methodGuidanceState.kind === "creating"
              ? checkingRetainedGuidance
                ? "Checking for retained local skill guidance. No model request or workspace change is being made."
                : "Preparing the exact outbound review. Nothing is sent until you approve context and choose Send request."
              : "Creates an inert local run and prepares exact outbound context. Nothing is sent until you approve context and choose Send request."
            : "This local workspace remains available for Files and governed Changes. Model-backed skill guidance is unavailable until access is ready."}
        </p>
      </form>
    </main>
  );
}
