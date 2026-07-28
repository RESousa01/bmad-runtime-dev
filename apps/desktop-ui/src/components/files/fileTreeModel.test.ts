import { describe, expect, it } from "vitest";

import type { WorkspaceTreeEntry } from "../../lib/hostClient";
import {
  buildVisibleTree,
  describeFilter,
  emptyExpansion,
  entryName,
  expandedForGrant,
  formatBytes,
  resolveActivePath,
  withExpanded,
  type DirectoryPage,
  type VisibleTreeInput,
} from "./fileTreeModel";

function directory(relativePath: string): WorkspaceTreeEntry {
  return { childCursor: `cursor:${relativePath}`, kind: "directory", relativePath, sizeBytes: 0 };
}

/** A directory the host offered no child cursor for: listed, but not openable. */
function sealedDirectory(relativePath: string): WorkspaceTreeEntry {
  return { childCursor: null, kind: "directory", relativePath, sizeBytes: 0 };
}

function textFile(relativePath: string, sizeBytes = 40): WorkspaceTreeEntry {
  return { childCursor: null, kind: "text_file", relativePath, sizeBytes };
}

function page(
  entries: readonly WorkspaceTreeEntry[],
  nextCursor: string | null = null,
): DirectoryPage {
  return { entries, nextCursor };
}

const rootEntries = [directory("_bmad"), textFile("README.md", 42), directory("src")];

/** The two-page src/ tree every filter test below searches. */
const srcPages = new Map<string, DirectoryPage>([
  ["src", page([textFile("src/index.ts"), directory("src/workspace")])],
  ["src/workspace", page([textFile("src/workspace/scanner.ts")])],
]);

function tree(overrides: Partial<VisibleTreeInput> = {}) {
  return buildVisibleTree({
    childPages: new Map(),
    expandedPaths: new Set(),
    filter: "",
    loadingDirectories: new Set(),
    rootPage: page(rootEntries),
    ...overrides,
  });
}

