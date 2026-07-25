import { Button } from "@sapphirus/ui";
import {
  ChevronDown,
  Send,
  ShieldAlert,
} from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import type { BmadRequestState } from "../lib/bmadModelProjection";
import type { BmadLibraryUiState } from "../lib/bmadProjection";
import type { ContextPreviewProjection } from "../lib/hostClient";
import type { ChatLaneState } from "../state/chatSessions";
import { brandIdentity } from "../lib/brand";
import { BrandMark } from "./BrandMark";
import { OpenCodeIcon } from "./OpenCodeIcon";
import {
  ContextWindow,
  type ContextSelectionMode,
} from "./ContextWindow";
import { AgentSelector } from "./composer/AgentSelector";
import "./composer/agent-selector.css";

const STARTER_INTENTS = [
  "Explain how this workspace is structured",
  "Review my recent changes",
  "Plan a focused refactor",
] as const;

export interface TaskWorkspaceProps {
  canAttachFiles: boolean;
  contextPreview: ContextPreviewProjection | null;
  contextSelectionMode: ContextSelectionMode;
  contextSuggestionBusy: boolean;
  hostStatusLabel: string;
  initialDraft?: string | undefined;
  interactionDisabled: boolean;
  isBrowserDemo: boolean;
  isInert?: boolean;
  isNewSession: boolean;
  isReadOnlyRecovery: boolean;
  methodGuidanceAvailable: boolean;
  methodGuidanceState: BmadRequestState;
  /// The task's conversation. Chat is the default lane; BMAD is optional.
  chatLane: ChatLaneState;
  /// Explicitly ask the BMAD Help advisor which skill fits — no longer what
  /// an ordinary message does.
  onAskMethodGuidance: (intent: string) => Promise<void>;
  /// Bind or clear the BMAD agent persona for this conversation.
  onSelectAgent: (agentCode: string | null) => void;
  methodLibraryAvailable: boolean;
  modelAccessDetail: string;
  modelAccessLabel: string;
  onAttachFiles?: (() => Promise<void>) | undefined;
  onAttachFolder?: (() => Promise<void>) | undefined;
  onClearContext: () => void;
  agentLibrary: BmadLibraryUiState;
  attachNotice: string | null;
  onOpenChanges: () => void;
  onOpenContext?: (() => void) | undefined;
  onOpenMethodLibrary: () => void;
  onOpenProviders?: (() => void) | undefined;
  onOpenRunDetails: () => void;
  onSuggestContext: (intent: string) => Promise<void>;
  onDraftChange?: ((draft: string) => void) | undefined;
  onOpenWorkspaceManager?: (() => void) | undefined;
  onSendRequest: (intent: string) => Promise<void>;
  sessionTitle: string;
  workspaceName: string;
  workspaceStatusLabel?: string | undefined;
}

/// The task's conversation. This is the app's ordinary surface: a plain
/// exchange of messages that needs no BMAD concept to work.
function ChatTranscript({ lane }: { readonly lane: ChatLaneState }) {
  if (lane.messages.length === 0 && lane.pending === null && lane.error === null) {
    return null;
  }
  return (
    <section aria-label="Conversation" className="chat-transcript">
      {lane.messages.map((message, index) => (
        <article
          className={message.role === "user" ? "message message--user" : "message message--agent"}
          key={`${message.createdAt}-${index}`}
        >
          <div
            className={
              message.role === "user"
                ? "message__avatar message__avatar--user"
                : "message__avatar"
            }
          >
            {message.role === "user" ? "You" : <BrandMark size={22} />}
          </div>
          <div>
            <p>{message.text}</p>
            <time dateTime={new Date(message.createdAt).toISOString()}>
              {new Date(message.createdAt).toLocaleTimeString()}
            </time>
          </div>
        </article>
      ))}
      {lane.pending !== null ? (
        <>
          <article className="message message--user">
            <div className="message__avatar message__avatar--user">You</div>
            <div>
              <p>{lane.pending}</p>
            </div>
          </article>
          <article className="message message--agent message--compact" role="status">
            <div className="message__avatar">
              <BrandMark size={22} />
            </div>
            <div>
              <p>Thinking…</p>
            </div>
          </article>
        </>
      ) : null}
      {lane.error !== null ? (
        <p className="chat-transcript__error" role="alert">
          {lane.error}
        </p>
      ) : null}
    </section>
  );
}

