/**
 * What the Files inspector shows, decided without a DOM.
 *
 * FE-21. The explorer used to decide visibility while it rendered: a recursive
 * component read the expansion set, the child-page map and the loading set at
 * every level and emitted rows as it went. Nothing about that is wrong on its
 * own, but it meant the two questions this file answers -- which rows exist,
 * and what each row's position in the tree is -- could only be asked through
 * jsdom, and jsdom is the worst place to ask them. `aria-level` and
 * `aria-posinset` in particular are arithmetic over a structure, and the
 * structure was never a value.
 *
 * So the rows are a value now. Everything here is a pure function over the
 * state the explorer already holds; `FileTree` renders what it returns and
 * decides nothing.
 */

import type { WorkspaceTreeEntry } from "../../lib/hostClient";

/** One host page of a directory, as the explorer holds it. */
export interface DirectoryPage {
  readonly entries: readonly WorkspaceTreeEntry[];
  readonly nextCursor: string | null;
}

/**
 * The expanded set, tagged with the grant it was collected under.
 *
 * The tag is the point. A workspace switch or a re-grant gives a different
 * tree, and `src/` in one is not `src/` in another -- applying the old set to
 * the new entries would show one workspace's shape over another's files, and
 * would do it silently, because the paths still resolve. Carrying the grant
 * with the set means a mismatch reads as "nothing is expanded" rather than as
 * a plausible wrong answer, whatever order the reset effect and the first
 * render happen to run in.
 */
export interface TreeExpansion {
  readonly grantKey: string;
  readonly expandedPaths: ReadonlySet<string>;
}

const noPaths: ReadonlySet<string> = new Set<string>();

export function emptyExpansion(grantKey: string): TreeExpansion {
  return { expandedPaths: noPaths, grantKey };
}

/** The expanded paths that belong to `grantKey`, and nothing else. */
export function expandedForGrant(
  expansion: TreeExpansion,
  grantKey: string,
): ReadonlySet<string> {
  return expansion.grantKey === grantKey ? expansion.expandedPaths : noPaths;
}

/**
 * Records one directory as expanded or collapsed under `grantKey`.
 *
 * Built from `expandedForGrant`, not from the raw set, so a toggle arriving
 * after a grant change starts from empty instead of adopting the previous
 * workspace's expansion wholesale.
 */
export function withExpanded(
  expansion: TreeExpansion,
  grantKey: string,
  relativePath: string,
  expanded: boolean,
): TreeExpansion {
  const current = expandedForGrant(expansion, grantKey);
  const next = new Set(current);
  if (expanded) {
    next.add(relativePath);
  } else {
    next.delete(relativePath);
  }
  return { expandedPaths: next, grantKey };
}

interface RowBase {
  /** Row identity: the roving-focus key and the React key. */
  readonly key: string;
  /** `aria-level`, 1-based. */
  readonly level: number;
  /** The `key` of the directory row this row sits under, or null at the root. */
  readonly parentKey: string | null;
  /** `aria-posinset`, 1-based. */
  readonly posInSet: number;
  /** `aria-setsize`. */
  readonly setSize: number;
}

export interface EntryRow extends RowBase {
  readonly kind: "entry";
  readonly entry: WorkspaceTreeEntry;
  /** A directory the host offered a child cursor for. Only these get `aria-expanded`. */
  readonly isExpandable: boolean;
  readonly isExpanded: boolean;
  /** A child page for this directory is in flight. */
  readonly isLoading: boolean;
  /** False on a row kept only because a descendant matched the filter. */
  readonly matchesFilter: boolean;
  /** Only a UTF-8 text file can be read or sent for context review. */
  readonly isSelectable: boolean;
}

/**
 * The next page of a directory, as a row rather than a control beside the tree.
 *
 * A button outside the tree is a second tab stop and is unreachable by the
 * arrow keys that got the user to the bottom of the directory in the first
 * place. As a row it is simply the last thing under that directory.
 */
export interface LoadMoreRow extends RowBase {
  readonly kind: "load_more";
  readonly cursor: string;
  readonly directory: string;
  readonly directoryName: string;
}

export type FileTreeRow = EntryRow | LoadMoreRow;

export interface VisibleTreeInput {
  readonly childPages: ReadonlyMap<string, DirectoryPage>;
  readonly expandedPaths: ReadonlySet<string>;
  readonly filter: string;
  readonly loadingDirectories: ReadonlySet<string>;
  readonly rootPage: DirectoryPage | null;
}

export interface VisibleTree {
  readonly rows: readonly FileTreeRow[];
  /** Entries the renderer has actually received, across every loaded page. */
  readonly loadedCount: number;
  /** Loaded entries whose own name matches the filter. Zero when unfiltered. */
  readonly matchCount: number;
  readonly isFiltered: boolean;
}

