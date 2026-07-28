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
 * What to tell the user after a pick, or null when there is nothing to say.
 *
 * Null is deliberate for the ordinary case: a notice that appears every time a
 * pick succeeds is one the user stops reading, and then the one that matters --
 * "some of your files were skipped" -- goes unread with it.
 */
export function attachmentNotice(
  pick: FilePickOutcome,
  merge: ContextMerge,
  limit: number = MAX_CONTEXT_FILES,
): string | null {
  const skipped = pick.rejectedOutsideRoot + pick.rejectedUnreadable;
  if (skipped > 0) {
    return `Added ${pick.selectedCount} files. ${skipped} unsupported files were skipped.`;
  }
  // The picker's own truncation and this module's cap both mean the same thing
  // to the user, so they read the same.
  if (pick.truncated || merge.cappedByLimit) {
    return `Added the first ${limit} supported files.`;
  }
  return null;
}

/**
 * What to say when a pick attached nothing.
 *
 * Cancelling the dialog is not a failure and says nothing. Selecting files that
 * were all rejected *is* worth saying, because from the user's side those two
 * outcomes look identical: they chose files and nothing appeared.
 */
export function emptyPickNotice(rejectedCount: number): string | null {
  return rejectedCount > 0
    ? `No files were attached. ${rejectedCount} were outside the workspace or unreadable.`
    : null;
}

export const maxContextFiles = MAX_CONTEXT_FILES;
