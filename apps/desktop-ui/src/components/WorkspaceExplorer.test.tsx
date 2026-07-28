import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi, type Mock } from "vitest";

import type { WorkspaceEntriesProjection, WorkspaceTreeEntry } from "../lib/hostClient";
import type { ReadonlyWorkspaceSource } from "../lib/workspaceReadSource";
import { WorkspaceExplorer } from "./WorkspaceExplorer";

function directory(relativePath: string): WorkspaceTreeEntry {
  return { childCursor: `cursor:${relativePath}`, kind: "directory", relativePath, sizeBytes: 0 };
}

function textFile(relativePath: string): WorkspaceTreeEntry {
  return { childCursor: null, kind: "text_file", relativePath, sizeBytes: 64 };
}

type ListEntries = ReadonlyWorkspaceSource["listEntries"];

/**
 * The pages a fake host will answer with, keyed by the cursor that asks for
 * them. A promise stands for a page still in flight.
 */
type PageTable = Readonly<
  Record<string, WorkspaceEntriesProjection | Promise<WorkspaceEntriesProjection>>
>;

/**
 * A host that answers from a cursor-keyed table of pages.
 *
 * `listEntries` is a spy because a lazy tree is a claim about the requests that
 * never happen, and only the call log can show one of those.
 */
function fakeSource(
  pages: PageTable,
): ReadonlyWorkspaceSource & { readonly listEntries: Mock<ListEntries> } {
  const listEntries = vi.fn<ListEntries>(async (cursor) => {
    const projection = pages[cursor ?? "root"];
    if (projection === undefined) {
      throw new Error(`No fixture page for ${cursor ?? "root"}`);
    }
    return projection;
  });
  return {
    listEntries,
    previewContext: async () => {
      throw new Error("Not used by these tests.");
    },
    provenance: "local_host",
    readText: async (relativePath) => ({
      byteCount: 12,
      content: `contents of ${relativePath}`,
      contentHash: `sha256:${"a".repeat(64)}`,
      relativePath,
      truncated: false,
    }),
    scanBmad: async () => ({ assets: [], status: "not_detected", truncated: false }),
    search: async () => [],
    workspaceId: "workspace_alpha",
  };
}

function page(
  entries: readonly WorkspaceTreeEntry[],
  nextCursor: string | null = null,
): WorkspaceEntriesProjection {
  return { entries: [...entries], nextCursor, workspaceId: "workspace_alpha" };
}

const twoLevelPages: PageTable = {
  root: page([directory("src"), textFile("README.md")]),
  "cursor:src": page([textFile("src/index.ts"), directory("src/workspace")]),
  "cursor:src/workspace": page([textFile("src/workspace/scanner.ts")]),
};

function renderExplorer(source: ReadonlyWorkspaceSource) {
  return render(
    <WorkspaceExplorer
      availabilityMessage="Unavailable"
      onContextReview={() => undefined}
      source={source}
      workspaceName="Alpha"
    />,
  );
}

function treeItems(): HTMLElement[] {
  return screen.queryAllByRole("treeitem");
}

function itemNamed(name: string | RegExp): HTMLElement {
  return screen.getByRole("treeitem", { name });
}

