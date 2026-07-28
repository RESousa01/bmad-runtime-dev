/**
 * Building a context selection from a file pick.
 *
 * FE-10 step 3. Two pure decisions that lived inline in `browseAndAttach` and
 * had no tests: how picked paths merge with what is already attached, and what
 * the user is told about the ones that did not make it.
 *
 * Both matter more than they look. The merge decides which files are sent to a
 * model, and the notice is the only place the user learns that some were not.
 * A silent drop is the failure to avoid: files the user selected, believed were
 * attached, and were not.
 */

import type { AttachmentRejection } from "../../features/composer/composerEvents";

/**
 * The most files one message will carry.
 *
 * A cap rather than an error, because a folder pick can select thousands and
 * refusing outright leaves the user with nothing. The notice says the cap was
 * hit, so the truncation is never silent.
 */
const MAX_CONTEXT_FILES = 100;

export interface FilePickOutcome {
  readonly relativePaths: readonly string[];
  readonly selectedCount: number;
  readonly rejectedOutsideRoot: number;
  readonly rejectedUnreadable: number;
  readonly truncated: boolean;
}

export interface ContextMerge {
  readonly relativePaths: readonly string[];
  /** True when the cap dropped paths that would otherwise have been attached. */
  readonly cappedByLimit: boolean;
}

/**
 * Merges newly picked paths into what is already attached.
 *
 * De-duplicated case-insensitively, because Windows paths differ only in case
 * for the same file and attaching one twice spends the model's context on a
 * duplicate. The later spelling wins: it is the one the picker just returned,
 * so it is what the host will resolve.
 */
export function mergeContextPaths(
  existingPaths: readonly string[],
  pickedPaths: readonly string[],
  limit: number = MAX_CONTEXT_FILES,
): ContextMerge {
  const combined = new Map<string, string>();
  for (const relativePath of [...existingPaths, ...pickedPaths]) {
    combined.set(relativePath.toLocaleLowerCase("en-US"), relativePath);
  }
  const relativePaths = [...combined.values()].slice(0, limit);
  return { cappedByLimit: combined.size > relativePaths.length, relativePaths };
}

/**
 * A pick's refusals, as separate reasons rather than one merged sentence.
 *
 * Replaces the two notice builders that used to live here. Both collapsed
 * every refusal into a single string -- "3 unsupported files were skipped" --
 * which is the failure FE-14 names: it tells the user something went wrong and
 * nothing about which of their choices to revisit.
 *
 * An empty result is the ordinary case and is deliberate. A notice that appears
 * every time a pick succeeds is one the user stops reading, and then the one
 * that matters goes unread with it.
 *
 * FE-14 asks for partial failure to be "visible per file". How far that can go
 * is decided by the host, not here: `WorkspaceBroker` documents that only
 * relative wire paths and rejection *counts* leave the broker, and a file
 * rejected as outside the workspace has no workspace-relative path to report --
 * naming it would mean shipping an absolute path from outside the grant into
 * the renderer, which is the exact thing that boundary exists to prevent.
 *
 * So this is as far as the current contract reaches: the two rejection classes
 * are separated, each with its own reason, because "outside the workspace" and
 * "could not be read" are different problems with different fixes. Carrying
 * per-file paths for the *inside-root* refusals -- a blocked or binary file,
 * which does have a relative path -- needs a change to `PickedFilesProjection`
 * and belongs with FE-15, which owns the attachment surface.
 *
 * `relativePath` is empty for both: neither is attributable to a named file
 * yet, and inventing one would be worse than admitting it.
 */
export function pickRejections(
  pick: FilePickOutcome,
  merge?: ContextMerge,
  limit: number = MAX_CONTEXT_FILES,
): readonly AttachmentRejection[] {
  const rejections: AttachmentRejection[] = [];
  if (pick.rejectedOutsideRoot > 0) {
    rejections.push({
      reason: `${pick.rejectedOutsideRoot} ${pick.rejectedOutsideRoot === 1 ? "file was" : "files were"} outside this workspace.`,
      relativePath: "",
    });
  }
  if (pick.rejectedUnreadable > 0) {
    rejections.push({
      reason: `${pick.rejectedUnreadable} ${pick.rejectedUnreadable === 1 ? "file" : "files"} could not be read.`,
      relativePath: "",
    });
  }
  // The picker's own truncation and this module's cap mean the same thing to
  // the user, and both are refusals: files they chose that are not attached.
  if (pick.truncated || merge?.cappedByLimit === true) {
    rejections.push({
      reason: `Only the first ${limit} files were attached.`,
      relativePath: "",
    });
  }
  return rejections;
}

export const maxContextFiles = MAX_CONTEXT_FILES;
