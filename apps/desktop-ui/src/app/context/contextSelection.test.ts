import { describe, expect, it } from "vitest";

import {
  maxContextFiles,
  mergeContextPaths,
  pickRejections,
  type FilePickOutcome,
} from "./contextSelection";

function pick(overrides: Partial<FilePickOutcome> = {}): FilePickOutcome {
  return {
    relativePaths: [],
    selectedCount: 0,
    rejectedOutsideRoot: 0,
    rejectedUnreadable: 0,
    truncated: false,
    ...overrides,
  };
}

describe("mergeContextPaths", () => {
  it("keeps what is already attached and adds the new picks", () => {
    const merge = mergeContextPaths(["src/a.ts"], ["src/b.ts"]);
    expect(merge.relativePaths).toEqual(["src/a.ts", "src/b.ts"]);
    expect(merge.cappedByLimit).toBe(false);
  });

  it("de-duplicates paths that differ only in case", () => {
    // Windows resolves these to one file. Attaching it twice spends the
    // model's context on a duplicate.
    const merge = mergeContextPaths(["Src/App.tsx"], ["src/app.tsx"]);
    expect(merge.relativePaths).toHaveLength(1);
  });

  it("keeps the spelling the picker just returned", () => {
    // It is the one the host will resolve.
    const merge = mergeContextPaths(["Src/App.tsx"], ["src/app.tsx"]);
    expect(merge.relativePaths).toEqual(["src/app.tsx"]);
  });

  it("de-duplicates within a single pick", () => {
    const merge = mergeContextPaths([], ["a.ts", "A.ts", "b.ts"]);
    expect(merge.relativePaths).toEqual(["A.ts", "b.ts"]);
  });

  it("caps the selection and says it did", () => {
    // A folder pick can select thousands. Refusing outright leaves the user
    // with nothing, so it truncates -- and reports that it truncated.
    const many = Array.from({ length: 150 }, (_, i) => `file-${i}.ts`);
    const merge = mergeContextPaths([], many);
    expect(merge.relativePaths).toHaveLength(maxContextFiles);
    expect(merge.cappedByLimit).toBe(true);
  });

  it("does not claim a cap when the selection fits exactly", () => {
    const exact = Array.from({ length: maxContextFiles }, (_, i) => `file-${i}.ts`);
    expect(mergeContextPaths([], exact).cappedByLimit).toBe(false);
  });

  it("caps on the merged total, not on the size of the new pick", () => {
    // The ordinary case: files are already attached and the user picks a few
    // more. Judging the cap by the pick alone would attach 90 of the 110 and
    // tell the user nothing was dropped.
    const existing = Array.from({ length: 90 }, (_, i) => `old-${i}.ts`);
    const picked = Array.from({ length: 20 }, (_, i) => `new-${i}.ts`);
    const merge = mergeContextPaths(existing, picked);
    expect(merge.relativePaths).toHaveLength(maxContextFiles);
    expect(merge.cappedByLimit).toBe(true);
  });

  it("counts duplicates before the cap, not after", () => {
    // 100 unique files reached by way of 120 picks is not a truncation, and
    // saying it is would tell the user files were dropped when none were.
    const paths = Array.from({ length: 100 }, (_, i) => `file-${i}.ts`);
    const merge = mergeContextPaths(paths, paths.map((p) => p.toUpperCase()));
    expect(merge.relativePaths).toHaveLength(100);
    expect(merge.cappedByLimit).toBe(false);
  });
});

describe("pickRejections", () => {
  it("says nothing when everything the user picked was attached", () => {
    // A notice on every success is one the user stops reading -- and then the
    // one that matters goes unread with it.
    const merge = mergeContextPaths([], ["a.ts"]);
    expect(pickRejections(pick({ selectedCount: 1, relativePaths: ["a.ts"] }), merge))
      .toEqual([]);
  });

  /// FE-14. The predecessor collapsed these into "3 unsupported files were
  /// skipped", which names a number and no cause -- and the two causes have
  /// different fixes: move the file in, or fix why it cannot be read.
  it("keeps the two rejection causes apart", () => {
    const merge = mergeContextPaths([], ["a.ts"]);
    const rejections = pickRejections(
      pick({ selectedCount: 1, rejectedOutsideRoot: 2, rejectedUnreadable: 1 }),
      merge,
    );
    expect(rejections).toHaveLength(2);
    expect(rejections[0]?.reason).toBe("2 files were outside this workspace.");
    expect(rejections[1]?.reason).toBe("1 file could not be read.");
  });

  it("agrees with itself about singular and plural", () => {
    const one = pickRejections(pick({ rejectedOutsideRoot: 1, rejectedUnreadable: 1 }));
    expect(one[0]?.reason).toBe("1 file was outside this workspace.");
    expect(one[1]?.reason).toBe("1 file could not be read.");
  });

  it("names no file it was not told the name of", () => {
    // The host reports counts, not paths: a file outside the workspace has no
    // workspace-relative path, and inventing one would be worse than the gap.
    for (const rejection of pickRejections(pick({ rejectedOutsideRoot: 2 }))) {
      expect(rejection.relativePath).toBe("");
    }
  });

  it("reports the picker's own truncation", () => {
    expect(pickRejections(pick({ truncated: true }))[0]?.reason)
      .toBe(`Only the first ${maxContextFiles} files were attached.`);
  });

  it("reports this module's cap the same way", () => {
    // Both mean the same thing to the user, so they read the same.
    const many = Array.from({ length: 150 }, (_, i) => `file-${i}.ts`);
    const merge = mergeContextPaths([], many);
    expect(pickRejections(pick(), merge)[0]?.reason)
      .toBe(`Only the first ${maxContextFiles} files were attached.`);
  });

  /// The predecessor returned one string, so a truncated pick that also had
  /// rejected files reported only one of the two. They are independent facts.
  it("reports a truncation and a rejection together", () => {
    const many = Array.from({ length: 150 }, (_, i) => `file-${i}.ts`);
    const merge = mergeContextPaths([], many);
    const rejections = pickRejections(pick({ selectedCount: 150, rejectedUnreadable: 5 }), merge);
    expect(rejections).toHaveLength(2);
    expect(rejections.map((rejection) => rejection.reason).join(" "))
      .toContain("could not be read");
  });

  it("works without a merge, for a pick that attached nothing", () => {
    // From the user's side, cancelling and having every file rejected look
    // identical: they chose files and nothing appeared.
    expect(pickRejections(pick({ rejectedOutsideRoot: 3 }))).toHaveLength(1);
    expect(pickRejections(pick())).toEqual([]);
  });
});

