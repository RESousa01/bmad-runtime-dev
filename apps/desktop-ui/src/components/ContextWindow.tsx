import { Sparkles, Trash2 } from "lucide-react";
import type { TaskAttachments } from "../features/attachments/attachmentTypes";
import { workspaceReadLimits } from "../lib/hostClient";
import { OpenCodeIcon } from "./OpenCodeIcon";
import "./context-window.css";

/// Re-exported from the attachment feature, which owns the concept now.
export type { AttachmentSelectionMode as ContextSelectionMode } from "../features/attachments/attachmentTypes";

export interface ContextWindowProps {
  readonly canSuggest: boolean;
  readonly onClear: () => void;
  readonly onOpen?: (() => void) | undefined;
  readonly onSuggest: () => void;
  /**
   * What *this task* has attached.
   *
   * FE-13: the descriptor list, not the host projection. This panel renders a
   * count and a token total, and the projection carries the full content of
   * every attached file -- holding that per task, for every open task, is
   * memory spent to display a filename.
   */
  readonly attachments: TaskAttachments;
}

const localTokenBudget = Math.floor(workspaceReadLimits.contextBytes / 4);

function compactCount(value: number): string {
  if (value < 1024) return value.toLocaleString("en-US");
  const units = value / 1024;
  return `${Number.isInteger(units) ? units : units.toFixed(1)}K`;
}

export function ContextWindow({
  canSuggest,
  onClear,
  onOpen,
  onSuggest,
  attachments,
}: ContextWindowProps) {
  const { descriptors, selectionMode, suggesting: suggestionBusy } = attachments;
  // Summed from the descriptors rather than carried alongside them: one number
  // that can disagree with the list it describes is one too many.
  const usedTokens = descriptors.reduce((total, item) => total + item.estimatedTokens, 0);
  const fill = Math.min(100, (usedTokens / localTokenBudget) * 100);
  const items = descriptors;

  return (
    <section
      aria-labelledby="context-window-title"
      className="context-window context-window--opencode-v2"
      data-layout="compact"
    >
      <h2 className="sr-only" id="context-window-title">Context window</h2>
      <div className="context-window__compact-row">
        <button
          aria-label="Open context details"
          className="context-window__summary"
          disabled={onOpen === undefined}
          onClick={onOpen}
          type="button"
        >
          <OpenCodeIcon name="filetree" size={14} />
          <span>Context</span>
          <strong>{compactCount(usedTokens)} / {compactCount(localTokenBudget)}</strong>
          <span className="context-window__file-count">
            {items.length === 0 ? "No files" : `${items.length} ${items.length === 1 ? "file" : "files"}`}
          </span>
        </button>
        <span className={`context-window__mode context-window__mode--${selectionMode}`}>
          {selectionMode === "automatic" ? "Automatic" : "Pinned"}
        </span>
        <div className="context-window__actions">
          <button
            aria-label="Auto-select context"
            className="context-window__action"
            disabled={!canSuggest || suggestionBusy}
            onClick={onSuggest}
            title={canSuggest ? "Suggest files from the current task locally" : "Describe the task first"}
            type="button"
          >
            <Sparkles aria-hidden="true" size={13} />
            {suggestionBusy ? "Selecting…" : "Auto"}
          </button>
          {items.length > 0 ? (
            <button
              aria-label="Clear context"
              className="context-window__action context-window__action--icon"
              onClick={onClear}
              title="Clear local context"
              type="button"
            >
              <Trash2 aria-hidden="true" size={13} />
            </button>
          ) : null}
        </div>
      </div>
      <div
        aria-label="Local context budget"
        aria-valuemax={localTokenBudget}
        aria-valuemin={0}
        aria-valuenow={Math.min(usedTokens, localTokenBudget)}
        aria-valuetext={`${compactCount(usedTokens)} of ${compactCount(localTokenBudget)} estimated tokens`}
        className="context-window__meter"
        role="progressbar"
      >
        <span style={{ width: `${fill}%` }} />
      </div>
      {items.length > 0 ? (
        <ul aria-label="Selected local context" className="context-window__items context-window__items--compact">
          {items.map((item) => (
            <li key={item.relativePath}>
              <code title={item.relativePath}>{item.relativePath}</code>
              <span>{compactCount(item.estimatedTokens)}</span>
            </li>
          ))}
        </ul>
      ) : null}
      <p className="sr-only">Only files you add are included with your message.</p>
    </section>
  );
}
