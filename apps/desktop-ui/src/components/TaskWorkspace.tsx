import { Button } from "@sapphirus/ui";
import {
  ChevronDown,
  Send,
  ShieldAlert,
} from "lucide-react";
import { Fragment, useEffect, useRef, useState, type FormEvent } from "react";
import { MAX_TASK_TITLE_CHARS } from "../state/taskList";
import type { BmadRequestState } from "../lib/bmadModelProjection";
import type { BmadLibraryUiState } from "../lib/bmadProjection";
import type { ChatMessage } from "../lib/hostClient";
import type { TaskAttachments } from "../features/attachments/attachmentTypes";
import type { ModelContextState } from "../features/model-context/modelContextTypes";
import {
  RETAINED_TRANSCRIPT_MESSAGES,
  type ChatLaneState,
} from "../state/chatSessions";
import { brandIdentity } from "../lib/brand";
import { OpenCodeIcon } from "./OpenCodeIcon";
import {
  ContextWindow,
} from "./ContextWindow";
import { AgentSelector } from "./composer/AgentSelector";
import { useComposer, type ImportOutcome } from "../features/composer/useComposer";
import { buildTimeline, toolResultPreview } from "../features/timeline/timelineModel";
import { taskStatus } from "../features/tasks/taskStatus";
import "./composer/agent-selector.css";

const STARTER_INTENTS = [
  "Explain how this workspace is structured",
  "Review my recent changes",
  "Plan a focused refactor",
] as const;

