/**
 * What a task has attached, and what an attachment is allowed to be.
 *
 * FE-13. Attachments used to live in one app-global slot, so switching tasks
 * did not *move* them -- it destroyed them. `selectSession` called
 * `clearContext()` on every switch precisely because the projection was global
 * and would otherwise have been sent with the next task's message: files the
 * user never attached there. Clearing was the only safe thing to do with state
 * that belonged to nobody.
 *
 * Making attachments task-owned removes the choice. A task keeps what it has
 * attached while the user works somewhere else, and nothing another task
 * attaches can reach it.
 *
 * A descriptor grants nothing
 * ---------------------------
 * `relativePath` is a label the host resolved and the user recognises. It is
 * not a capability: the renderer cannot open it, and re-sending one does not
 * re-grant access -- the host re-resolves against the workspace grant on every
 * request. `contentHash` is what actually identifies the bytes, which is why a
 * descriptor whose hash has moved is stale rather than merely renamed.
 *
 * Attachments are not the Context panel's source of truth
 * ------------------------------------------------------
 * FE-13 states that outright, and the two really are different questions.
 * Attachments are what the user chose to send; model context is what the model
 * was actually given, which includes the conversation and the persona and is
 * measured after the host has assembled it. They live in separate modules for
 * that reason -- see `features/model-context/modelContextTypes.ts`.
 */

import type { ContextItemProjection, ContextPreviewProjection } from "../../lib/hostClient";

/** How the current attachment set was arrived at. */
export type AttachmentSelectionMode = "manual" | "automatic";

/**
 * One attached file, as the renderer is allowed to know it.
 *
 * A projection of `ContextItemProjection` minus `content`: the renderer shows a
 * list, and holding the bytes of every attached file for every open task is
 * memory spent to display a filename.
 */
export interface AttachmentDescriptor {
  readonly relativePath: string;
  /** Identifies the bytes. A descriptor whose hash has moved is stale. */
  readonly contentHash: string;
  readonly byteCount: number;
  readonly estimatedTokens: number;
  readonly startLine: number;
  readonly endLine: number;
}

/** Everything a task has attached, and how it got there. */
export interface TaskAttachments {
  readonly descriptors: readonly AttachmentDescriptor[];
  readonly selectionMode: AttachmentSelectionMode;
  /**
   * The manifest hash the host issued for this exact set.
   *
   * Carried so a send can name the set the user was looking at. Null once the
   * set is edited locally, because the host has not confirmed the new one.
   */
  readonly manifestHash: string | null;
  /** What to tell the user about the last attach, or null. */
  readonly notice: string | null;
  /** True while an auto-selection is running for *this* task. */
  readonly suggesting: boolean;
}

export const emptyTaskAttachments: TaskAttachments = {
  descriptors: [],
  manifestHash: null,
  notice: null,
  selectionMode: "manual",
  suggesting: false,
};

export function describeAttachment(item: ContextItemProjection): AttachmentDescriptor {
  return {
    byteCount: item.byteCount,
    contentHash: item.contentHash,
    endLine: item.endLine,
    estimatedTokens: item.estimatedTokens,
    relativePath: item.relativePath,
    startLine: item.startLine,
  };
}

/** Adopts a host-confirmed selection as this task's attachments. */
export function adoptPreview(
  projection: ContextPreviewProjection,
  selectionMode: AttachmentSelectionMode,
  notice: string | null = null,
): TaskAttachments {
  return {
    descriptors: projection.items.map(describeAttachment),
    manifestHash: projection.manifestHash,
    notice,
    selectionMode,
    suggesting: false,
  };
}

/**
 * Clears a task's attachments.
 *
 * Used after a send: the attachments are spent by the message that carried
 * them. Leaving them armed meant the next message silently re-sent, re-billed
 * and re-entered every one of them into the reviewed-context manifest --
 * consent given once, acted on every turn.
 */
export function clearAttachments(notice: string | null = null): TaskAttachments {
  return { ...emptyTaskAttachments, notice };
}

/** The paths a send should carry, in the order they were attached. */
export function attachmentPaths(attachments: TaskAttachments): readonly string[] {
  return attachments.descriptors.map(({ relativePath }) => relativePath);
}
