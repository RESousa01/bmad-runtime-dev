import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  GitFileComparisonProjection,
  GitStatusProjection,
} from "../lib/hostClient";
import {
  GitReviewPanel,
  ReviewPanel,
  type GitReviewSource,
} from "./GitReviewPanel";

const status: GitStatusProjection = {
  branch: "codex/forge-v2",
  entries: [
    {
      relativePath: "src/App.tsx",
      indexState: "modified",
      worktreeState: "modified",
    },
    {
      relativePath: "src/new-panel.tsx",
      indexState: null,
      worktreeState: "untracked",
    },
  ],
  truncated: false,
};

function comparison(
  relativePath: string,
  baseline: GitFileComparisonProjection["comparison"],
): GitFileComparisonProjection {
  return {
    relativePath,
    comparison: baseline,
    beforeContent: "export const name = \"Sapphirus\";\n",
    afterContent: "export const name = \"Forge\";\n",
    truncated: false,
  };
}

function createSource(): GitReviewSource {
  return {
    readStatus: vi.fn().mockResolvedValue(status),
    readDiff: vi.fn().mockImplementation((path, baseline) =>
      Promise.resolve(comparison(path, baseline))),
  };
}

afterEach(cleanup);

describe("GitReviewPanel", () => {
  /// Browser preview serves a fabricated branch, file list and diff. Unlabelled,
  /// that is indistinguishable from the user's own repository — a branch name,
  /// modified files and real-looking counts, none of which came from anywhere.
  /// The label has to sit with the content, because the content is what is
  /// being misread.
  it("labels a sample-data working tree, above the content it describes", async () => {
    const source = createSource();
    render(<GitReviewPanel isSampleData source={source} />);

    const note = await screen.findByRole("note");
    expect(note.textContent).toContain("not your repository");
    const branch = await screen.findByText("codex/forge-v2");
    // Above, not tucked in a corner below.
    expect(note.compareDocumentPosition(branch) & Node.DOCUMENT_POSITION_FOLLOWING)
      .toBeTruthy();
  });

  it("does not label a real working tree as sample data", async () => {
    const source = createSource();
    render(<GitReviewPanel source={source} />);

    // The branch must still render, so this cannot pass by rendering nothing.
    expect(await screen.findByText("codex/forge-v2")).toBeTruthy();
    expect(screen.queryByRole("note")).toBeNull();
  });

  it("shows the working-tree summary and exact per-line Git difference", async () => {
    const source = createSource();
    render(<GitReviewPanel source={source} />);

    expect(await screen.findByText("codex/forge-v2")).toBeTruthy();
    expect(screen.getByRole("button", { name: /src\/App\.tsx/u })).toBeTruthy();
    expect(screen.getByRole("button", { name: /src\/new-panel\.tsx/u })).toBeTruthy();

    const diff = await screen.findByRole("region", { name: "Changes to src/App.tsx" });
    expect(diff.querySelector(".diff-line--removed")?.textContent).toContain("Sapphirus");
    expect(diff.querySelector(".diff-line--added")?.textContent).toContain("Forge");
    expect(source.readDiff).toHaveBeenCalledWith("src/App.tsx", "head_to_worktree");
  });

  it("uses the OpenCode review window with a filterable tree and split diff", async () => {
    render(<GitReviewPanel source={createSource()} />);

    expect(await screen.findByRole("searchbox", { name: "Filter files" })).toBeTruthy();
    expect(document.querySelector('[data-component="session-review-v2"]')).toBeTruthy();
    expect(await screen.findByRole("region", { name: "Split changes to src/App.tsx" })).toBeTruthy();

    fireEvent.change(screen.getByRole("searchbox", { name: "Filter files" }), {
      target: { value: "new-panel" },
    });
    expect(screen.queryByRole("button", { name: /src\/App\.tsx/u })).toBeNull();
    expect(screen.getByRole("button", { name: /src\/new-panel\.tsx/u })).toBeTruthy();
  });

  it("filters staged and unstaged files and requests the matching baseline", async () => {
    const source = createSource();
    render(<GitReviewPanel source={source} />);
    await screen.findByRole("region", { name: "Changes to src/App.tsx" });

    fireEvent.change(screen.getByRole("combobox", { name: "Git comparison" }), {
      target: { value: "head_to_index" },
    });
    await waitFor(() => {
      expect(source.readDiff).toHaveBeenCalledWith("src/App.tsx", "head_to_index");
    });
    expect(screen.queryByRole("button", { name: /src\/new-panel\.tsx/u })).toBeNull();

    fireEvent.change(screen.getByRole("combobox", { name: "Git comparison" }), {
      target: { value: "index_to_worktree" },
    });
    await waitFor(() => {
      expect(source.readDiff).toHaveBeenCalledWith("src/App.tsx", "index_to_worktree");
    });
    expect(screen.getByRole("button", { name: /src\/new-panel\.tsx/u })).toBeTruthy();
  });

  it("does not replace a newer selection with a stale diff response", async () => {
    let resolveFirst: ((value: GitFileComparisonProjection) => void) | undefined;
    const source: GitReviewSource = {
      readStatus: vi.fn().mockResolvedValue(status),
      readDiff: vi.fn()
        .mockImplementationOnce(() => new Promise((resolve) => {
          resolveFirst = resolve;
        }))
        .mockImplementationOnce((path, baseline) =>
          Promise.resolve({
            ...comparison(path, baseline),
            beforeContent: "old panel\n",
            afterContent: "new panel\n",
          })),
    };
    render(<GitReviewPanel source={source} />);
    await screen.findByRole("button", { name: /src\/new-panel\.tsx/u });

    fireEvent.click(screen.getByRole("button", { name: /src\/new-panel\.tsx/u }));
    expect(await screen.findByRole("region", {
      name: "Changes to src/new-panel.tsx",
    })).toBeTruthy();

    resolveFirst?.(comparison("src/App.tsx", "head_to_worktree"));
    await waitFor(() => {
      expect(screen.queryByRole("region", { name: "Changes to src/App.tsx" })).toBeNull();
    });
  });

  it("keeps governed agent proposals inside Review", async () => {
    render(
      <ReviewPanel
        governedReview={<div>Bound proposal review</div>}
        source={createSource()}
      />,
    );
    expect(await screen.findByRole("heading", { name: "Working tree" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Agent proposals" }));
    expect(screen.getByText("Bound proposal review")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Back to Git changes" }));
    expect(await screen.findByRole("heading", { name: "Working tree" })).toBeTruthy();
  });
});

/**
 * FE-22. The Review Center used to be a mode switch: git *or* proposals, with
 * whichever was hidden shrunk to a 14px icon carrying a dot. The view flipped
 * from an effect watching an attention flag rise, so a second proposal arriving
 * while the flag was already true moved nothing, and a user who had navigated
 * back to Git sat in front of an unanswered decision represented by a dot.
 */
describe("ReviewPanel — what leads", () => {
  function panel(overrides: Partial<Parameters<typeof ReviewPanel>[0]> = {}) {
    return (
      <ReviewPanel
        governedReview={<h3>Agent proposal body</h3>}
        source={createSource()}
        {...overrides}
      />
    );
  }

  it("shows git changes when nothing is waiting on an answer", async () => {
    render(panel());
    expect(await screen.findByRole("heading", { name: "Working tree" })).toBeTruthy();
  });

  /// The failure this replaces: a decision reachable only through a dot.
  it("leads with the decision when a proposal is pending", () => {
    render(panel({ proposalAttention: true }));
    expect(screen.getByRole("heading", { name: "Agent proposal body" })).toBeTruthy();
  });

  it("refuses to be browsed away from while a decision stands", () => {
    // A decision the user can walk away from is one they can lose track of.
    render(panel({ proposalAttention: true }));
    expect(screen.queryByRole("button", { name: "Back to Git changes" })).toBeNull();
  });

  it("still lets the user reach proposals when nothing is pending", async () => {
    // The governed side is not only decisions -- it is change history,
    // enabling edits, and starting a proposal.
    render(panel());
    await screen.findByRole("heading", { name: "Working tree" });
    fireEvent.click(screen.getByRole("button", { name: "Agent proposals" }));

    expect(screen.getByRole("heading", { name: "Agent proposal body" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Back to Git changes" })).toBeTruthy();
  });

  /*
   * There is no attention-marker test, because there is no marker.
   *
   * A pending decision pins the surface to itself, so the git view renders
   * only when nothing is waiting -- a dot there could never appear. The dot
   * was the old answer to "your decision is somewhere else", and being
   * somewhere else was the defect.
   */

  /// A recovery review is a different question from a different source, and
  /// the header said "Agent proposals" over it.
  it("names a recovery review rather than calling it a proposal", () => {
    render(panel({ hasPreparedRecovery: true }));
    expect(screen.getByRole("heading", { name: "Recovery review" })).toBeTruthy();
  });

  it("keeps naming it a recovery when a proposal is pending too", () => {
    // Recovery outranks the proposal, so it is the one being answered.
    render(panel({ hasPreparedRecovery: true, proposalAttention: true }));
    expect(screen.getByRole("heading", { name: "Recovery review" })).toBeTruthy();
  });
});
