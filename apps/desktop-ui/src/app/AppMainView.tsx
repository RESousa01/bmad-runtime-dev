/**
 * The three primary views, and the rule that picks between them.
 *
 * FE-10 shell composition. `AppRouter` owns the *decision* — no task selected
 * is Home, a task with no workspace behind it is the empty state — and this
 * owns what each of those renders. Splitting them that way keeps the rule
 * testable without a rendered tree, which is why `resolveAppView` has its own
 * tests and this file has none beyond the integration ones.
 *
 * Each view has exactly one consumer, so its props travel as a typed
 * pass-through rather than a re-typed slice: re-listing twenty-five fields
 * would be an isomorphic copy that can only drift, and `ComponentProps` cannot.
 * That is the same reasoning as Settings in `AppProviders`.
 */

import { HomeView } from "../components/HomeView";
import { NoWorkspaceState } from "../components/redesign/NoWorkspaceState";
import { TaskWorkspace } from "../components/TaskWorkspace";
import { AppRouter, type PrimaryRoute } from "./AppRouter";
import { useShell } from "./AppProviders";

export function AppMainView({ route }: { readonly route: PrimaryRoute }) {
  const { views, workspace } = useShell();
  return (
    <AppRouter
      hasSelectedTask={views.task !== null}
      hasWorkspace={workspace.active !== null}
      renderHome={() => <HomeView {...views.home} />}
      renderNoWorkspace={() => (
        // Wrapped in `main` so the empty state is the document's main region:
        // it is what the user is here to act on, not a notice beside
        // something else.
        <main className="task-shell-empty">
          <NoWorkspaceState {...views.noWorkspace} />
        </main>
      )}
      renderTask={() => {
        // The router only reaches here with a task; narrowing again keeps that
        // a type-level fact rather than an assertion.
        if (views.task === null) return null;
        return <TaskWorkspace {...views.task.props} key={views.task.key} />;
      }}
      route={route}
    />
  );
}
