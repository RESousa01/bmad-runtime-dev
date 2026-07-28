/**
 * What a key press means to the tree, decided without a DOM.
 *
 * FE-21. The Files panel was a list with indentation: every row was its own
 * tab stop, and the arrow keys did what the browser does to a page. A tree the
 * user cannot walk is not a tree, and the walk is arithmetic over the visible
 * rows -- which row is next, which is the parent, whether Left should collapse
 * or step out -- so it belongs here rather than in a keydown handler where the
 * only way to check it is to render one.
 *
 * The intents are deliberately not "call this function". The renderer owns
 * what expanding costs (a host round trip) and what activating means (a
 * preview read, a toggle, a page fetch); this module only says which of them
 * the key asked for.
 */

import type { FileTreeRow } from "./fileTreeModel";

export type TreeKeyIntent =
  /** Move the roving tab stop to this row. */
  | { readonly kind: "focus"; readonly path: string }
  | { readonly kind: "expand"; readonly path: string }
  | { readonly kind: "collapse"; readonly path: string }
  | { readonly kind: "activate"; readonly path: string }
  /**
   * Step into the row's own controls -- in practice its context checkbox.
   *
   * The tree holds one tab stop, so the checkbox on each row is out of the
   * sequential order; without this, selecting files for context would be a
   * mouse-only capability. Tab reaching the controls of the focused row is the
   * treegrid pattern, and it costs nothing on rows that have none: the
   * renderer only takes the key when there is something focusable to take it.
   */
  | { readonly kind: "enter_row"; readonly path: string };

export interface KeyModifiers {
  readonly altKey?: boolean;
  readonly ctrlKey?: boolean;
  readonly metaKey?: boolean;
  readonly shiftKey?: boolean;
}

/**
 * The intent for `key` pressed while `activePath` holds the tab stop, or null
 * when the tree has nothing to do and the browser should keep the key.
 *
 * Movement does not wrap: ArrowDown on the last row returns null so the key
 * falls through rather than silently teleporting the user to the top.
 */
export function treeKeyIntent(
  key: string,
  rows: readonly FileTreeRow[],
  activePath: string | null,
  modifiers: KeyModifiers = {},
): TreeKeyIntent | null {
  if (rows.length === 0) {
    return null;
  }
  // A shortcut is not a tree command. Ctrl+Home belongs to the scroll
  // container, and Alt+Left to history.
  if (modifiers.altKey === true || modifiers.ctrlKey === true || modifiers.metaKey === true) {
    return null;
  }
  const index = rows.findIndex((row) => row.key === activePath);
  const row = index === -1 ? undefined : rows[index];

  switch (key) {
    case "ArrowDown": {
      const next = rows[index + 1];
      return next === undefined ? null : { kind: "focus", path: next.key };
    }
    case "ArrowUp": {
      if (index <= 0) {
        return null;
      }
      const previous = rows[index - 1];
      return previous === undefined ? null : { kind: "focus", path: previous.key };
    }
    case "Home": {
      const first = rows[0];
      return first === undefined ? null : { kind: "focus", path: first.key };
    }
    case "End": {
      const last = rows[rows.length - 1];
      return last === undefined ? null : { kind: "focus", path: last.key };
    }
    case "ArrowRight": {
      if (row === undefined || row.kind !== "entry" || !row.isExpandable) {
        return null;
      }
      if (!row.isExpanded) {
        return { kind: "expand", path: row.key };
      }
      // Already open: step to the first child, if one has arrived. A page
      // still in flight has no row to move to, so the key does nothing rather
      // than jumping past the directory to its next sibling.
      const child = rows[index + 1];
      return child !== undefined && child.parentKey === row.key
        ? { kind: "focus", path: child.key }
        : null;
    }
    case "ArrowLeft": {
      if (row === undefined) {
        return null;
      }
      if (row.kind === "entry" && row.isExpandable && row.isExpanded) {
        return { kind: "collapse", path: row.key };
      }
      return row.parentKey === null ? null : { kind: "focus", path: row.parentKey };
    }
    case "Enter":
    case " ":
      return row === undefined ? null : { kind: "activate", path: row.key };
    case "Tab":
      if (modifiers.shiftKey === true || row === undefined || row.kind !== "entry") {
        return null;
      }
      return { kind: "enter_row", path: row.key };
    default:
      return null;
  }
}
