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
import type {
  PickRejectionReason,
  RejectedPickProjection,
} from "../../lib/hostClient";

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
  /// Every refusal the host reported, named where naming was safe.
  readonly rejected?: readonly RejectedPickProjection[] | undefined;
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
 * How far "per file" reaches is decided by the host, not here. A file rejected
 * as outside the workspace has no workspace-relative path -- naming it would
 * mean shipping an absolute path from outside the grant into the renderer,
 * which is the thing that boundary exists to prevent -- so the broker sends
 * `relativePath: null` for those and a name for the ones it can. FE-15 added
 * that field; before it, every refusal here was an anonymous count.
 *
 * The anonymous ones are still grouped and counted, because "2 files were
 * outside this workspace" is all that can truthfully be said about them, and
 * two identical unnamed lines would read as a rendering fault.
 */

/** What each refusal reason means, in the user's terms. */
const rejectionReasons: Record<PickRejectionReason, string> = {
  blocked: "Not readable here: it is a link, a placeholder, or a protected file.",
  not_text: "Not a readable text file.",
  outside_workspace: "Outside this workspace.",
  unreadable: "Could not be read.",
};

/** The grouped sentence for refusals the host could not name. */
function anonymousReason(reason: PickRejectionReason, count: number): string {
  const files = count === 1 ? "file" : "files";
  switch (reason) {
    case "outside_workspace":
      return `${count} ${files} ${count === 1 ? "was" : "were"} outside this workspace.`;
    case "blocked":
      return `${count} ${files} ${count === 1 ? "was" : "were"} refused: a link, a placeholder, or a protected file.`;
    case "not_text":
      return `${count} ${files} ${count === 1 ? "is" : "are"} not readable text.`;
    case "unreadable":
      return `${count} ${files} could not be read.`;
  }
}

export function pickRejections(
  pick: FilePickOutcome,
  merge?: ContextMerge,
  limit: number = MAX_CONTEXT_FILES,
): readonly AttachmentRejection[] {
  const rejections: AttachmentRejection[] = [];
  const refusals = pick.rejected;
  if (refusals === undefined) {
    /*
     * A host that predates the per-file field. Only the counts are available,
     * and the grouping they were tallied under puts everything except an I/O
     * failure in `rejectedOutsideRoot` -- so a binary file inside the
     * workspace lands under "outside this workspace". That mislabel is why the
     * field exists; it is reproduced rather than guessed at, because guessing
     * would put a confident wrong cause on the screen instead of a vague one.
     */
    if (pick.rejectedOutsideRoot > 0) {
      rejections.push({
        reason: anonymousReason("outside_workspace", pick.rejectedOutsideRoot),
        relativePath: "",
      });
    }
    if (pick.rejectedUnreadable > 0) {
      rejections.push({
        reason: anonymousReason("unreadable", pick.rejectedUnreadable),
        relativePath: "",
      });
    }
  } else {
    // Named first, because they are the ones the user can act on.
    for (const refusal of refusals) {
      if (refusal.relativePath === null) continue;
      rejections.push({
        reason: rejectionReasons[refusal.reason],
        relativePath: refusal.relativePath,
      });
    }
    const unnamed = new Map<PickRejectionReason, number>();
    for (const refusal of refusals) {
      if (refusal.relativePath !== null) continue;
      unnamed.set(refusal.reason, (unnamed.get(refusal.reason) ?? 0) + 1);
    }
    for (const [reason, count] of unnamed) {
      rejections.push({ reason: anonymousReason(reason, count), relativePath: "" });
    }
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