/** The last segment of a relative path, which is what the tree displays. */
export function entryName(relativePath: string): string {
  return relativePath.split("/").at(-1) ?? relativePath;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function normalize(value: string): string {
  return value.trim().toLocaleLowerCase("en-US");
}

function childrenOf(
  childPages: ReadonlyMap<string, DirectoryPage>,
  relativePath: string,
): readonly WorkspaceTreeEntry[] {
  return childPages.get(relativePath)?.entries ?? [];
}

/**
 * Walks every page the renderer holds, not just the visible ones.
 *
 * A directory that was expanded, loaded and collapsed again still has its page
 * in memory, and a filter that ignored it would report "no match" for a file
 * the user has already seen. Counting the same way means the "N of M loaded"
 * line describes the set that was actually searched.
 */
function survey(
  entries: readonly WorkspaceTreeEntry[],
  childPages: ReadonlyMap<string, DirectoryPage>,
  needle: string,
  kept: Set<string>,
  matched: Set<string>,
): { anyKept: boolean; loadedCount: number } {
  let anyKept = false;
  let loadedCount = 0;
  for (const entry of entries) {
    loadedCount += 1;
    const selfMatch = needle.length > 0
      && normalize(entryName(entry.relativePath)).includes(needle);
    if (selfMatch) {
      matched.add(entry.relativePath);
    }
    let descendantKept = false;
    if (entry.kind === "directory") {
      const below = survey(
        childrenOf(childPages, entry.relativePath),
        childPages,
        needle,
        kept,
        matched,
      );
      descendantKept = below.anyKept;
      loadedCount += below.loadedCount;
    }
    if (selfMatch || descendantKept) {
      kept.add(entry.relativePath);
      anyKept = true;
    }
  }
  return { anyKept, loadedCount };
}

/**
 * The rows to render, flat and in visual order.
 *
 * Flat because the tree is lazy: a nested list forces the level of every row
 * to be re-derived from where it sits in the DOM, and rows arrive one host
 * page at a time. `aria-level` and `aria-posinset` carry the shape instead,
 * which is what they exist for.
 */
export function buildVisibleTree(input: VisibleTreeInput): VisibleTree {
  const { childPages, expandedPaths, filter, loadingDirectories, rootPage } = input;
  const needle = normalize(filter);
  const isFiltered = needle.length > 0;
  const kept = new Set<string>();
  const matched = new Set<string>();
  const { loadedCount } = survey(
    rootPage?.entries ?? [],
    childPages,
    needle,
    kept,
    matched,
  );

  const rows: FileTreeRow[] = [];

  const append = (
    directory: string | null,
    entries: readonly WorkspaceTreeEntry[],
    nextCursor: string | null,
    level: number,
  ): void => {
    const visible = isFiltered
      ? entries.filter((entry) => kept.has(entry.relativePath))
      : entries;
    // The filtered view is a claim about matches; offering to fetch a page
    // whose contents have not been searched would not be one.
    const showLoadMore = directory !== null && nextCursor !== null && !isFiltered;
    const setSize = visible.length + (showLoadMore ? 1 : 0);
    visible.forEach((entry, index) => {
      const path = entry.relativePath;
      const page = childPages.get(path);
      const isExpandable = entry.kind === "directory" && entry.childCursor !== null;
      // Under a filter the view expands whatever it must to reach a match, so
      // a hit three directories down is visible rather than merely counted.
      const isExpanded = isExpandable && (isFiltered
        ? (page?.entries ?? []).some((child) => kept.has(child.relativePath))
        : expandedPaths.has(path));
      rows.push({
        entry,
        isExpandable,
        isExpanded,
        isLoading: loadingDirectories.has(path),
        isSelectable: entry.kind === "text_file",
        key: path,
        kind: "entry",
        level,
        matchesFilter: !isFiltered || matched.has(path),
        parentKey: directory,
        posInSet: index + 1,
        setSize,
      });
      if (isExpanded && page) {
        append(path, page.entries, page.nextCursor, level + 1);
      }
    });
    if (showLoadMore && nextCursor !== null && directory !== null) {
      rows.push({
        cursor: nextCursor,
        directory,
        directoryName: entryName(directory),
        key: `load-more:${directory}`,
        kind: "load_more",
        level,
        parentKey: directory,
        posInSet: setSize,
        setSize,
      });
    }
  };

  // The root's own next page stays outside the tree: it extends the tree
  // rather than sitting inside it, and it has no parent row to sit under.
  append(null, rootPage?.entries ?? [], null, 1);

  return { isFiltered, loadedCount, matchCount: matched.size, rows };
}

/**
 * What the filter is showing, in words.
 *
 * A filtered tree with no hits and an unopened workspace look identical --
 * both are an empty box -- and the difference between "there is nothing here"
 * and "you are looking through a filter" is the whole of the user's next move.
 * The caveat is not padding: only loaded pages can be searched, and a filter
 * that quietly reports on a fraction of the workspace is worse than one that
 * says which fraction.
 */
export function describeFilter(
  query: string,
  matchCount: number,
  loadedCount: number,
): string {
  const trimmed = query.trim();
  if (trimmed.length === 0) {
    return "";
  }
  const loadedNoun = loadedCount === 1 ? "entry" : "entries";
  const caveat = "Folders that have not been expanded are not searched.";
  if (matchCount === 0) {
    return `No loaded entry matches “${trimmed}”. `
      + `${loadedCount} loaded ${loadedNoun} ${loadedCount === 1 ? "is" : "are"} hidden by this filter. `
      + caveat;
  }
  return `${matchCount} of ${loadedCount} loaded ${loadedNoun} `
    + `${matchCount === 1 ? "matches" : "match"} “${trimmed}”. ${caveat}`;
}

/**
 * Which row holds the tree's single tab stop.
 *
 * Collapsing a directory takes the focused row out of the tree with it. Falling
 * back to the first row would throw the user to the top of the workspace for
 * pressing Left; the nearest surviving ancestor is where they were standing.
 */
export function resolveActivePath(
  rows: readonly FileTreeRow[],
  activePath: string | null,
): string | null {
  const first = rows[0];
  if (first === undefined) {
    return null;
  }
  if (activePath === null) {
    return first.key;
  }
  let ancestor: string | null = null;
  for (const row of rows) {
    if (row.key === activePath) {
      return activePath;
    }
    if (
      activePath.startsWith(`${row.key}/`)
      && (ancestor === null || row.key.length > ancestor.length)
    ) {
      ancestor = row.key;
    }
  }
  return ancestor ?? first.key;
}