describe("buildVisibleTree", () => {
  it("lays the root page out as one level, in the order the host gave", () => {
    const { rows } = tree();
    expect(rows.map((row) => row.key)).toEqual(["_bmad", "README.md", "src"]);
    expect(rows.map((row) => row.level)).toEqual([1, 1, 1]);
    expect(rows.map((row) => row.parentKey)).toEqual([null, null, null]);
    expect(rows.map((row) => row.posInSet)).toEqual([1, 2, 3]);
    expect(rows.map((row) => row.setSize)).toEqual([3, 3, 3]);
  });

  it("keeps a page it already holds out of the tree while its directory is closed", () => {
    // The page survives a collapse so re-opening costs no round trip, and a
    // model that rendered every page it held would undo the collapse.
    const { rows } = tree({ childPages: srcPages });
    expect(rows.map((row) => row.key)).toEqual(["_bmad", "README.md", "src"]);
  });

  it("nests an opened directory's page beneath it, numbered within that directory", () => {
    const { rows } = tree({ childPages: srcPages, expandedPaths: new Set(["src"]) });
    expect(rows.map((row) => row.key)).toEqual([
      "_bmad",
      "README.md",
      "src",
      "src/index.ts",
      "src/workspace",
    ]);
    expect(rows.map((row) => row.level)).toEqual([1, 1, 1, 2, 2]);
    expect(rows.map((row) => row.parentKey)).toEqual([null, null, null, "src", "src"]);
    expect(rows.map((row) => row.posInSet)).toEqual([1, 2, 3, 1, 2]);
    expect(rows.map((row) => row.setSize)).toEqual([3, 3, 3, 2, 2]);
  });

  it("opens the branch that was opened and no other", () => {
    const { rows } = tree({ childPages: srcPages, expandedPaths: new Set(["src"]) });
    expect(rows.map((row) => row.kind === "entry" && row.isExpanded))
      .toEqual([false, false, true, false, false]);
  });

  it("ends a part-loaded directory with one more-row instead of the rest of it", () => {
    // The host pages at a bounded size; what keeps a directory of thousands
    // from becoming thousands of rows is that the remainder arrives as a
    // single row the user asks for.
    const childPages = new Map([
      ["src", page([textFile("src/a.ts"), textFile("src/b.ts")], "cursor:src:2")],
    ]);
    const { rows } = tree({ childPages, expandedPaths: new Set(["src"]) });
    expect(rows.map((row) => row.key)).toEqual([
      "_bmad",
      "README.md",
      "src",
      "src/a.ts",
      "src/b.ts",
      "load-more:src",
    ]);
    expect(rows[5]).toEqual({
      cursor: "cursor:src:2",
      directory: "src",
      directoryName: "src",
      key: "load-more:src",
      kind: "load_more",
      level: 2,
      parentKey: "src",
      posInSet: 3,
      setSize: 3,
    });
    // The children count it too, or the last file would announce itself as
    // "2 of 2" with a third row sitting under it.
    expect(rows[3]?.setSize).toBe(3);
    expect(rows[4]?.setSize).toBe(3);
  });

  it("adds no more-row to a directory the host finished", () => {
    const childPages = new Map([["src", page([textFile("src/a.ts")])]]);
    const { rows } = tree({ childPages, expandedPaths: new Set(["src"]) });
    expect(rows.map((row) => row.kind)).toEqual(["entry", "entry", "entry", "entry"]);
  });

  it("leaves the root's own next page outside the tree", () => {
    // It extends the tree rather than sitting in it, and there is no parent
    // row for it to belong to.
    const { rows } = tree({ rootPage: page(rootEntries, "cursor:root:2") });
    expect(rows.map((row) => row.key)).toEqual(["_bmad", "README.md", "src"]);
  });

  it("offers no disclosure on a directory the host gave no cursor for", () => {
    const { rows } = tree({
      childPages: new Map([["vendor", page([textFile("vendor/a.js")])]]),
      expandedPaths: new Set(["vendor"]),
      rootPage: page([sealedDirectory("vendor")]),
    });
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({ isExpandable: false, isExpanded: false });
  });

  it("marks the directory whose page is in flight, and only that one", () => {
    const { rows } = tree({ loadingDirectories: new Set(["src"]) });
    expect(rows.map((row) => row.kind === "entry" && row.isLoading))
      .toEqual([false, false, true]);
  });

  it("offers only UTF-8 text files for selection", () => {
    const rootPage = page([
      textFile("a.ts"),
      directory("dir"),
      { childCursor: null, kind: "binary_file", relativePath: "logo.png", sizeBytes: 900 },
      { childCursor: null, kind: "blocked", relativePath: ".env", sizeBytes: 10 },
    ]);
    expect(tree({ rootPage }).rows.map((row) => row.kind === "entry" && row.isSelectable))
      .toEqual([true, false, false, false]);
  });

  it("counts every entry it holds, and calls nothing a match, while unfiltered", () => {
    const result = tree({ childPages: srcPages });
    expect(result.isFiltered).toBe(false);
    expect(result.matchCount).toBe(0);
    expect(result.loadedCount).toBe(6);
  });
});

