/**
 * Which primary view the shell shows.
 *
 * FE-10 step 2. This used to be a three-deep ternary inside a 160-line JSX
 * return, which hid a real design decision behind punctuation: *no task
 * selected is Home*. There is no phantom "New task" record to fall back onto
 * any more, so an empty task list, a closed last tab and a deleted last task
 * all resolve to Home rather than to a tab that cannot be acted on.
 *
 * The rule is separated from the rendering so it can be stated once and tested
 * directly. The component below is deliberately thin -- it picks a thunk. Thunks
 * rather than elements because the task view cannot even be constructed without
 * a selected task, and building it eagerly to hand to a router that will not
 * render it is how a null dereference gets introduced by a refactor.
 */

import type { ReactNode } from "react";

/** The route the user asked for, independent of whether it can be served. */
export type PrimaryRoute =
  | { kind: "home" }
  | { kind: "task"; taskId: string | null };

/** The view actually rendered, after availability is taken into account. */
export type AppView = "home" | "task" | "no-workspace";

export interface AppRouteInput {
  readonly route: PrimaryRoute;
  /** Whether the routed task resolves to a real record. */
  readonly hasSelectedTask: boolean;
  /** Whether a workspace is bound. Without one a task cannot be worked on. */
  readonly hasWorkspace: boolean;
}

/**
 * Resolves the view.
 *
 * Order matters and is the whole rule:
 *
 *   1. Home is Home, and so is a task route with nothing selected.
 *   2. A task with no workspace behind it shows the empty state, because
 *      nothing in the task view is usable without one -- and showing the task
 *      chrome over an unbound workspace invites edits that cannot be applied.
 *   3. Otherwise, the task.
 */
export function resolveAppView({
  hasSelectedTask,
  hasWorkspace,
  route,
}: AppRouteInput): AppView {
  if (route.kind === "home" || !hasSelectedTask) return "home";
  return hasWorkspace ? "task" : "no-workspace";
}

export interface AppRouterProps extends AppRouteInput {
  readonly renderHome: () => ReactNode;
  readonly renderTask: () => ReactNode;
  readonly renderNoWorkspace: () => ReactNode;
}

export function AppRouter({
  renderHome,
  renderNoWorkspace,
  renderTask,
  ...input
}: AppRouterProps): ReactNode {
  switch (resolveAppView(input)) {
    case "home": return renderHome();
    case "task": return renderTask();
    case "no-workspace": return renderNoWorkspace();
  }
}