/**
 * FE-15. The host now names the refusals it can name.
 *
 * A file rejected below the containment check is proven to be inside the grant
 * and has a workspace-relative path, so its refusal says which file it was. One
 * decided above that check has no name but an absolute path from outside the
 * grant, so it stays a count -- and that split is the host's, reproduced here
 * rather than reinvented.
 */
describe("pickRejections with named refusals", () => {
  it("names the files the host named", () => {
    const rejections = pickRejections(pick({
      rejected: [
        { relativePath: "src/image.bin", reason: "not_text" },
        { relativePath: ".env", reason: "blocked" },
      ],
      rejectedOutsideRoot: 2,
    }));

    expect(rejections).toEqual([
      { reason: "Not a readable text file.", relativePath: "src/image.bin", retryable: false },
      {
        reason: "Not readable here: it is a link, a placeholder, or a protected file.",
        relativePath: ".env",
        retryable: false,
      },
    ]);
  });

  it("groups the ones it could not name, by cause", () => {
    // Two identical unnamed lines would read as a rendering fault, and a count
    // is all that can truthfully be said about a file with no name.
    const rejections = pickRejections(pick({
      rejected: [
        { relativePath: null, reason: "outside_workspace" },
        { relativePath: null, reason: "outside_workspace" },
        { relativePath: null, reason: "unreadable" },
      ],
    }));

    expect(rejections).toEqual([
      { reason: "2 files were outside this workspace.", relativePath: "" },
      { reason: "1 file could not be read.", relativePath: "" },
    ]);
  });

  it("puts the actionable ones first", () => {
    // A named file is something the user can go and look at. A count is not,
    // so it does not get to sit above the thing that is.
    const rejections = pickRejections(pick({
      rejected: [
        { relativePath: null, reason: "outside_workspace" },
        { relativePath: "src/image.bin", reason: "not_text" },
      ],
    }));

    expect(rejections[0]?.relativePath).toBe("src/image.bin");
  });

  it("ignores the counts once the host sends the list", () => {
    // The counts carry the host's older grouping, in which a binary file
    // inside the workspace is an outside-root rejection. Reading both would
    // report the same refusal twice, once correctly and once not.
    const rejections = pickRejections(pick({
      rejected: [{ relativePath: "src/image.bin", reason: "not_text" }],
      rejectedOutsideRoot: 1,
    }));

    expect(rejections).toHaveLength(1);
    expect(rejections[0]?.relativePath).toBe("src/image.bin");
  });

  it("says nothing when a pick refused nothing", () => {
    expect(pickRejections(pick({ rejected: [] }))).toEqual([]);
  });

  it("still reports the cap alongside the host's refusals", () => {
    const many = Array.from({ length: 150 }, (_, i) => `file-${i}.ts`);
    const merge = mergeContextPaths([], many);
    const rejections = pickRejections(
      pick({ rejected: [{ relativePath: "a.bin", reason: "not_text" }] }),
      merge,
    );

    expect(rejections).toHaveLength(2);
    expect(rejections[1]?.reason).toBe(`Only the first ${maxContextFiles} files were attached.`);
  });

  /**
   * FE-15's per-file retry, and where it is allowed to appear.
   *
   * Only an I/O failure is worth attaching again. A binary file will be binary
   * next time, a `.env` will still be sensitive, a file outside the workspace
   * will still be outside it -- so a retry control on any of those is one that
   * cannot succeed, and pressing it teaches the user only that the app is not
   * working.
   */
  it("offers a retry only for a refusal that could go differently", () => {
    const rejections = pickRejections(pick({
      rejected: [
        { relativePath: "src/locked.ts", reason: "unreadable" },
        { relativePath: "src/image.bin", reason: "not_text" },
        { relativePath: ".env", reason: "blocked" },
      ],
    }));

    expect(rejections.map((rejection) => rejection.retryable))
      .toEqual([true, false, false]);
  });

  it("never offers a retry for a refusal the host would not name", () => {
    // Retrying needs a file to name, and these are exactly the ones it has no
    // name for.
    const rejections = pickRejections(pick({
      rejected: [
        { relativePath: null, reason: "unreadable" },
        { relativePath: null, reason: "outside_workspace" },
      ],
    }));

    expect(rejections.every((rejection) => rejection.retryable !== true)).toBe(true);
  });

  it("falls back to the counts for a host that does not send the list", () => {
    // `rejected` absent is an older host, not a pick that refused nothing --
    // and the difference decides whether the user is told anything at all.
    expect(pickRejections(pick({ rejectedUnreadable: 2 }))).toEqual([
      { reason: "2 files could not be read.", relativePath: "" },
    ]);
  });
});