describe("buildVisibleTree under a filter", () => {
  it("opens the way down to a match in a directory nobody opened", () => {
    const result = tree({ childPages: srcPages, filter: "scanner" });
    expect(result.isFiltered).toBe(true);
    expect(result.rows.map((row) => row.key))
      .toEqual(["src", "src/workspace", "src/workspace/scanner.ts"]);
    expect(result.rows.map((row) => row.level)).toEqual([1, 2, 3]);
  });

  it("separates the match from the rows that are only the way to it", () => {
    const { rows } = tree({ childPages: srcPages, filter: "scanner" });
    expect(rows.map((row) => row.kind === "entry" && row.matchesFilter))
      .toEqual([false, false, true]);
  });

  it("counts matches against every page it holds, opened or not", () => {
    const result = tree({ childPages: srcPages, filter: "scanner" });
    expect(result.matchCount).toBe(1);
    expect(result.loadedCount).toBe(6);
  });

  it("reads the entry's name, not the path above it", () => {
    // Filtering on whole paths would make "src" match every file beneath
    // src/ as well, and the count would describe a search nobody asked for.
    const result = tree({ childPages: srcPages, filter: "src" });
    expect(result.rows.map((row) => row.key)).toEqual(["src"]);
    expect(result.matchCount).toBe(1);
  });

  it("ignores case on both sides", () => {
    const result = tree({ childPages: srcPages, filter: "INDEX" });
    expect(result.rows.map((row) => row.key)).toEqual(["src", "src/index.ts"]);
    expect(result.matchCount).toBe(1);
  });

  it("withholds a page it has not searched", () => {
    // A more-row here would offer to fetch entries the match count does not
    // describe, which would make the count a claim about nothing in particular.
    const childPages = new Map([
      ["src", page([textFile("src/scanner.ts")], "cursor:src:2")],
    ]);
    const { rows } = tree({ childPages, expandedPaths: new Set(["src"]), filter: "scanner" });
    expect(rows.map((row) => row.key)).toEqual(["src", "src/scanner.ts"]);
    expect(rows[1]?.setSize).toBe(1);
  });

  it("hides everything when nothing matches, and still says it is filtering", () => {
    const result = tree({ childPages: srcPages, filter: "zzz" });
    expect(result.rows).toEqual([]);
    expect(result.isFiltered).toBe(true);
    expect(result.matchCount).toBe(0);
  });

  it("treats a filter of only spaces as no filter", () => {
    const result = tree({ childPages: srcPages, filter: "   " });
    expect(result.isFiltered).toBe(false);
    expect(result.rows.map((row) => row.key)).toEqual(["_bmad", "README.md", "src"]);
  });
});

describe("describeFilter", () => {
  const caveat = "Folders that have not been expanded are not searched.";

  it("says nothing when there is no filter", () => {
    expect(describeFilter("", 0, 12)).toBe("");
    expect(describeFilter("   ", 0, 12)).toBe("");
  });

  it("names the query, the hits and the set that was searched", () => {
    expect(describeFilter("scanner", 2, 40))
      .toBe(`2 of 40 loaded entries match “scanner”. ${caveat}`);
  });

  it("agrees with itself when the whole workspace is one entry and it matches", () => {
    expect(describeFilter("readme", 1, 1))
      .toBe(`1 of 1 loaded entry matches “readme”. ${caveat}`);
  });

  it("tells an empty result apart from an empty workspace", () => {
    // Both are a blank box; only one of them is fixed by clearing the filter.
    expect(describeFilter("zzz", 0, 40)).toBe(
      `No loaded entry matches “zzz”. 40 loaded entries are hidden by this filter. ${caveat}`,
    );
    expect(describeFilter("zzz", 0, 1)).toBe(
      `No loaded entry matches “zzz”. 1 loaded entry is hidden by this filter. ${caveat}`,
    );
  });

  it("quotes the query the tree actually applied", () => {
    expect(describeFilter("  scanner  ", 1, 3))
      .toBe(`1 of 3 loaded entries matches “scanner”. ${caveat}`);
  });
});

