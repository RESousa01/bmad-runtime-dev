/**
 * Erasing every trace of local data, and what the renderer must forget with it.
 *
 * FE-10 step 3. The host's erase destroys the store. The renderer's copy of
 * what was in it does not go anywhere on its own, and leaving it up means the
 * user is looking at the tasks, transcripts, drafts and diffs they just asked
 * to have destroyed — the bytes are gone from disk and the words are still in
 * the window. For a feature whose entire purpose is "remove my data from this
 * machine", that is the failure that matters.
 *
 * So the discard list is data, not a sequence of calls buried in a `try`. Each
 * entry says what the host destroyed and therefore what the renderer must stop
 * showing, and `localDataDiscards` is the checklist a reviewer can compare
 * against the host's erase.
 *
 * The list was wrong once. An earlier version discarded only the
 * workspace-bound set and a comment claimed that was "the same set". It was
 * not: the host's erase also signs the model out and revokes *every* workspace
 * grant, so Home still listed folders as granted, the title bar still named an
 * active one, and Settings still reported a connected provider — all revoked
 * seconds earlier. The workbench looked operational and every action failed.
 */

/** One thing the host destroyed, and the renderer's mirror of it. */
export interface LocalDataDiscard {
  readonly what: string;
  readonly why: string;
}

/**
 * Everything the renderer must forget when local data is erased.
 *
 * Kept as a list so it can be asserted against. A discard added to the host
 * without a matching entry here leaves something on screen that no longer
 * exists anywhere.
 */
export const localDataDiscards: readonly LocalDataDiscard[] = [
  {
    what: "workspace-bound state",
    why: "Transcripts, changes flows, capability runs and the method library all belong to a workspace binding the host has revoked.",
  },
  {
    what: "tasks",
    why: "Tasks are not workspace-bound, so the workspace reset does not reach them. Without this the tab strip lists conversations whose transcripts no longer exist anywhere.",
  },
  {
    what: "workspace grants",
    why: "The erase revokes every grant. Leaving them listed shows folders as granted that are not.",
  },
  {
    what: "model authentication",
    why: "The erase signs the model out. Settings would otherwise report a connected provider and model that were revoked seconds earlier.",
  },
  {
    what: "task drafts",
    why: "Unsent text belonging to tasks that no longer exist.",
  },
  {
    what: "attached context",
    why: "File selections pointing into a workspace the renderer can no longer read.",
  },
  {
    what: "the open drawer and the task route",
    why: "With no tasks left, Home is the empty state. Staying on a task route renders a workbench for a conversation that does not exist.",
  },
];

export interface EraseLocalDataPorts {
  /** Null unless the host is ready and offers the erase command. */
  readonly erase: ((confirmation: string) => Promise<unknown>) | null;
  readonly setStatus: (status: "erasing" | "erased" | "ready") => void;
  readonly setNotice: (notice: string | null) => void;
  readonly safeMessage: (error: unknown) => string;
  /** Applies every entry in `localDataDiscards`. */
  readonly discardEverything: () => void;
}

export const eraseUnavailableNotice = "Erasing local data is unavailable in this mode.";

/**
 * Runs the erase, then discards the renderer's mirror of what it destroyed.
 *
 * The discard runs only on success. An erase that failed destroyed nothing, and
 * clearing the screen would tell the user their data is gone when it is not —
 * the one lie this feature must never tell.
 */
export async function eraseLocalData(
  confirmation: string,
  ports: EraseLocalDataPorts,
): Promise<void> {
  if (ports.erase === null) {
    ports.setNotice(eraseUnavailableNotice);
    return;
  }
  ports.setStatus("erasing");
  ports.setNotice(null);
  try {
    await ports.erase(confirmation);
    // Status first: `erased` is terminal and stops the retention manifest being
    // re-read, which would replace the confirmation with "unavailable".
    ports.setStatus("erased");
    ports.discardEverything();
  } catch (error) {
    // Back to `ready`, not `unavailable`: the host answered, it just refused.
    ports.setStatus("ready");
    ports.setNotice(ports.safeMessage(error));
  }
}