describe("WorkspaceExplorer files tab", () => {
  it("asks the host for a directory's page only when the directory is opened", async () => {
    const user = userEvent.setup();
    const source = fakeSource(twoLevelPages);
    renderExplorer(source);

    await screen.findByRole("treeitem", { name: /README\.md/ });
    expect(source.listEntries.mock.calls).toEqual([[null, 100]]);

    await user.click(screen.getByRole("button", { name: /^src$/ }));

    await screen.findByRole("treeitem", { name: /index\.ts/ });
    expect(source.listEntries.mock.calls).toEqual([[null, 100], ["cursor:src", 100]]);
    // The grandchild page is still untouched: opening src/ opened one level.
    expect(screen.queryByRole("treeitem", { name: /scanner\.ts/ })).toBeNull();
  });

  it("says the page is in flight while it is in flight", async () => {
    const user = userEvent.setup();
    let releaseChildren!: (projection: WorkspaceEntriesProjection) => void;
    const children = new Promise<WorkspaceEntriesProjection>((resolve) => {
      releaseChildren = resolve;
    });
    renderExplorer(fakeSource({
      root: page([directory("src")]),
      "cursor:src": children,
    }));

    await user.click(await screen.findByRole("button", { name: /^src$/ }));

    const row = itemNamed(/^src/);
    expect(row.getAttribute("aria-busy")).toBe("true");
    expect(within(row).getByText("Loading…")).toBeTruthy();

    releaseChildren(page([textFile("src/index.ts")]));
    await screen.findByRole("treeitem", { name: /index\.ts/ });
    expect(itemNamed(/^src/).getAttribute("aria-busy")).toBeNull();
  });

  it("renders one more-row instead of the rest of a large directory", async () => {
    const user = userEvent.setup();
    const many = Array.from({ length: 100 }, (_, index) => textFile(`src/file-${index}.ts`));
    const source = fakeSource({
      root: page([directory("src")]),
      "cursor:src": page(many, "cursor:src:2"),
      "cursor:src:2": page([textFile("src/file-100.ts"), textFile("src/file-101.ts")]),
    });
    renderExplorer(source);

    await user.click(await screen.findByRole("button", { name: /^src$/ }));
    await screen.findByRole("treeitem", { name: /file-0\.ts/ });

    // src/ plus its hundred loaded children plus the one row that stands in
    // for everything the host has not sent.
    expect(treeItems()).toHaveLength(102);
    expect(source.listEntries).toHaveBeenCalledTimes(2);
    const more = screen.getByRole("treeitem", { name: /Load more in src/ });
    expect(more.getAttribute("aria-level")).toBe("2");

    await user.click(within(more).getByRole("button"));
    await screen.findByRole("treeitem", { name: /file-100\.ts/ });
    // The two entries the last page held, and no more-row now the host is done.
    expect(treeItems()).toHaveLength(103);
    expect(screen.queryByRole("treeitem", { name: /Load more in src/ })).toBeNull();
  });

  it("extends the root a page at a time rather than listing the whole workspace", async () => {
    const user = userEvent.setup();
    const source = fakeSource({
      root: page([textFile("a.ts"), textFile("b.ts")], "cursor:root:2"),
      "cursor:root:2": page([textFile("c.ts")]),
    });
    renderExplorer(source);

    await screen.findByRole("treeitem", { name: /a\.ts/ });
    expect(treeItems()).toHaveLength(2);

    await user.click(screen.getByRole("button", { name: "Load more files" }));

    await screen.findByRole("treeitem", { name: /c\.ts/ });
    expect(treeItems()).toHaveLength(3);
    expect(source.listEntries.mock.calls).toEqual([[null, 100], ["cursor:root:2", 100]]);
    expect(screen.queryByRole("button", { name: "Load more files" })).toBeNull();
  });

  it("presents the rows as one named tree", async () => {
    renderExplorer(fakeSource(twoLevelPages));
    const tree = await screen.findByRole("tree", { name: "Workspace files" });
    expect(within(tree).getAllByRole("treeitem")).toHaveLength(2);
  });

  it("announces each row's place in the tree", async () => {
    const user = userEvent.setup();
    renderExplorer(fakeSource(twoLevelPages));

    await user.click(await screen.findByRole("button", { name: /^src$/ }));
    await screen.findByRole("treeitem", { name: /index\.ts/ });

    const shape = treeItems().map((row) => [
      row.getAttribute("aria-level"),
      row.getAttribute("aria-posinset"),
      row.getAttribute("aria-setsize"),
      row.getAttribute("aria-expanded"),
    ]);
    expect(shape).toEqual([
      ["1", "1", "2", "true"],
      ["2", "1", "2", null],
      ["2", "2", "2", "false"],
      ["1", "2", "2", null],
    ]);
  });
});

