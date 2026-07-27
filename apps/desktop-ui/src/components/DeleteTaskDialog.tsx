import { Button } from "@sapphirus/ui";
import { AlertTriangle } from "lucide-react";
import { useEffect, useRef } from "react";

export interface DeleteTaskDialogProps {
  /// The task's current name, shown so the user can see which one they are
  /// about to destroy rather than trusting that the right tab was focused.
  readonly taskTitle: string;
  /// How many messages the transcript holds. Zero reads differently from
  /// forty, and the user deserves to know which one they are discarding.
  ///
  /// Only meaningful once `historyStatus` is `"loaded"` — see below.
  readonly messageCount: number;
  /// Whether the renderer has actually read this task's transcript back from
  /// the host yet.
  ///
  /// `messageCount` is `lane.messages.length`, which is `0` for a genuinely
  /// empty task *and* for one whose history has never been requested, is still
  /// in flight, or failed to load. Saying "no messages yet" on any of those
  /// last three tells the user there is nothing to lose at the exact moment
  /// that is most likely to be false — and the transcript behind this dialog
  /// may simultaneously be saying the messages could not be loaded.
  ///
  /// Deleting is irreversible and task ids are never reused, so a transcript
  /// discarded on that sentence is orphaned permanently.
  readonly historyStatus: "unread" | "loading" | "loaded" | "unavailable";
  /// Whether a durable encrypted copy could exist for this conversation.
  ///
  /// False for a browser preview, which has no host and no store, and for a
  /// task proven to hold no messages. Warning about a copy that was never
  /// written is the same class of untruth as denying one that was.
  readonly mayHaveDurableCopy: boolean;
  readonly onCancel: () => void;
  readonly onConfirm: () => void;
}

/**
 * Confirms destroying a task.
 *
 * Deleting is the one irreversible thing the task list can do, so it is the one
 * action that gets a stop. Closing is offered here as the non-destructive
 * alternative, because "I want this out of my way" is the far more common
 * intent and the two are easy to confuse.
 *
 * The copy is deliberately precise about the limit of what this can promise.
 * Forge's durable chat store (`chat_sessions` / `chat_messages`) exposes no
 * per-session delete through the host command catalog, so this removes the
 * conversation from Forge and permanently orphans the encrypted local record —
 * it does not scrub it from disk. Claiming otherwise would be a lie the user
 * could not check. Erasing the on-disk copy is Settings → Local data.
 */
export function DeleteTaskDialog({
  taskTitle,
  messageCount,
  historyStatus,
  mayHaveDurableCopy,
  onCancel,
  onConfirm,
}: DeleteTaskDialogProps) {
  const dialogRef = useRef<HTMLElement>(null);

  useEffect(() => {
    // Focus lands on Cancel, the dialog's safe edge. The destructive button is
    // deliberately not autofocused: a stray Enter must not destroy a
    // transcript.
    dialogRef.current
      ?.querySelector<HTMLElement>(".delete-task-dialog__cancel")
      ?.focus();
  }, []);

  return (
    <section
      aria-labelledby="delete-task-title"
      aria-modal="true"
      className="delete-task-dialog"
      data-component="delete-task-confirm"
      ref={dialogRef}
      role="dialog"
    >
      <header className="delete-task-dialog__header">
        <span aria-hidden="true" className="delete-task-dialog__icon">
          <AlertTriangle size={18} />
        </span>
        <div>
          <h2 id="delete-task-title">Delete this task?</h2>
          <p className="delete-task-dialog__subject">{taskTitle}</p>
        </div>
      </header>

      <div className="delete-task-dialog__body">
        <p>
          {historyStatus !== "loaded"
            ? "Forge has not finished reading this task's messages, so it cannot say how many there are. Deleting removes the task from Forge and cannot be undone."
            : messageCount === 0
              ? "This task has no messages yet. Deleting it removes the task from Forge."
              : `Deleting removes this task and its ${messageCount} ${messageCount === 1 ? "message" : "messages"} from Forge. This cannot be undone.`}
        </p>
        {mayHaveDurableCopy ? (
          <p className="delete-task-dialog__note">
            Forge cannot erase the encrypted copy this device already wrote for
            this conversation; deleting permanently detaches it, and no task can
            reach it again. To erase local data from this device, use Settings →
            Local data.
          </p>
        ) : null}
        <p className="delete-task-dialog__alternative">
          Want to keep the transcript? Close the task instead — it leaves your
          open tabs and can be reopened from Home.
        </p>
      </div>

      <footer className="delete-task-dialog__actions">
        <Button
          className="delete-task-dialog__cancel"
          onPress={onCancel}
          size="small"
          variant="secondary"
        >
          Cancel
        </Button>
        <Button
          className="delete-task-dialog__confirm"
          onPress={onConfirm}
          size="small"
          variant="danger"
        >
          Delete task
        </Button>
      </footer>
    </section>
  );
}
