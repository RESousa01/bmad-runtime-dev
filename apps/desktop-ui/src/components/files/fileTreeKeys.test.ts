import { describe, expect, it } from "vitest";

import type { WorkspaceTreeEntry } from "../../lib/hostClient";
import { treeKeyIntent } from "./fileTreeKeys";
import { buildVisibleTree, type DirectoryPage } from "./fileTreeModel";

function directory(relativePath: string): WorkspaceTreeEntry {
  return { childCursor: `cursor:${relativePath}`, kind: "directory", relativePath, sizeBytes: 0 };
}

function sealedDirectory(relativePath: string): WorkspaceTreeEntry {
  return { childCursor: null, kind: "directory", relativePath, sizeBytes: 0 };
}

function textFile(relativePath: string): WorkspaceTreeEntry {
  return { childCursor: null, kind: "text_file", relativePath, sizeBytes: 40 };
}

function rowsOf(
  childPages: ReadonlyMap<string, DirectoryPage>,
  expandedPaths: ReadonlySet<string>,
  rootEntries: readonly WorkspaceTreeEntry[],
) {
  return buildVisibleTree({
    childPages,
    expandedPaths,
    filter: "",
    loadingDirectories: new Set(),
    rootPage: { entries: rootEntries, nextCursor: null },
  }).rows;
}

/*
 * docs/            closed, openable
 * README.md
 * src/             open
 *   src/index.ts
 *   src/workspace/ closed, openable
 *   load-more:src  src/ has another page
 */
const rows = rowsOf(
  new Map([[
    "src",
    { entries: [textFile("src/index.ts"), directory("src/workspace")], nextCursor: "cursor:src:2" },
  ]]),
  new Set(["src"]),
  [directory("docs"), textFile("README.md"), directory("src")],
);