describe("WorkspaceExplorer keyboard", () => {
  async function openedTree() {
    const user = userEvent.setup();
    renderExplorer(fakeSource(twoLevelPages));
    await screen.findByRole("treeitem", { name: /README\.md/ });
    return user;
  }

  it("gives the whole tree one tab stop", async () => {
    await openedTree();
    const rows = treeItems();
    expect(rows).toHaveLength(2);
    expect(rows.map((row) => row.getAttribute("tabindex"))).toEqual(["0", "-1"]);
    // Every control inside a row is out of the sequential order, or Tab would
    // walk the workspace one button at a time. src/ contributes its disclosure
    // and README.md its checkbox and its own button.
    const inner = rows.flatMap((row) =>
      [...row.querySelectorAll("button, input")].map((node) => node.getAttribute("tabindex")));
    expect(inner).toEqual(["-1", "-1", "-1"]);
  });

  it("walks rows with the arrow keys and moves the tab stop with the caret", async () => {
    const user = await openedTree();
    const [first, second] = treeItems();
    first?.focus();

    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(second);
    expect(second?.getAttribute("tabindex")).toBe("0");
    expect(first?.getAttribute("tabindex")).toBe("-1");

    await user.keyboard("{ArrowUp}");
    expect(document.activeElement).toBe(first);
  });

  it("steps into the focused row's controls with Tab and leaves their keys alone", async () => {
    // The tree holds one tab stop, so the checkbox is out of the sequential
    // order and Tab has to reach it. Once there the keys are the checkbox's:
    // a tree that still answered ArrowDown would move its tab stop and pull
    // focus out of the control the user is standing in, mid-selection.
    const user = await openedTree();
    itemNamed(/README/).focus();

    await user.keyboard("{Tab}");
    const checkbox = screen.getByRole<HTMLInputElement>("checkbox", {
      name: "Include README.md in context",
    });
    expect(document.activeElement).toBe(checkbox);

    await user.keyboard(" ");
    expect(checkbox.checked).toBe(true);
    expect(screen.queryByLabelText("Read-only preview of README.md")).toBeNull();

    // Up, because README.md is the last row and Down would have nowhere to go
    // whether the tree answered the key or not.
    await user.keyboard("{ArrowUp}");
    expect(document.activeElement).toBe(checkbox);
  });

  it("opens with Right, steps in, then closes and steps out with Left", async () => {
    const user = await openedTree();
    itemNamed(/^src/).focus();

    await user.keyboard("{ArrowRight}");
    await screen.findByRole("treeitem", { name: /index\.ts/ });
    expect(itemNamed(/^src/).getAttribute("aria-expanded")).toBe("true");

    await user.keyboard("{ArrowRight}");
    expect(document.activeElement).toBe(itemNamed(/index\.ts/));

    // Left from a row with nothing to close steps out to the parent, and only
    // then does a second Left close it.
    await user.keyboard("{ArrowLeft}");
    expect(document.activeElement).toBe(itemNamed(/^src/));
    await user.keyboard("{ArrowLeft}");
    await waitFor(() =>
      expect(itemNamed(/^src/).getAttribute("aria-expanded")).toBe("false"));
    expect(screen.queryByRole("treeitem", { name: /index\.ts/ })).toBeNull();
  });

  it("activates the focused row with Enter and with Space", async () => {
    const user = await openedTree();
    itemNamed(/README/).focus();

    await user.keyboard("{Enter}");
    expect(await screen.findByLabelText("Read-only preview of README.md")).toBeTruthy();

    itemNamed(/^src/).focus();
    await user.keyboard(" ");
    await screen.findByRole("treeitem", { name: /index\.ts/ });

    // Activating a directory is a disclosure, so it goes both ways: the same
    // key on an open one closes it rather than opening it a second time.
    await user.keyboard("{Enter}");
    await waitFor(() => expect(treeItems()).toHaveLength(2));
    expect(itemNamed(/^src/).getAttribute("aria-expanded")).toBe("false");
  });
});

