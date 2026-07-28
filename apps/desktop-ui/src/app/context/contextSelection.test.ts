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
