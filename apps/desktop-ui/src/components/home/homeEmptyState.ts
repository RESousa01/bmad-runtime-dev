/**
 * Why the Home index has nothing to show.
 *
 * Home is the surface a user lands on to resume work, so an index with no rows
 * is the one screen that has to explain itself. It used to answer every
 * emptiness with the same sentence -- "No tasks yet. Describe what you want
 * above" -- including the case where no folder had been granted at all, where
 * the composer above it is disabled and there is nothing to describe anything
 * to. One sentence covering three causes is not a smaller interface; it is a
 * wrong one, and it points the user at a control that cannot accept them.
 *
 * The three causes nest, and that nesting is the whole rule: a host that cannot
 * answer is why there is no project, and no project is why there is no task.
 * The outermost unmet condition is the honest one to name, so this walks from
 * the outside in and stops at the first thing that is actually missing.
 *
 * Separate from the components because it is the part worth proving. Whether a
 * rail draws a hairline is a matter of taste; whether Home tells a user to open
 * a folder while the host it would ask is unreachable is not.
 */

/// The host states Home distinguishes between.
///
/// Declared here rather than imported from the shell's `RuntimeKind`, the same
/// way `NoWorkspaceState` declares `NoWorkspaceMode`: a presentation component
/// naming the states it has copy for is a smaller contract than one that moves
/// whenever the runtime union does.
export type HomeHostKind =
  | "browser_demo"
  | "loading"
  | "read_only_recovery"
  | "ready"
  | "unavailable";

export type HomeEmptyReason = "host-unready" | "no-project" | "no-tasks";

export interface HomeEmptyState {
  readonly reason: HomeEmptyReason;
  readonly message: string;
}

export interface HomeEmptyInput {
  readonly hostKind: HomeHostKind;
  /// Workspaces the host has granted, which is what the projects rail lists.
  readonly projectCount: number;
  /// Open tasks, which is what the index lists. Closed tasks are deliberately
  /// not counted: they have their own section, and a list of things the user
  /// has finished with does not make the index non-empty.
  readonly taskCount: number;
}

/// What each host state says when it is the reason the index is bare.
///
/// `ready` is absent by type rather than by a placeholder string, because a
/// ready host never explains an emptiness -- something further in does.
const hostUnreadyMessages: Record<Exclude<HomeHostKind, "ready">, string> = {
  browser_demo:
    "Browser preview runs without the Windows desktop host, so there is no local project to open.",
  loading:
    "Forge is still verifying the Windows desktop host. Projects appear here as soon as it answers.",
  read_only_recovery:
    "Local authority is in read-only recovery. Existing projects stay listed, but no new one can be opened until it is repaired.",
  unavailable:
    "The Windows desktop host is unavailable, so no project can be opened. Use Retry host once it is running.",
};

const noProjectMessage =
  "No project yet. Forge works only inside a folder you grant it, so open one to start.";

const noTasksMessage =
  "No tasks yet. Describe what you want above, or start an empty one with New task.";

/**
 * The sentence the index shows in place of its rows, or `null` when it has rows.
 *
 * Returning `null` rather than an "everything is fine" state keeps the caller
 * from having to decide what an empty message means: there is exactly one value
 * that renders nothing, and it is the absence of a reason.
 */
export function homeEmptyState({
  hostKind,
  projectCount,
  taskCount,
}: HomeEmptyInput): HomeEmptyState | null {
  if (taskCount > 0) {
    return null;
  }
  if (
    hostKind !== "ready"
    // Browser preview is the one host that is not ready and can still carry a
    // task, so it explains the emptiness only while it has no project either.
    // Blaming the missing desktop host over a demo project the user can work in
    // would be true and useless.
    && (hostKind !== "browser_demo" || projectCount === 0)
  ) {
    return { message: hostUnreadyMessages[hostKind], reason: "host-unready" };
  }
  if (projectCount === 0) {
    return { message: noProjectMessage, reason: "no-project" };
  }
  return { message: noTasksMessage, reason: "no-tasks" };
}