describe("resolveActivePath", () => {
  const openRows = tree({
    childPages: srcPages,
    expandedPaths: new Set(["src", "src/workspace"]),
  }).rows;

  it("has no tab stop while the tree has no rows", () => {
    expect(resolveActivePath([], null)).toBeNull();
    expect(resolveActivePath([], "src")).toBeNull();
  });

  it("starts the tab stop on the first row", () => {
    expect(resolveActivePath(openRows, null)).toBe("_bmad");
  });

  it("leaves the tab stop alone while its row survives", () => {
    expect(openRows.map((row) => row.key)).toEqual([
      "_bmad",
      "README.md",
      "src",
      "src/index.ts",
      "src/workspace",
      "src/workspace/scanner.ts",
    ]);
    expect(resolveActivePath(openRows, "src/workspace/scanner.ts"))
      .toBe("src/workspace/scanner.ts");
  });

  it("falls back to the deepest surviving ancestor, not the top of the tree", () => {
    // Collapsing src/workspace takes the focused row with it. Landing on the
    // first row would answer a press of Left by scrolling to the workspace root.
    const collapsed = tree({ childPages: srcPages, expandedPaths: new Set(["src"]) }).rows;
    expect(collapsed.map((row) => row.key)).toEqual([
      "_bmad",
      "README.md",
      "src",
      "src/index.ts",
      "src/workspace",
    ]);
    expect(resolveActivePath(collapsed, "src/workspace/scanner.ts")).toBe("src/workspace");
  });

  it("returns to the first row when nothing above the lost row survives", () => {
    expect(resolveActivePath(openRows, "vanished.ts")).toBe("_bmad");
  });

  it("does not mistake a name-prefix sibling for an ancestor", () => {
    const rows = tree({ rootPage: page([textFile("srcx.ts"), directory("src")]) }).rows;
    expect(resolveActivePath(rows, "src-legacy/a.ts")).toBe("srcx.ts");
  });
});

describe("expansion scoped to a grant", () => {
  it("starts with nothing expanded", () => {
    expect([...expandedForGrant(emptyExpansion("w1"), "w1")]).toEqual([]);
  });

  it("keeps what was expanded under the grant it was collected for", () => {
    const expansion = withExpanded(emptyExpansion("w1"), "w1", "src", true);
    expect([...expandedForGrant(expansion, "w1")]).toEqual(["src"]);
  });

  it("reads as nothing expanded once the grant changes", () => {
    // src/ in one workspace is not src/ in another. The paths resolve either
    // way, so a carried-over set is a wrong answer that looks like a right one.
    const expansion = withExpanded(emptyExpansion("w1"), "w1", "src", true);
    expect([...expandedForGrant(expansion, "w2")]).toEqual([]);
  });

  it("starts a toggle that arrives after a grant change from empty", () => {
    const before = withExpanded(emptyExpansion("w1"), "w1", "src", true);
    const after = withExpanded(before, "w2", "docs", true);
    expect([...expandedForGrant(after, "w2")]).toEqual(["docs"]);
    expect([...expandedForGrant(after, "w1")]).toEqual([]);
  });

  it("collapses one directory without disturbing the others", () => {
    const both = withExpanded(
      withExpanded(emptyExpansion("w1"), "w1", "src", true),
      "w1",
      "docs",
      true,
    );
    expect([...expandedForGrant(withExpanded(both, "w1", "src", false), "w1")])
      .toEqual(["docs"]);
  });

  it("leaves the expansion it was given untouched", () => {
    const before = withExpanded(emptyExpansion("w1"), "w1", "src", true);
    withExpanded(before, "w1", "docs", true);
    expect([...expandedForGrant(before, "w1")]).toEqual(["src"]);
  });
});

describe("entryName", () => {
  it("shows the last segment of a nested path", () => {
    expect(entryName("src/workspace/scanner.ts")).toBe("scanner.ts");
  });

  it("shows a root entry whole", () => {
    expect(entryName("README.md")).toBe("README.md");
  });
});

describe("formatBytes", () => {
  it("keeps anything under a kilobyte in bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("gives the small kilobytes a decimal and drops it from ten up", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(10 * 1024 - 1)).toBe("10.0 KB");
    expect(formatBytes(10 * 1024)).toBe("10 KB");
  });

  it("switches to megabytes at a megabyte", () => {
    expect(formatBytes(1024 * 1024 - 1)).toBe("1024 KB");
    expect(formatBytes(1024 * 1024)).toBe("1.0 MB");
  });
});
