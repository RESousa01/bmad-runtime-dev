import { Button } from "@sapphirus/ui";
import {
  ChevronDown,
  FileCode,
  Pencil,
  Send,
  ShieldAlert,
  Terminal,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { MAX_TASK_TITLE_CHARS } from "../state/taskList";
import type { BmadRequestState } from "../lib/bmadModelProjection";
import type { BmadLibraryUiState } from "../lib/bmadProjection";
import type { ChatMessage, ContextPreviewProjection } from "../lib/hostClient";
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
  /// Renames this task. A name typed here is never overwritten by the
  /// automatic title derived from the first message.
  onRenameTask?: ((title: string) => void) | undefined;
  /// Closes this task: it leaves the tab strip but keeps its transcript.
  onCloseTask?: (() => void) | undefined;
  /// Asks to delete this task. The confirmation lives in the shell, because
  /// destroying a transcript is not something a header button does directly.
  onDeleteTask?: (() => void) | undefined;
  sessionTitle: string;
  workspaceName: string;
  workspaceStatusLabel?: string | undefined;
}

/// How much of a tool result the collapsed summary shows. A result can be 8
/// KiB, so the transcript stays readable by keeping the body behind a
/// disclosure rather than inlining it.
const TOOL_RESULT_PREVIEW_CHARS = 96;

function toolResultPreview(text: string): string {
  const firstLine = (text.split("\n", 1)[0] ?? "").trim();
  return firstLine.length > TOOL_RESULT_PREVIEW_CHARS
    ? `${firstLine.slice(0, TOOL_RESULT_PREVIEW_CHARS)}…`
    : firstLine;
}

function MessageTime({ at }: { readonly at: number }) {
  return (
    <time dateTime={new Date(at).toISOString()}>
      {new Date(at).toLocaleTimeString()}
    </time>
  );
}

/// What the model *asked* the host to run.
///
/// This is a request, not a record of something done on the user's authority,
/// so it is never styled as an assistant reply and never wears the brand mark.
/// The label says who wanted it; the subject is shown verbatim as monospace
/// data.
function ToolCallMessage({
  awaitingResult,
  message,
}: {
  readonly awaitingResult: boolean;
  readonly message: ChatMessage;
}) {
  return (
    <article className="message message--tool message--tool-call">
      <div className="message__avatar message__avatar--tool">
        <Terminal size={15} />
      </div>
      <div>
        <p className="message__label">
          {awaitingResult
            ? "Tool requested by the model · awaiting result"
            : "Tool requested by the model"}
        </p>
        <p className="message__tool-subject">{message.text}</p>
        <MessageTime at={message.createdAt} />
      </div>
    </article>
  );
}

/// What the host got back for a tool call.
///
/// This is workspace-derived content: attacker-influenced by definition, and
/// never Forge speaking. It is therefore collapsed by default, labelled with
/// its provenance, and rendered as inert monospace data rather than prose — so
/// a file that contains something shaped like an assistant reply cannot be
/// mistaken for one.
function ToolResultMessage({ message }: { readonly message: ChatMessage }) {
  const preview = toolResultPreview(message.text);
  return (
    <article className="message message--tool message--tool-result">
      <div className="message__avatar message__avatar--tool">
        <FileCode size={15} />
      </div>
      <div>
        <details className="tool-result">
          <summary>
            <span className="message__label">
              Tool result · workspace content, not Forge
            </span>
            {preview.length > 0 ? (
              <span className="tool-result__preview">{preview}</span>
            ) : null}
          </summary>
          <pre className="tool-result__body">{message.text}</pre>
        </details>
        <MessageTime at={message.createdAt} />
      </div>
    </article>
  );
}