export interface TaskWorkspaceProps {
  canAttachFiles: boolean;
  /**
   * What *this task* has attached, including its notice and whether an
   * auto-selection is running for it.
   *
   * FE-13: one task-owned value rather than four app-global props. The four
   * could disagree -- a notice from one task shown beside another's file list
   * -- because nothing tied them together.
   */
  attachments: TaskAttachments;
  hostStatusLabel: string;
  initialDraft?: string | undefined;
  interactionDisabled: boolean;
  /**
   * Whether an ordinary chat turn can be sent: a ready host, a granted
   * workspace, and a model provider connected.
   *
   * Deliberately independent of `methodGuidanceAvailable`. Audit finding F-05
   * is that the two were the same signal, so BMAD Help being incomplete
   * disabled ordinary conversation.
   */
  chatSendAvailable: boolean;
  isBrowserDemo: boolean;
  isInert?: boolean;
  isReadOnlyRecovery: boolean;
  methodGuidanceAvailable: boolean;
  methodGuidanceState: BmadRequestState;
  /// The task's conversation. Chat is the default lane; BMAD is optional.
  chatLane: ChatLaneState;
  /// Asks the shell to re-read this task's transcript from the host after a
  /// failed read. Optional so the component still renders in surfaces that
  /// cannot reach the host at all, where the button would be a dead control.
  onRetryHistory?: (() => void) | undefined;
  /// Explicitly ask the BMAD Help advisor which skill fits — no longer what
  /// an ordinary message does.
  onAskMethodGuidance: (intent: string) => Promise<void>;
  /// Bind or clear the BMAD agent persona for this conversation.
  onSelectAgent: (agentCode: string | null) => void;
  methodLibraryAvailable: boolean;
  modelAccessDetail: string;
  modelAccessLabel: string;
  /**
   * What this task's last measured request cost, if one has been measured.
   *
   * Optional because nothing constructs a snapshot yet -- no host command
   * returns one -- so in production this stays `unmeasured` and the Context
   * surface says so. That is the honest state, not a placeholder for one.
   */
  modelContext?: ModelContextState | undefined;
  /**
   * Opens the native file dialog and resolves to what was attached.
   *
   * Resolving to `undefined` means the user cancelled. That distinction is the
   * whole of FE-14's "cancelling produces no toast and no state mutation": a
   * handler that could only say "done" left the composer unable to tell a
   * cancel from an import that attached nothing.
   */
  onAttachFiles?: (() => Promise<ImportOutcome | void>) | undefined;
  onAttachFolder?: (() => Promise<ImportOutcome | void>) | undefined;
  /**
   * Attaches one previously refused file again, resolving to whether it worked.
   *
   * Resolving false rather than throwing, because a file that is still
   * unreadable is the expected outcome of a retry and not an exception.
   */
  onRetryAttachment?: ((relativePath: string) => Promise<boolean>) | undefined;
  /** Drops one attachment, by its opaque id. */
  onRemoveAttachment?: ((attachmentId: string) => void) | undefined;
  onClearContext: () => void;
  agentLibrary: BmadLibraryUiState;
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
function ChatTranscript({
  lane,
  onRetryHistory,
}: {
  readonly lane: ChatLaneState;
  readonly onRetryHistory?: (() => void) | undefined;
}) {
  const historyUnavailable = lane.historyStatus === "unavailable";
  // The host keeps the newest `RETAINED_TRANSCRIPT_MESSAGES` messages and drops
  // whole older turns past that. At the ceiling the user is looking at a window
  // onto the conversation, not the conversation, and is told so rather than
  // being left to assume the beginning is simply missing.
  const atRetentionCeiling = lane.messages.length >= RETAINED_TRANSCRIPT_MESSAGES;
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
      {atRetentionCeiling ? (
        <p className="chat-transcript__notice">
          {`Showing the ${RETAINED_TRANSCRIPT_MESSAGES} most recent messages. `}
          This conversation has reached the limit Forge keeps on this device;
          older turns are dropped as it continues.
        </p>
      ) : null}
      {/*
        Built by `buildTimeline` rather than read off the flat list.

        Whether a call is still outstanding used to be `messages[index + 1]
        ?.role !== "tool_result"` -- a lookahead standing in for a structure,
        and wrong on exactly the transcripts this app produces. A retained
        transcript is truncated to its last N messages, so a slice can start
        with a result whose call was cut off, and the lookahead reported that
        as a call still in flight. It also told two back-to-back calls they
        were both waiting, because neither was immediately followed by a
        result.
      */}
      {buildTimeline(lane.messages).map((entry, index) => {
        // The two tool roles are deliberately not folded into the assistant
        // branch. Falling through to `message--agent` would put the brand mark
        // on a file's contents and present it as Forge's own words.
        if (entry.kind === "tool") {
          // Two articles, still: the request and what came back are different
          // kinds of content with different provenance, and the result's
          // "workspace content, not Forge" labelling depends on it keeping its
          // own container. Pairing them decides `awaitingResult`, not markup.
          return (
            <Fragment key={`tool-${entry.call.createdAt}-${index}`}>
              <ToolCallMessage
                awaitingResult={entry.result === null}
                message={entry.call}
              />
              {entry.result === null
                ? null
                : <ToolResultMessage message={entry.result} />}
            </Fragment>
          );
        }
        if (entry.kind === "orphan_result") {
          // Its call was truncated away, not cancelled. Dropping it would hide
          // work the app actually did.
          return (
            <ToolResultMessage
              key={`orphan-${entry.result.createdAt}-${index}`}
              message={entry.result}
            />
          );
        }
        const { message } = entry;
        return (
          <article
            className={entry.kind === "user" ? "message message--user" : "message message--agent"}
            key={`${message.createdAt}-${index}`}
          >
            {/* 3a names the speaker in a label and puts the time opposite it,
                then gives the text the whole column. Master audit 7.2 rules out
                chat avatars: a 31px circle repeated down the transcript was
                saying who spoke in a way the label already says, and it cost
                every message a 32px indent it did not need. */}
            <header className="message__meta">
              <span className="message__role">
                {entry.kind === "user" ? "You" : brandIdentity.displayName}
              </span>
              <MessageTime at={message.createdAt} />
            </header>
            <p>{message.text}</p>
          </article>
        );
      })}
      {lane.pending !== null ? (
        <>
          <article className="message message--user">
            <header className="message__meta">
              <span className="message__role">You</span>
            </header>
            <p>{lane.pending}</p>
          </article>
          {/*
            Gated on `sending`, not on `pending`. A failed send now leaves the
            question standing so the user can see and retry what they wrote —
            but nothing is thinking about it, and saying so would be the app
            claiming work it is not doing. The error renders below instead.
          */}
          {lane.sending ? (
          <article className="message message--agent message--compact" role="status">
            <div>
              <p>Thinking…</p>
            </div>
          </article>
          ) : null}
        </>
      ) : null}
      {lane.error !== null ? (
        <div className="chat-transcript__error" role="alert">
          <p>
            {historyUnavailable
              // Naming what is missing matters more than the host's own wording
              // here: an empty transcript with a bare error reads as "nothing
              // was ever said", which is not what happened.
              //
              // It used to add "They are still on this device." The renderer
              // reaches this branch *because* the host refused to hand the
              // transcript back, so it has no basis for that. The host errors
              // that land here include a grant mismatch and an integrity
              // failure raised when a durable transcript exists but cannot be
              // rebuilt — the second is the opposite of what the sentence
              // claimed. Say what is knowable: the read failed, so this may not
              // be the whole conversation.
              ? `Forge could not read this task's earlier messages, so this may not be the whole conversation. ${lane.error}`
              : lane.error}
          </p>
          {historyUnavailable && onRetryHistory ? (
            <Button
              onPress={onRetryHistory}
              size="small"
              variant="secondary"
            >
              Retry loading messages
            </Button>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

export function TaskWorkspace({
  canAttachFiles,
  attachments,
  hostStatusLabel,
  initialDraft = "",
  interactionDisabled,
  chatSendAvailable,
  isBrowserDemo,
  isInert = false,
  isReadOnlyRecovery,
  methodGuidanceAvailable,
  methodGuidanceState,
  chatLane,
  onRetryHistory,
  onAskMethodGuidance,
  onSelectAgent,
  methodLibraryAvailable,
  modelAccessDetail,
  modelAccessLabel,
  modelContext,
  onAttachFiles,
  onAttachFolder,
  onRetryAttachment,
  onRemoveAttachment,
  onClearContext,
  agentLibrary,
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
  const renameInputRef = useRef<HTMLInputElement>(null);
  const headerRef = useRef<HTMLElement>(null);
  const scrollRegionRef = useRef<HTMLDivElement>(null);
  /// Whether the user is reading the newest turn. Starts true so the first
  /// reply of a task is followed, and is re-evaluated on every scroll.
  const anchoredToBottomRef = useRef(true);
  const [renameDraft, setRenameDraft] = useState<string | null>(null);
  const renaming = renameDraft !== null;
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
  /*
   * Everything that withholds the composer from outside it.
   *
   * A chat turn in flight disables it for the same reason a Help request does:
   * the transcript accepts one question at a time. These are permissions, not
   * phases -- FE-14's machine owns what the composer is *doing*, and cannot be
   * allowed to transition its way out of a lock the shell imposed.
   *
   * `chatLane.sending` stays in the list even though the machine has a
   * `sending` phase of its own. They are not redundant: the lane is the host's
   * view, which arrives a render late, and the machine is the synchronous one
   * that closes the double-click window the lane leaves open.
   */
  const composerDisabled =
    interactionDisabled || guidancePending || failedSubmission || chatLane.sending;

  const composer = useComposer({ initialDraft, locked: composerDisabled, onDraftChange });
  const draft = composer.draft;
  const status = taskStatus({
    chatLane,
    guidance: methodGuidanceState,
    readOnlyRecovery: isReadOnlyRecovery,
    selectingContext: attachments.suggesting,
  });

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

  /// A few pixels of slack, so a fractional scroll height or a sub-pixel
  /// rounding difference does not read as "the user scrolled up".
  const bottomAnchorSlackPx = 24;

  function rememberScrollAnchor() {
    const region = scrollRegionRef.current;
    if (region === null) return;
    const distanceFromBottom = region.scrollHeight - region.scrollTop - region.clientHeight;
    anchoredToBottomRef.current = distanceFromBottom <= bottomAnchorSlackPx;
  }

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
  ].join("\u0000");

  useEffect(() => {
    const region = scrollRegionRef.current;
    if (region === null || !anchoredToBottomRef.current) {
      return;
    }
    region.scrollTop = region.scrollHeight;
  }, [transcriptSignature]);
  /*
   * Sending gates on whether *chat* can be sent, not on whether BMAD Help is
   * available. This used to read `|| !methodGuidanceAvailable`, which is audit
   * finding F-05: an incomplete BMAD Help capability made a host perfectly able
   * to hold an ordinary conversation present itself as unable to send anything.
   *
   * Ordinary conversation is not a Method feature. The two signals are separate
   * props now so one cannot be reached for in place of the other.
   */
  const canSubmit = composer.canSend && chatSendAvailable;

  function submitTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!chatSendAvailable) return;
    /*
     * The guard is inside `composer.send`, checked against a ref rather than
     * this render's state. That is the whole double-submit fix: the second
     * click of a double-click arrives before React has re-rendered with the
     * first click's phase, so a check against `canSubmit` here would read the
     * value from before the first click and let the second through.
     */
    composer.send(async (intent) => {
      await onSendRequest(intent);
      setSubmittedTask(intent);
    });
  }

  /// Opens the native dialog under the machine's guard, so a second press
  /// while one is open is dropped rather than stacking a second dialog.
  function pickAttachments(open: (() => Promise<ImportOutcome | void>) | undefined) {
    if (open === undefined) return;
    composer.openFileDialog(open);
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
          {/*
            What *this task* is doing, beside what the host is.

            The header carried only the host label, so a task waiting on an
            approval and a task with nothing happening looked identical -- and
            the one signal the header did carry described the connection rather
            than the work. `taskStatus` owns the precedence between the several
            states that are true at once; the only decision here is that a
            state the user must act on is announced and the rest are not,
            because a live region that reports every transition to "Working" is
            one the important announcement arrives inside of.
          */}
          <span
            className={`task-status task-status--${status.tone}`}
            role={status.announce ? "status" : undefined}
          >
            <span aria-hidden="true" className="task-status__dot" />
            {status.label}
          </span>
        </div>
        <div className="task-header__actions">
          <Button onPress={onOpenChanges} size="small" variant="quiet">
            Review
          </Button>
          <Button onPress={onOpenRunDetails} size="small" variant="quiet">
            Run details
          </Button>
          {methodLibraryAvailable ? (
            <Button
              aria-label="Skills and agents"
              className="method-library-trigger"
              onPress={onOpenMethodLibrary}
              size="small"
              variant="quiet"
            >
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
              Rename
            </Button>
          )}
          {onCloseTask === undefined ? null : (
            <Button onPress={onCloseTask} size="small" variant="quiet">
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
        <ChatTranscript lane={chatLane} onRetryHistory={onRetryHistory} />
        {methodGuidanceView ? (
          <>
            {methodGuidanceState.kind === "creating" ? (
              <article className="message message--agent message--compact" role="status">
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
                <div><span className="message__label">Sending</span><p>Waiting for the selected model…</p></div>
              </article>
            ) : methodGuidanceState.kind === "completed" ? (
              <article className="message message--agent message--compact" role="status">
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
                <div><span className="message__label">Interrupted</span><p>This message cannot resume. Send it again.</p></div>
              </article>
            ) : methodGuidanceState.kind === "terminal" ? (
              <article className="message message--agent message--compact" role="status">
                <div><span className="message__label">Message cancelled</span><p>Nothing was sent. You can edit the message and try again.</p></div>
              </article>
            ) : methodGuidanceState.kind === "unavailable" ? (
              <article className="message message--agent message--compact" role="alert">
                <div><span className="message__label">Request unavailable</span><p>{methodGuidanceState.message}</p></div>
              </article>
            ) : methodGuidanceState.run ? (
              <article className="message message--agent message--compact" role="status">
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
                            composer.setDraft(intent);
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
                  : "Connect a model provider in Settings to send prompts."}
            </p>
            {!isBrowserDemo
              && !isReadOnlyRecovery
              && !interactionDisabled
              && !chatSendAvailable
              && onOpenProviders !== undefined ? (
                <Button
                  onPress={onOpenProviders}
                  size="small"
                  variant="secondary"
                >
                  Connect model
                </Button>
              ) : null}
          </section>
        )}
      </div>

      <form aria-label="Task composer" className="composer" onSubmit={submitTask}>
        {attachments.notice ? (
          <p className="attach-notice" role="status">{attachments.notice}</p>
        ) : null}
        {/* One line per refusal, not one merged sentence. "3 files were
            skipped" tells the user something went wrong and nothing about
            which of their choices to revisit. */}
        {composer.rejections.length > 0 ? (
          <div className="attach-rejections" role="status">
            <ul>
              {composer.rejections.map((rejection) => (
                <li key={`${rejection.relativePath}|${rejection.reason}`}>
                  {rejection.relativePath === ""
                    ? rejection.reason
                    : <><code>{rejection.relativePath}</code> — {rejection.reason}</>}
                  {/*
                    Offered only where the host said another attempt could go
                    differently -- an I/O failure. A binary file will be binary
                    next time and a `.env` will still be sensitive, so a retry
                    on those is a control that cannot succeed, and pressing it
                    teaches the user only that the app is not working.
                  */}
                  {onRetryAttachment !== undefined
                    && composer.canRetryAttachment(rejection.relativePath) ? (
                      <Button
                        onPress={() => {
                          composer.retryAttachment(rejection.relativePath, onRetryAttachment);
                        }}
                        size="small"
                        variant="quiet"
                      >
                        Try again
                      </Button>
                    ) : null}
                  {composer.retryingPath === rejection.relativePath
                    ? <span className="attach-rejections__busy">Trying…</span>
                    : null}
                </li>
              ))}
            </ul>
            <Button
              onPress={composer.dismissRejections}
              size="small"
              variant="quiet"
            >
              Dismiss
            </Button>
          </div>
        ) : null}
        {/*
          A failed send renders no line here on purpose.

          The transcript already reports it, next to the question that failed,
          which is where it belongs -- an error floating above the composer is
          detached from the thing it is about. The machine still enters
          `failed`: that is what keeps the draft, and what lets Send re-enable
          so retrying is simply pressing it again. Rendering it twice was the
          same two-channel mistake the import path just had removed.
        */}
        <div className="composer__input-row">
          {/* Design specification 7.4: pressing the composer control invokes
              the native dialog immediately. No Forge popover, menu, workspace
              search step or route change may come first. Windows has no dialog
              mode that accepts files and folders together -- IFileOpenDialog
              either has FOS_PICKFOLDERS set or it does not -- so folder
              support stays a sibling control rather than a menu item. */}
          <div className="composer__attach">
            <Button
              aria-label="Add file"
              isDisabled={!canAttachFiles || !onAttachFiles || !composer.canOpenFileDialog}
              onPress={() => { pickAttachments(onAttachFiles); }}
              size="small"
              variant="quiet"
            >
              <OpenCodeIcon name="plus" />
              <span className="composer__attach-label">Add file</span>
            </Button>
            <Button
              aria-label="Add folder"
              isDisabled={!canAttachFiles || !onAttachFolder || !composer.canOpenFileDialog}
              onPress={() => { pickAttachments(onAttachFolder); }}
              size="small"
              variant="quiet"
            >
              <OpenCodeIcon name="folder" />
            </Button>
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
            onChange={(event) => { composer.setDraft(event.target.value); }}
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
          {/* 3a reads top to bottom: the prompt, then one line describing what
              will be sent with it -- lane, model, files, budget -- then send.
              This readout sat above the input, which put an accounting strip
              between the user and the thing they came here to type. */}
          <ContextWindow
            canSuggest={canAttachFiles && draft.trim().length > 0}
            onClear={onClearContext}
            onOpen={onOpenContext}
            onSuggest={() => {
              const intent = draft.trim();
              if (intent.length > 0) void onSuggestContext(intent);
            }}
            attachments={attachments}
            modelContext={modelContext}
            onRemove={onRemoveAttachment}
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
            {/* Quiet, not filled. 3a reserves its one accent for the
                consequential decision -- Apply changes -- and sending a
                message is not that. The glyph plus Enter is the affordance. */}
            <Button
              aria-label={methodGuidanceView ? "Send message" : "Send task"}
              className="composer__submit"
              isDisabled={!canSubmit}
              size="icon"
              title={methodGuidanceView ? "Send message" : "Send task"}
              type="submit"
              variant="quiet"
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
            : "Connect a model provider in Settings to send prompts."}
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