describe("treeKeyIntent", () => {
  it("has nothing to say about an empty tree, whichever key arrives", () => {
    // Asserted key by key rather than through one early return: an empty tree
    // is the sum of every branch finding nothing to act on, so each branch is
    // what has to be right.
    const keys = [
      "ArrowDown",
      "ArrowUp",
      "ArrowLeft",
      "ArrowRight",
      "Home",
      "End",
      "Enter",
      " ",
      "Tab",
    ];
    expect(keys.map((key) => treeKeyIntent(key, [], null))).toEqual(keys.map(() => null));
    expect(keys.map((key) => treeKeyIntent(key, [], "docs"))).toEqual(keys.map(() => null));
  });

  it("lays the fixture out as the tests below assume", () => {
    expect(rows.map((row) => row.key)).toEqual([
      "docs",
      "README.md",
      "src",
      "src/index.ts",
      "src/workspace",
      "load-more:src",
    ]);
  });

  describe("moving", () => {
    it("steps down one row at a time", () => {
      expect(treeKeyIntent("ArrowDown", rows, "docs"))
        .toEqual({ kind: "focus", path: "README.md" });
      expect(treeKeyIntent("ArrowDown", rows, "src"))
        .toEqual({ kind: "focus", path: "src/index.ts" });
    });

    it("steps up one row at a time", () => {
      expect(treeKeyIntent("ArrowUp", rows, "src/index.ts"))
        .toEqual({ kind: "focus", path: "src" });
    });

    it("does not wrap off either end", () => {
      // Wrapping would answer "keep going" by teleporting the user to the
      // other end of a tree they cannot see all of.
      expect(treeKeyIntent("ArrowDown", rows, "load-more:src")).toBeNull();
      expect(treeKeyIntent("ArrowUp", rows, "docs")).toBeNull();
    });

    it("jumps to the ends", () => {
      expect(treeKeyIntent("Home", rows, "src/workspace"))
        .toEqual({ kind: "focus", path: "docs" });
      expect(treeKeyIntent("End", rows, "docs"))
        .toEqual({ kind: "focus", path: "load-more:src" });
    });

    it("enters the tree from the top when nothing holds the tab stop yet", () => {
      expect(treeKeyIntent("ArrowDown", rows, null)).toEqual({ kind: "focus", path: "docs" });
      expect(treeKeyIntent("ArrowUp", rows, null)).toBeNull();
    });

    it("does nothing for a row that is no longer in the tree", () => {
      expect(treeKeyIntent("ArrowUp", rows, "gone.ts")).toBeNull();
      expect(treeKeyIntent("Enter", rows, "gone.ts")).toBeNull();
      expect(treeKeyIntent("ArrowLeft", rows, "gone.ts")).toBeNull();
    });
  });

  describe("Right", () => {
    it("opens a closed directory", () => {
      expect(treeKeyIntent("ArrowRight", rows, "docs"))
        .toEqual({ kind: "expand", path: "docs" });
    });

    it("steps into an already open directory", () => {
      expect(treeKeyIntent("ArrowRight", rows, "src"))
        .toEqual({ kind: "focus", path: "src/index.ts" });
    });

    it("waits rather than skipping past a directory whose page is still coming", () => {
      // rows[index + 1] is the next sibling until the children arrive, and
      // moving there would look like Right had jumped over the directory.
      const pending = rowsOf(new Map(), new Set(["docs"]), [directory("docs"), textFile("a.ts")]);
      expect(pending.map((row) => row.key)).toEqual(["docs", "a.ts"]);
      expect(treeKeyIntent("ArrowRight", pending, "docs")).toBeNull();
    });

    it("has nothing to open on a file, a more-row, or a directory with no cursor", () => {
      const sealed = rowsOf(new Map(), new Set(), [sealedDirectory("vendor")]);
      expect(treeKeyIntent("ArrowRight", rows, "README.md")).toBeNull();
      expect(treeKeyIntent("ArrowRight", rows, "load-more:src")).toBeNull();
      expect(treeKeyIntent("ArrowRight", sealed, "vendor")).toBeNull();
    });
  });

  describe("Left", () => {
    it("closes an open directory", () => {
      expect(treeKeyIntent("ArrowLeft", rows, "src"))
        .toEqual({ kind: "collapse", path: "src" });
    });

    it("steps out to the parent from a row that has nothing to close", () => {
      expect(treeKeyIntent("ArrowLeft", rows, "src/index.ts"))
        .toEqual({ kind: "focus", path: "src" });
      expect(treeKeyIntent("ArrowLeft", rows, "src/workspace"))
        .toEqual({ kind: "focus", path: "src" });
    });

    it("steps out of a more-row to the directory it belongs to", () => {
      expect(treeKeyIntent("ArrowLeft", rows, "load-more:src"))
        .toEqual({ kind: "focus", path: "src" });
    });

    it("stops at the root rather than leaving the tree", () => {
      expect(treeKeyIntent("ArrowLeft", rows, "README.md")).toBeNull();
      expect(treeKeyIntent("ArrowLeft", rows, "docs")).toBeNull();
    });
  });

  describe("activating", () => {
    it("treats Enter and Space alike, on any row", () => {
      expect(treeKeyIntent("Enter", rows, "README.md"))
        .toEqual({ kind: "activate", path: "README.md" });
      expect(treeKeyIntent(" ", rows, "README.md"))
        .toEqual({ kind: "activate", path: "README.md" });
      expect(treeKeyIntent("Enter", rows, "load-more:src"))
        .toEqual({ kind: "activate", path: "load-more:src" });
      expect(treeKeyIntent(" ", rows, "src"))
        .toEqual({ kind: "activate", path: "src" });
    });
  });

  describe("Tab", () => {
    it("steps into the focused row's own controls", () => {
      // The tree holds one tab stop, so a row's checkbox is out of the
      // sequential order; without this, context selection is mouse-only.
      expect(treeKeyIntent("Tab", rows, "README.md"))
        .toEqual({ kind: "enter_row", path: "README.md" });
    });

    it("leaves Shift+Tab and more-rows to the browser", () => {
      expect(treeKeyIntent("Tab", rows, "README.md", { shiftKey: true })).toBeNull();
      expect(treeKeyIntent("Tab", rows, "load-more:src")).toBeNull();
    });
  });

  describe("keys that are not the tree's", () => {
    it("gives up any chord to the shortcut that owns it", () => {
      // Ctrl+Home belongs to the scroll container and Alt+Left to history.
      expect(treeKeyIntent("Home", rows, "src", { ctrlKey: true })).toBeNull();
      expect(treeKeyIntent("ArrowLeft", rows, "src", { altKey: true })).toBeNull();
      expect(treeKeyIntent("ArrowDown", rows, "docs", { metaKey: true })).toBeNull();
      expect(treeKeyIntent("Enter", rows, "README.md", { ctrlKey: true })).toBeNull();
    });

    it("keeps Shift for the key itself", () => {
      expect(treeKeyIntent("ArrowDown", rows, "docs", { shiftKey: true }))
        .toEqual({ kind: "focus", path: "README.md" });
    });

    it("ignores keys it has no meaning for", () => {
      expect(treeKeyIntent("PageDown", rows, "docs")).toBeNull();
      expect(treeKeyIntent("a", rows, "docs")).toBeNull();
      expect(treeKeyIntent("Escape", rows, "docs")).toBeNull();
    });
  });
});