/// The task's conversation. This is the app's ordinary surface: a plain
/// exchange of messages that needs no BMAD concept to work.
function ChatTranscript({ lane }: { readonly lane: ChatLaneState }) {
  if (lane.messages.length === 0 && lane.pending === null && lane.error === null) {
    return null;
  }
  return (
    // `role="log"` with a polite live region, because this is the one surface
    // where the app tells the user something without being asked.
    //
    // Without it a screen-reader user was told the agent had started — the
    // "Thinking…" placeholder below carries `role="status"` — and then never
    // told it had finished. The reply was appended to a silent container, so
    // the last thing announced was that a request was in flight, indefinitely.
    // `aria-relevant="additions"` keeps that to newly appended turns rather
    // than re-reading the transcript whenever anything in it re-renders.
    <section
      aria-label="Conversation"
      aria-live="polite"
      aria-relevant="additions"
      className="chat-transcript"
      role="log"
    >
      {lane.messages.map((message, index) => {
        const key = `${message.createdAt}-${index}`;
        // The two tool roles are deliberately not folded into the assistant
        // branch. Falling through to `message--agent` would put the brand mark
        // on a file's contents and present it as Forge's own words.
        if (message.role === "tool_call") {
          return (
            <ToolCallMessage
              awaitingResult={lane.messages[index + 1]?.role !== "tool_result"}
              key={key}
              message={message}
            />
          );
        }
        if (message.role === "tool_result") {
          return <ToolResultMessage key={key} message={message} />;
        }
        return (
          <article
            className={message.role === "user" ? "message message--user" : "message message--agent"}
            key={key}
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
              <MessageTime at={message.createdAt} />
            </div>
          </article>
        );
      })}
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
  onRenameTask,
  onCloseTask,
  onDeleteTask,
  sessionTitle,
  workspaceName,
  workspaceStatusLabel,
}: TaskWorkspaceProps) {
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const attachMenuRef = useRef<HTMLDivElement>(null);
  const renameInputRef = useRef<HTMLInputElement>(null);
  const headerRef = useRef<HTMLElement>(null);
  const scrollRegionRef = useRef<HTMLDivElement>(null);
  /// Whether the user is reading the newest turn. Starts true so the first
  /// reply of a task is followed, and is re-evaluated on every scroll.
  const anchoredToBottomRef = useRef(true);
  const [attachMenuOpen, setAttachMenuOpen] = useState(false);
  const [draft, setDraft] = useState(initialDraft);
  const [renameDraft, setRenameDraft] = useState<string | null>(null);
  const renaming = renameDraft !== null;

  function startRename() {
    setRenameDraft(sessionTitle);
  }

  function cancelRename() {
    setRenameDraft(null);
    // The trigger is re-rendered by leaving rename mode, so restore focus once
    // it is back rather than dropping the user on <body>.
    window.requestAnimationFrame(() => {
      headerRef.current
        ?.querySelector<HTMLElement>(".task-rename-trigger")
        ?.focus();
    });
  }

  function commitRename(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (renameDraft === null) return;
    onRenameTask?.(renameDraft);
    setRenameDraft(null);
    window.requestAnimationFrame(() => {
      headerRef.current
        ?.querySelector<HTMLElement>(".task-rename-trigger")
        ?.focus();
    });
  }

  useEffect(() => {
    if (renaming) {
      renameInputRef.current?.select();
    }
  }, [renaming]);

  function updateDraft(value: string) {
    setDraft(value);
    onDraftChange?.(value);
  }

  /// A few pixels of slack, so a fractional scroll height or a sub-pixel
  /// rounding difference does not read as "the user scrolled up".
  const bottomAnchorSlackPx = 24;

  function rememberScrollAnchor() {
    const region = scrollRegionRef.current;
    if (region === null) return;
    const distanceFromBottom = region.scrollHeight - region.scrollTop - region.clientHeight;
    anchoredToBottomRef.current = distanceFromBottom <= bottomAnchorSlackPx;
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

  // Follow the conversation as it grows.
  //
  // There was no autoscroll anywhere in this app: every reply landed below the
  // fold and the user had to scroll to find out that anything had happened at
  // all. The anchor makes it a *stick to the bottom* rather than a jump —
  // scroll up to read an earlier turn and the view stays where it was put,
  // scroll back down and it resumes following.
  const transcriptSignature = [
    chatLane.messages.length,
    chatLane.pending ?? "",
    chatLane.error ?? "",
    methodGuidanceState.kind,
  ].join(" ");

  useEffect(() => {
    const region = scrollRegionRef.current;
    if (region === null || !anchoredToBottomRef.current) {
      return;
    }
    region.scrollTop = region.scrollHeight;
  }, [transcriptSignature]);
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
      <header className="task-header" ref={headerRef}>
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
          {onRenameTask === undefined ? null : (
            <Button
              aria-label={`Rename task ${sessionTitle}`}
              className="task-rename-trigger"
              onPress={startRename}
              size="small"
              variant="quiet"
            >
              <Pencil aria-hidden="true" size={14} />
              Rename
            </Button>
          )}
          {onCloseTask === undefined ? null : (
            <Button onPress={onCloseTask} size="small" variant="quiet">
              <X aria-hidden="true" size={14} />
              Close task
            </Button>
          )}
          {onDeleteTask === undefined ? null : (
            <Button
              className="task-header__destructive"
              onPress={onDeleteTask}
              size="small"
              variant="quiet"
            >
              <Trash2 aria-hidden="true" size={14} />
              Delete task
            </Button>
          )}
        </div>
        {/*
          The heading carries the task's name for assistive tech and for the
          base shell. The v2 shell hides `.task-title-row` visually because the
          tab strip is where the name is read, so the rename *control* lives in
          the visible actions row above rather than next to this heading.
        */}
        <div className="task-title-row">
          <span className="task-kicker">Task</span>
          <h1>{sessionTitle}</h1>
        </div>
        {renaming ? (
          <form
            aria-label="Rename task"
            className="task-rename"
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                event.stopPropagation();
                cancelRename();
              }
            }}
            onSubmit={commitRename}
          >
            <label htmlFor="task-rename">Task name</label>
            <input
              autoComplete="off"
              id="task-rename"
              maxLength={MAX_TASK_TITLE_CHARS}
              onChange={(event) => setRenameDraft(event.target.value)}
              ref={renameInputRef}
              spellCheck={false}
              type="text"
              value={renameDraft}
            />
            <Button size="small" type="submit" variant="primary">Save name</Button>
            <Button onPress={cancelRename} size="small" variant="quiet">Cancel</Button>
          </form>
        ) : null}
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

      <div
        className="task-scroll-region"
        onScroll={rememberScrollAnchor}
        ref={scrollRegionRef}
      >
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
        {/*
          The transcript is the record of what was said. `onSendRequest` is the
          chat lane, which echoes the user's own message back into `chatLane`,
          so re-rendering `submittedTask` here as a second user bubble showed
          every sent message twice. `submittedTask` still gates the guidance
          view, the failed-submission state, and the title.
        */}
        <ChatTranscript lane={chatLane} />
        {methodGuidanceView ? (
          <>
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
            {/*
              The submit control stays mounted and goes disabled while a turn is
              in flight; it is never swapped for a different action.

              An in-place swap to a "Stop" control was tried and reverted. React
              reconciles both branches of a ternary to the same <button> node, so
              the second click of a double-click landed on whatever replaced the
              send button — abandoning the very turn the first click started, and
              dropping the host's reply. That is the same lost-message failure
              `sendChatMessage`'s `sending` guard exists to prevent, so an abandon
              affordance must not occupy the submit slot.
            */}
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
