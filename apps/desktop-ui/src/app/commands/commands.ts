/**
 * The command catalog.
 *
 * Before this existed the palette built its own array, the Ctrl+K shortcut was
 * a separate document listener, and task actions were *omitted* from the list
 * when no task was open. Omission is the failure FE-09 names: a command that
 * disappears tells the user nothing, and they cannot tell "this app has no such
 * feature" from "this needs a task open first".
 *
 * So availability lives here, with a reason attached, and the palette renders
 * unavailable commands rather than dropping them.
 */

import {
  available,
  CommandRegistry,
  unavailable,
  type CommandAvailability,
  type CommandDefinition,
} from "./commandRegistry";

/**
 * The state availability depends on.
 *
 * Deliberately a snapshot of primitives rather than the app's own objects: an
 * availability rule that can reach into a task can also mutate one, and these
 * rules are meant to be pure enough to test without a rendered app.
 */
export interface CommandContext {
  /** Title of the open task, or null when none is. */
  readonly selectedTaskTitle: string | null;
  readonly selectedTaskId: string | null;
  /** False in the browser preview, where there is no host to open a folder with. */
  readonly workspaceAvailable: boolean;
  readonly actions: CommandActions;
}

export interface CommandActions {
  readonly startNewSession: () => void;
  readonly selectSession: (taskId: string) => void;
  readonly closeTask: (taskId: string) => void;
  readonly requestTaskDeletion: (taskId: string) => void;
  readonly openWorkspaceManager: () => void;
  readonly openContextDrawer: (kind: "files" | "changes" | "activity" | "skills") => void;
  readonly openSettings: () => void;
  readonly togglePalette: () => void;
}

/**
 * Availability rules, named so they read as the sentence the user gets.
 *
 * Each returns the reason in the second person and says what to *do*, not what
 * went wrong. "Open a task first" is actionable; "no task selected" describes
 * the app's state and leaves the user to work out the remedy.
 */
function requiresTask(context: CommandContext): CommandAvailability {
  return context.selectedTaskId === null
    ? unavailable("Open a task first.")
    : available;
}

function requiresWorkspace(context: CommandContext): CommandAvailability {
  return context.workspaceAvailable
    ? available
    : unavailable("Not available in the browser preview — run the desktop app.");
}

/**
 * Builds the catalog for a context.
 *
 * Titles that name the open task are computed here rather than in the palette,
 * so the menu and the palette show the same words for the same command.
 */
export function commandDefinitions(
  context: CommandContext,
): readonly CommandDefinition<CommandContext>[] {
  const taskTitle = context.selectedTaskTitle;
  // Suffix, not a separate command: "Close task" and "Close task: Fix login"
  // must be one id, or the palette and the menu can disagree about which one
  // the keybinding runs.
  const forTask = (verb: string) => (taskTitle === null ? verb : `${verb}: ${taskTitle}`);

  return [
    {
      id: "task.new",
      title: "New task",
      category: "Task",
      keywords: ["chat", "start", "session"],
      keybinding: "Mod+N",
      // Unconditional on purpose. Starting a task needs no model -- only
      // sending a message does, and that gate lives on the composer's send
      // button. Gating task creation here would have quietly taken away
      // something the app allowed before this registry existed.
      availability: () => available,
      run: (ctx) => ctx.actions.startNewSession(),
    },
    {
      id: "task.focus",
      title: forTask("Go to task"),
      category: "Task",
      keywords: ["switch", "open", "tab"],
      availability: requiresTask,
      run: (ctx) => {
        if (ctx.selectedTaskId !== null) ctx.actions.selectSession(ctx.selectedTaskId);
      },
    },
    {
      id: "task.close",
      title: forTask("Close task"),
      category: "Task",
      keywords: ["dismiss"],
      keybinding: "Mod+W",
      availability: requiresTask,
      run: (ctx) => {
        if (ctx.selectedTaskId !== null) ctx.actions.closeTask(ctx.selectedTaskId);
      },
    },
    {
      id: "task.delete",
      title: forTask("Delete task"),
      category: "Task",
      keywords: ["remove", "discard"],
      availability: requiresTask,
      run: (ctx) => {
        if (ctx.selectedTaskId !== null) ctx.actions.requestTaskDeletion(ctx.selectedTaskId);
      },
    },
    {
      id: "workspace.open",
      title: "Open or switch workspace",
      category: "Workspace",
      keywords: ["folder", "project", "directory"],
      keybinding: "Mod+O",
      availability: requiresWorkspace,
      run: (ctx) => ctx.actions.openWorkspaceManager(),
    },
    {
      id: "view.files",
      title: "Show workspace files",
      category: "View",
      keywords: ["drawer", "explorer", "tree"],
      availability: () => available,
      run: (ctx) => ctx.actions.openContextDrawer("files"),
    },
    {
      id: "view.changes",
      title: "Show review",
      category: "View",
      keywords: ["drawer", "diff", "changes", "git"],
      availability: () => available,
      run: (ctx) => ctx.actions.openContextDrawer("changes"),
    },
    {
      id: "view.activity",
      title: "Show run activity",
      category: "View",
      keywords: ["drawer", "log", "runs"],
      availability: () => available,
      run: (ctx) => ctx.actions.openContextDrawer("activity"),
    },
    {
      id: "view.skills",
      title: "Show skills and agents",
      category: "View",
      keywords: ["drawer", "bmad", "agents", "workflows"],
      availability: () => available,
      run: (ctx) => ctx.actions.openContextDrawer("skills"),
    },
    {
      id: "app.settings",
      title: "Open settings",
      category: "Application",
      keywords: ["preferences", "options", "config"],
      keybinding: "Mod+,",
      availability: () => available,
      run: (ctx) => ctx.actions.openSettings(),
    },
    {
      id: "app.palette",
      title: "Show all commands",
      category: "Application",
      keywords: ["palette", "search"],
      keybinding: "Mod+K",
      availability: () => available,
      run: (ctx) => ctx.actions.togglePalette(),
    },
  ];
}

export function createCommandRegistry(context: CommandContext): CommandRegistry<CommandContext> {
  return new CommandRegistry(commandDefinitions(context));
}

/**
 * The one matching rule the palette uses.
 *
 * Substring, not fuzzy. Fuzzy matching puts unrelated entries above the exact
 * one often enough that typing the full name of a command and pressing Enter
 * stops being reliable, and that is the interaction the palette exists for.
 *
 * Exported as a function over plain strings so the palette component can share
 * it without importing the catalog: two implementations of "does this match"
 * is how a command becomes unreachable by the word it is filed under.
 */
export function matchesTerms(terms: readonly string[], query: string): boolean {
  const needle = query.trim().toLocaleLowerCase("en-US");
  if (needle.length === 0) return true;
  return terms.some((text) => text.toLocaleLowerCase("en-US").includes(needle));
}