describe("WorkspaceExplorer filter", () => {
  /**
   * Loads both nested pages, then closes both directories again.
   *
   * The filter only searches what the renderer holds, so the pages have to be
   * fetched; closing the directories afterwards is what makes the tests below
   * about the filter rather than about what was already open. A page survives
   * its directory closing, so this is the state a user reaches by looking
   * around and then tidying up.
   */
  async function filterFor(query: string) {
    const user = userEvent.setup();
    renderExplorer(fakeSource(twoLevelPages));
    await user.click(await screen.findByRole("button", { name: /^src$/ }));
    await screen.findByRole("treeitem", { name: /index\.ts/ });
    await user.click(screen.getByRole("button", { name: /^workspace$/ }));
    await screen.findByRole("treeitem", { name: /scanner\.ts/ });

    await user.click(screen.getByRole("button", { name: /^workspace$/ }));
    await user.click(screen.getByRole("button", { name: /^src$/ }));
    await waitFor(() => expect(treeItems()).toHaveLength(2));

    await user.type(screen.getByLabelText("Filter loaded files"), query);
    return user;
  }

  it("says how many matched, out of how much it could search", async () => {
    await filterFor("scanner");
    expect(screen.getByRole("status").textContent).toBe(
      "1 of 5 loaded entries matches “scanner”. "
      + "Folders that have not been expanded are not searched.",
    );
    expect(treeItems().map((row) => row.getAttribute("aria-level"))).toEqual(["1", "2", "3"]);
  });

  it("says it is filtering rather than looking like an empty workspace", async () => {
    // The failure this avoids: a filter with no hits and a workspace with no
    // readable files are the same blank box, and only one is fixed by
    // clearing the filter.
    await filterFor("nothing-matches-this");
    expect(treeItems()).toHaveLength(0);
    expect(screen.getByRole("status").textContent).toBe(
      "No loaded entry matches “nothing-matches-this”. "
      + "5 loaded entries are hidden by this filter. "
      + "Folders that have not been expanded are not searched.",
    );
    expect(screen.getByText(/Every loaded entry is hidden by this filter/)).toBeTruthy();
    expect(screen.queryByText("No visible entries were projected.")).toBeNull();
  });

  it("does not record an expansion for a row the user asked to close", async () => {
    // src/ is closed, and the filter opens it to reach the hit underneath. A
    // toggle that asked the expanded set which way to go would read "closed"
    // for a row the user has just pressed Left on, and record an expansion --
    // silent while the filter holds the row open, and waiting for the user the
    // moment they cleared it.
    const user = await filterFor("scanner");
    const src = itemNamed(/^src/);
    expect(src.getAttribute("aria-expanded")).toBe("true");

    src.focus();
    await user.keyboard("{ArrowLeft}");
    await user.click(screen.getByRole("button", { name: "Clear the file filter" }));

    await waitFor(() => expect(treeItems()).toHaveLength(2));
    expect(itemNamed(/^src/).getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByRole("treeitem", { name: /index\.ts/ })).toBeNull();
  });
});

describe("WorkspaceExplorer expansion across workspaces", () => {
  it("does not carry one workspace's open directories onto another's files", async () => {
    const user = userEvent.setup();
    const alpha = fakeSource(twoLevelPages);
    const { rerender } = renderExplorer(alpha);

    await user.click(await screen.findByRole("button", { name: /^src$/ }));
    await screen.findByRole("treeitem", { name: /index\.ts/ });

    /*
     * Same id, new grant: the paths still resolve, so a carried-over expansion
     * would show the outgoing workspace's shape over the incoming one's files
     * -- and over a page nothing would ever fetch, because opening the
     * directory is what fetches it.
     */
    const beta = fakeSource({
      root: page([directory("src"), textFile("LICENSE")]),
      "cursor:src": page([textFile("src/other.ts")]),
    });
    rerender(
      <WorkspaceExplorer
        availabilityMessage="Unavailable"
        onContextReview={() => undefined}
        source={beta}
        workspaceName="Beta"
      />,
    );

    await screen.findByRole("treeitem", { name: /LICENSE/ });
    expect(itemNamed(/^src/).getAttribute("aria-expanded")).toBe("false");
    expect(treeItems()).toHaveLength(2);
    expect(beta.listEntries.mock.calls).toEqual([[null, 100]]);
  });

  it("closes what was open when the workspace is refreshed", async () => {
    const user = userEvent.setup();
    renderExplorer(fakeSource(twoLevelPages));

    await user.click(await screen.findByRole("button", { name: /^src$/ }));
    await screen.findByRole("treeitem", { name: /index\.ts/ });

    await user.click(screen.getByRole("button", { name: "Refresh workspace projections" }));

    await waitFor(() =>
      expect(itemNamed(/^src/).getAttribute("aria-expanded")).toBe("false"));
    expect(screen.queryByRole("treeitem", { name: /index\.ts/ })).toBeNull();
  });
});