export function TaskWorkspace({
  canAttachFiles,
  contextPreview,
  contextSelectionMode,
  contextSuggestionBusy,
  hostStatusLabel,
  initialDraft = "",
  interactionDisabled,
  isBrowserDemo,
  isInert = false,
  isNewSession,
  isReadOnlyRecovery,
  methodGuidanceAvailable,
  methodGuidanceState,
  chatLane,
  onAskMethodGuidance,
  onSelectAgent,
  methodLibraryAvailable,
  modelAccessDetail,
  modelAccessLabel,
  onAttachFiles,
  onAttachFolder,
  onClearContext,
  agentLibrary,
  attachNotice,
  onOpenChanges,
  onOpenContext,
  onOpenMethodLibrary,
  onOpenProviders,
  onOpenRunDetails,
  onSuggestContext,
  onDraftChange,
  onOpenWorkspaceManager,
  onSendRequest,
  sessionTitle,
  workspaceName,
  workspaceStatusLabel,
}: TaskWorkspaceProps) {
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const attachMenuRef = useRef<HTMLDivElement>(null);
  const [attachMenuOpen, setAttachMenuOpen] = useState(false);
  const [draft, setDraft] = useState(initialDraft);

  function updateDraft(value: string) {
    setDraft(value);
    onDraftChange?.(value);
  }

  useEffect(() => {
    if (!attachMenuOpen) return undefined;
    function closeOnOutsidePointer(event: PointerEvent) {
      if (!attachMenuRef.current?.contains(event.target as Node)) {
        setAttachMenuOpen(false);
      }
    }
    document.addEventListener("pointerdown", closeOnOutsidePointer);
    return () => document.removeEventListener("pointerdown", closeOnOutsidePointer);
  }, [attachMenuOpen]);
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
  // A chat turn in flight disables the composer for the same reason a Help
  // request does: the transcript accepts one question at a time.
  const composerDisabled =
    interactionDisabled || guidancePending || failedSubmission || chatLane.sending;
  const submitDisabled = composerDisabled || !methodGuidanceAvailable;

  async function submitTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (submitDisabled) {
      return;
    }
    const value = draft.trim();
    if (!value) {
      return;
    }
    try {
      await onSendRequest(value);
      setSubmittedTask(value);
      updateDraft("");
    } catch {
      updateDraft(value);
    }
  }

  return (
    <main
      className={`task-workspace ${isReadOnlyRecovery ? "has-recovery" : ""} task-workspace--opencode-v2`}
      inert={isInert}
    >
      <header className="task-header">
        <div className="task-header__workspace">
          <strong>{workspaceName}</strong>
          <span className="workspace-status">
            <span className="workspace-status__folder" aria-hidden="true" />
            {hostStatusLabel}
            <span aria-hidden="true" className={`status-dot ${isReadOnlyRecovery ? "status-dot--warning" : ""}`} />
          </span>
        </div>
        <div className="task-header__actions">
          <Button onPress={onOpenChanges} size="small" variant="quiet">
            <OpenCodeIcon name="review" />
            Review
          </Button>
          <Button onPress={onOpenRunDetails} size="small" variant="quiet">
            <OpenCodeIcon name="status" />
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
              <OpenCodeIcon name="grid-plus" />
              Skills and agents
            </Button>
          ) : null}
        </div>
        <div className="task-title-row">
          <span className="task-kicker">Task</span>
          <h1>{isNewSession && !submittedTask && sessionTitle === "New task" ? "New task" : sessionTitle}</h1>
        </div>
      </header>

      {isReadOnlyRecovery ? (
        <div className="recovery-banner" role="status">
          <ShieldAlert aria-hidden="true" size={17} />
          <div>
            <strong>Read-only recovery</strong>
            <span>{brandIdentity.displayName} could not verify its local workspace data. Workspace changes remain blocked.</span>
          </div>
        </div>
      ) : null}

      <div className="task-scroll-region">
        {!isBrowserDemo ? (
          <div className="preview-notice" role="note">
            <strong>{methodGuidanceView ? "BMAD Help" : "Current local product"}</strong>
            <span>
              {methodGuidanceView
                ? "Send a message to BMAD Help. Local files are included only when you add them."
                : "This workspace is open locally. Files and governed changes remain available; model-backed skill guidance stays disabled until access is ready."}
            </span>
          </div>
        ) : null}
        <ChatTranscript lane={chatLane} />
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
                    {checkingRetainedGuidance ? "Checking" : "Preparing"}
                  </span>
                  <p>
                    {checkingRetainedGuidance
                      ? "Checking for a retained BMAD session…"
                      : "Preparing and sending your message…"}
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
                      ? "Preparing attached files"
                      : methodGuidanceState.kind === "approving"
                        ? "Preparing attached files"
                        : "Sending"}
                  </span>
                  <p>
                    Sending your message with the files you attached.
                  </p>
                </div>
              </article>
            ) : methodGuidanceState.kind === "submitting" ? (
              <article className="message message--agent message--compact" role="status">
                <div className="message__avatar"><BrandMark size={22} /></div>
                <div><span className="message__label">Sending</span><p>Waiting for the selected model…</p></div>
              </article>
            ) : methodGuidanceState.kind === "completed" ? (
              <article className="message message--agent message--compact" role="status">
                <div className="message__avatar"><BrandMark size={22} /></div>
                <div>
                  <span className="message__label">BMAD Help</span>
                  <p>
                    {methodGuidanceState.result.recommendation.recommendationKind === "recommended_capability"
                      ? `${methodGuidanceState.result.recommendation.displayName}: ${methodGuidanceState.result.recommendation.rationaleSummary}`
                      : "BMAD could not identify a supported next action from the available project context."}
                  </p>
                </div>
              </article>
            ) : methodGuidanceState.kind === "interrupted" ? (
              <article className="message message--agent message--compact" role="alert">
                <div className="message__avatar"><BrandMark size={22} /></div>
                <div><span className="message__label">Interrupted</span><p>This message cannot resume. Send it again.</p></div>
              </article>
            ) : methodGuidanceState.kind === "terminal" ? (
              <article className="message message--agent message--compact" role="status">
                <div className="message__avatar"><BrandMark size={22} /></div>
                <div><span className="message__label">Message cancelled</span><p>Nothing was sent. You can edit the message and try again.</p></div>
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
                <h2 id="empty-session-title">What do you want to build?</h2>
                <p>
                  Describe what you want to accomplish. Add local files only when they help.
                </p>
                {composerDisabled ? null : (
                  <ul aria-label="Starter intents" className="empty-session__starters">
                    {STARTER_INTENTS.map((intent) => (
                      <li key={intent}>
                        <button
                          onClick={() => {
                            updateDraft(intent);
                            composerRef.current?.focus();
                          }}
                          type="button"
                        >
                          {intent}
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </section>
            )}
          </>
        ) : (
          <section className="empty-session" aria-labelledby="empty-session-title">
            <h2 id="empty-session-title">What do you want to build?</h2>
            <p>
              {isBrowserDemo
                ? "Browse the included sample project structure and explore the desktop task flow."
                : methodGuidanceAvailable
                  ? "Ask the Agent to inspect your local workspace, explain code, create a plan, or prepare skill-guided work."
                  : "Connect OpenAI in Settings to send prompts. Files and Review remain available while disconnected."}
            </p>
          </section>
        )}
      </div>

      <form aria-label="Task composer" className="composer" onSubmit={submitTask}>
        <ContextWindow
          canSuggest={canAttachFiles && draft.trim().length > 0}
          onClear={onClearContext}
          onOpen={onOpenContext}
          onSuggest={() => {
            const intent = draft.trim();
            if (intent.length > 0) void onSuggestContext(intent);
          }}
          preview={contextPreview}
          selectionMode={contextSelectionMode}
          suggestionBusy={contextSuggestionBusy}
        />
        {attachNotice ? (
          <p className="attach-notice" role="status">{attachNotice}</p>
        ) : null}
        <div className="composer__input-row">
          <div
            className="attach-menu"
            onKeyDown={(event) => {
              if (event.key === "Escape" && attachMenuOpen) {
                event.preventDefault();
                event.stopPropagation();
                setAttachMenuOpen(false);
              }
            }}
            ref={attachMenuRef}
          >
            <Button
              aria-label="Add files or folder"
              aria-expanded={attachMenuOpen}
              aria-haspopup="menu"
              isDisabled={!canAttachFiles || (!onAttachFiles && !onAttachFolder)}
              onPress={() => setAttachMenuOpen((open) => !open)}
              size="small"
              variant="quiet"
            >
              <OpenCodeIcon name="folder-add-left" />
              <span className="composer__attach-label">Add context</span>
            </Button>
            {attachMenuOpen ? (
              <div className="attach-menu__popover" role="menu">
                <button
                  disabled={!onAttachFiles}
                  onClick={() => {
                    setAttachMenuOpen(false);
                    void onAttachFiles?.();
                  }}
                  role="menuitem"
                  type="button"
                >
                  Add files…
                </button>
                <button
                  disabled={!onAttachFolder}
                  onClick={() => {
                    setAttachMenuOpen(false);
                    void onAttachFolder?.();
                  }}
                  role="menuitem"
                  type="button"
                >
                  Add folder…
                </button>
              </div>
            ) : null}
          </div>
          <label className="sr-only" htmlFor="task-composer">
            {methodGuidanceView
              ? "Describe what you want skill guidance for"
              : "Describe a task"}
          </label>
          <textarea
            id="task-composer"
            ref={composerRef}
            aria-describedby="task-composer-availability"
            disabled={composerDisabled}
            onChange={(event) => updateDraft(event.target.value)}
            placeholder={methodGuidanceView
              ? "Ask BMAD Help…"
              : "Ask Forge…"}
            rows={2}
            value={draft}
          />
        </div>
        <div className="composer__toolbar">
          <AgentSelector
            library={agentLibrary}
            modelAccessDetail={modelAccessDetail}
            modelAccessLabel={modelAccessLabel}
            onOpenMethodLibrary={methodLibraryAvailable ? onOpenMethodLibrary : undefined}
            onOpenProviders={onOpenProviders}
            selectedAgentCode={chatLane.agentCode}
            onSelectAgent={onSelectAgent}
            onAskMethodGuidance={draft.trim().length > 0
              ? () => void onAskMethodGuidance(draft.trim()).catch(() => undefined)
              : undefined}
          />
          <div className="composer__right">
            <Button
              aria-label={methodGuidanceView ? "Send message" : "Send task"}
              className="composer__submit"
              isDisabled={submitDisabled || !draft.trim()}
              size="icon"
              title={methodGuidanceView ? "Send message" : "Send task"}
              type="submit"
              variant="primary"
            >
              <Send aria-hidden="true" size={16} />
            </Button>
          </div>
        </div>
        <p
          className="composer__availability sr-only"
          id="task-composer-availability"
        >
          {isBrowserDemo
            ? "Read only: sample data only. No access to your device or a model."
            : methodGuidanceView
            ? methodGuidanceState.kind === "creating"
              ? checkingRetainedGuidance
                ? "Checking for a retained BMAD session."
                : "Preparing and sending your message."
              : "Messages are sent through the selected model with only the files you attach."
            : "Connect OpenAI in Settings to send prompts. Files and Review remain available while disconnected."}
        </p>
      </form>
      {onOpenWorkspaceManager === undefined ? null : (
        <button
          aria-label={`Manage workspace ${workspaceName}`}
          className="workspace-crumb"
          onClick={onOpenWorkspaceManager}
          type="button"
        >
          <span className="workspace-crumb__name">{workspaceName}</span>
          <ChevronDown aria-hidden="true" size={13} strokeWidth={1.8} />
          {workspaceStatusLabel === undefined ? null : (
            <>
              <span aria-hidden="true" className="workspace-crumb__divider">/</span>
              <span>{workspaceStatusLabel}</span>
            </>
          )}
        </button>
      )}
    </main>
  );
}
