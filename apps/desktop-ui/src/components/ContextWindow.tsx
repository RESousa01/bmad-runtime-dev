import { Sparkles, Trash2 } from "lucide-react";
import type { TaskAttachments } from "../features/attachments/attachmentTypes";
import { projectContextUsage, type ContextUsage } from "../features/model-context/contextUsage";
import {
  unmeasuredModelContext,
  type ModelContextState,
} from "../features/model-context/modelContextTypes";
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
  /**
   * What the host measured for this task's last assembled request.
   *
   * Optional, defaulting to `unmeasured`, because nothing issues a measurement
   * yet -- the host exposes no command that returns one. Making it required
   * would force every caller to invent a value to pass, and an invented value
   * is precisely how a workspace read cap came to be displayed as a model
   * window in the first place.
   */
  readonly modelContext?: ModelContextState | undefined;
}

/**
 * A token count, short enough for the composer's meta line.
 *
 * Thousands, not kibibytes. The divisor here used to be 1024 because the number
 * being formatted was a byte cap wearing a token's name. A context window is
 * quoted in decimal thousands by every provider that publishes one, and
 * rendering a 200,000-token window as "195.3K" would misstate it in the one
 * place this surface exists to be exact about.
 */
function compactTokens(value: number): string {
  if (value < 1000) return value.toLocaleString("en-US");
  const thousands = value / 1000;
  return `${Number.isInteger(thousands) ? thousands : thousands.toFixed(1)}K`;
}

/** The verdict, in the two words the meta line has room for. */
function basisLabel(usage: ContextUsage): string {
  if (usage.basis === "estimated") return "Estimated";
  return usage.stale ? "Last reported" : "Reported";
}

/**
 * The sentence behind the label.
 *
 * Both are needed. "Estimated" on its own does not say estimated *from what*,
 * and a user who reads it as "estimated by the model's tokenizer" ends up with
 * the same false confidence the old 64K gave them -- which is the whole defect,
 * not a detail of it. The label fits the row; the reason goes here, where a
 * screen reader and a hover can both reach it.
 */
function describeUsage(usage: ContextUsage): string {
  const noWindow = usage.window.kind === "known"
    ? ""
    : " The selected model's context window is not reported by the host, so there is no budget to measure this against.";
  if (usage.basis === "estimated") {
    return `Estimated from the byte size of the attached files, not counted by a tokenizer.${noWindow}`;
  }
  return usage.stale
    ? `Reported by the host for an earlier request. Something has changed since, so it no longer describes what would be sent now.${noWindow}`
    : `Reported by the host for the assembled request.${noWindow}`;
}

export function ContextWindow({
  canSuggest,
  onClear,
  onOpen,
  onSuggest,
  attachments,
  modelContext = unmeasuredModelContext,
}: ContextWindowProps) {
  const { descriptors, selectionMode, suggesting: suggestionBusy } = attachments;
  // Summed from the descriptors rather than carried alongside them: one number
  // that can disagree with the list it describes is one too many.
  const attachedTokens = descriptors.reduce((total, item) => total + item.estimatedTokens, 0);
  const usage = projectContextUsage(modelContext, attachedTokens);
  const extent = usage.window;
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
          aria-describedby="context-window-basis"
          aria-label="Open context details"
          className="context-window__summary"
          disabled={onOpen === undefined}
          onClick={onOpen}
          type="button"
        >
          <OpenCodeIcon name="filetree" size={14} />
          <span>Context</span>
          {/* The denominator reads "Unavailable" rather than being dropped.
              An omitted limit is read as no limit, and the user who has to be
              told the window is unknown is exactly the one who would otherwise
              assume there is room. */}
          <strong>
            {compactTokens(usage.usedTokens)}
            {" / "}
            {extent.kind === "known" ? compactTokens(extent.windowTokens) : "Unavailable"}
          </strong>
          <span className="context-window__basis">{basisLabel(usage)}</span>
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
      {/* No window, no meter. A bar drawn against an unknown denominator is a
          picture of a budget, and a picture is more persuasive than the caption
          under it -- it would undo the honesty of the readout above. */}
      {extent.kind === "known" ? (
        <div
          aria-label="Model context usage"
          aria-valuemax={extent.windowTokens}
          aria-valuemin={0}
          aria-valuenow={Math.min(usage.usedTokens, extent.windowTokens)}
          aria-valuetext={`${compactTokens(usage.usedTokens)} of ${compactTokens(extent.windowTokens)} tokens`}
          className="context-window__meter"
          role="progressbar"
        >
          <span style={{ width: `${Math.min(100, extent.fraction * 100)}%` }} />
        </div>
      ) : null}
      {items.length > 0 ? (
        <ul
          aria-label="Attached files and their estimated token counts"
          className="context-window__items context-window__items--compact"
        >
          {items.map((item) => (
            <li key={item.relativePath}>
              <code title={item.relativePath}>{item.relativePath}</code>
              {/* The tilde carries the same claim the row label does, and has
                  to carry it independently: once the host reports a figure the
                  row reads "Reported" while these are still bytes over four. */}
              <span title={`${item.estimatedTokens.toLocaleString("en-US")} estimated tokens`}>
                ~{compactTokens(item.estimatedTokens)}
              </span>
            </li>
          ))}
        </ul>
      ) : null}
      <p className="sr-only" id="context-window-basis">{describeUsage(usage)}</p>
      <p className="sr-only">Only files you add are included with your message.</p>
    </section>
  );
}
