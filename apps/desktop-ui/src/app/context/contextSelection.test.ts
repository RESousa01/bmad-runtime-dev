import { describe, expect, it } from "vitest";

import {
  attachmentNotice,
  emptyPickNotice,
  maxContextFiles,
  mergeContextPaths,
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

describe("attachmentNotice", () => {
  it("says nothing when everything the user picked was attached", () => {
    // A notice on every success is one the user stops reading -- and then the
    // one that matters goes unread with it.
    const merge = mergeContextPaths([], ["a.ts"]);
    expect(attachmentNotice(pick({ selectedCount: 1, relativePaths: ["a.ts"] }), merge))
      .toBeNull();
  });

  it("reports files rejected by the host", () => {
    const merge = mergeContextPaths([], ["a.ts"]);
    const notice = attachmentNotice(
      pick({ selectedCount: 1, rejectedOutsideRoot: 2, rejectedUnreadable: 1 }),
      merge,
    );
    expect(notice).toBe("Added 1 files. 3 unsupported files were skipped.");
  });

  it("counts both rejection reasons together", () => {
    const merge = mergeContextPaths([], ["a.ts"]);
    expect(attachmentNotice(pick({ rejectedOutsideRoot: 4 }), merge))
      .toContain("4 unsupported");
    expect(attachmentNotice(pick({ rejectedUnreadable: 4 }), merge))
      .toContain("4 unsupported");
  });

  it("reports the picker's own truncation", () => {
    const merge = mergeContextPaths([], ["a.ts"]);
    expect(attachmentNotice(pick({ truncated: true }), merge))
      .toBe(`Added the first ${maxContextFiles} supported files.`);
  });

  it("reports this module's cap the same way", () => {
    // Both mean the same thing to the user, so they read the same.
    const many = Array.from({ length: 150 }, (_, i) => `file-${i}.ts`);
    const merge = mergeContextPaths([], many);
    expect(attachmentNotice(pick(), merge))
      .toBe(`Added the first ${maxContextFiles} supported files.`);
  });

  it("prefers the rejection message over the truncation one", () => {
    // Rejected files are the more surprising outcome: the user chose them and
    // they are not there.
    const many = Array.from({ length: 150 }, (_, i) => `file-${i}.ts`);
    const merge = mergeContextPaths([], many);
    expect(attachmentNotice(pick({ selectedCount: 150, rejectedUnreadable: 5 }), merge))
      .toContain("skipped");
  });
});

describe("emptyPickNotice", () => {
  it("says nothing when the user cancelled the dialog", () => {
    // Cancelling is not a failure.
    expect(emptyPickNotice(0)).toBeNull();
  });

  it("explains when everything the user chose was rejected", () => {
    // From the user's side, cancelling and having every file rejected look
    // identical: they chose files and nothing appeared.
    expect(emptyPickNotice(3))
      .toBe("No files were attached. 3 were outside the workspace or unreadable.");
  });
});
