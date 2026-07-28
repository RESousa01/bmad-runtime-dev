import { describe, expect, it } from "vitest";

import type { ContextPreviewProjection } from "../../lib/hostClient";
import {
  adoptPreview,
  attachmentPaths,
  clearAttachments,
  emptyTaskAttachments,
  removeAttachment,
} from "./attachmentTypes";

function preview(
  items: readonly { path: string; hash: string }[],
  manifestHash = "manifest-1",
): ContextPreviewProjection {
  return {
    estimatedTokens: items.length * 8,
    items: items.map(({ hash, path }) => ({
      byteCount: 24,
      classification: "source",
      content: "authenticated source content",
      contentHash: hash,
      endLine: 3,
      estimatedTokens: 8,
      reason: "Selected for this task",
      redactions: [],
      relativePath: path,
      startLine: 1,
    })),
    manifestHash,
    modelTarget: null,
    totalBytes: items.length * 24,
    workspaceId: "workspace-1",
  };
}

describe("attachment identity", () => {
  it("gives every attachment an id that is not its path", () => {
    // The path is a label. Using a label as an identity is how a list comes to
    // act on the wrong row.
    const { descriptors } = adoptPreview(preview([
      { hash: "hash-a", path: "src/a.ts" },
      { hash: "hash-b", path: "src/b.ts" },
    ]), "manual");

    expect(descriptors[0]?.attachmentId).not.toContain("src/a.ts");
    expect(descriptors[0]?.attachmentId).not.toBe(descriptors[1]?.attachmentId);
  });

  it("gives the same set the same ids twice", () => {
    // Otherwise a re-render looks like every row being replaced.
    const items = [{ hash: "hash-a", path: "src/a.ts" }];
    expect(adoptPreview(preview(items), "manual").descriptors[0]?.attachmentId)
      .toBe(adoptPreview(preview(items), "manual").descriptors[0]?.attachmentId);
  });

  it("separates two files whose content is identical", () => {
    // Same bytes, different files. An id derived from the hash alone would
    // collide, and removing one would remove both.
    const { descriptors } = adoptPreview(preview([
      { hash: "same-hash", path: "src/a.ts" },
      { hash: "same-hash", path: "src/copy.ts" },
    ]), "manual");

    expect(descriptors[0]?.attachmentId).not.toBe(descriptors[1]?.attachmentId);
  });

  it("separates two entries whose paths differ only in case", () => {
    // Windows resolves these to one file, so a path-keyed list cannot tell
    // them apart -- which is precisely when acting on the wrong row destroys
    // the user's selection.
    const { descriptors } = adoptPreview(preview([
      { hash: "hash-a", path: "Src/App.tsx" },
      { hash: "hash-b", path: "src/app.tsx" },
    ]), "manual");

    expect(descriptors[0]?.attachmentId).not.toBe(descriptors[1]?.attachmentId);
  });
});

describe("removeAttachment", () => {
  const attached = adoptPreview(preview([
    { hash: "hash-a", path: "src/a.ts" },
    { hash: "hash-b", path: "src/b.ts" },
  ]), "manual");

  it("drops exactly the one named", () => {
    const id = attached.descriptors[0]?.attachmentId ?? "";
    const next = removeAttachment(attached, id);

    expect(attachmentPaths(next)).toEqual(["src/b.ts"]);
  });

  /**
   * The manifest hash is the host's confirmation of *that exact set*.
   *
   * Keeping it against a set the user has edited would let a send name a
   * reviewed manifest for files it is no longer carrying: consent recorded for
   * one selection and spent on another.
   */
  it("drops the manifest hash with it", () => {
    const id = attached.descriptors[0]?.attachmentId ?? "";
    expect(attached.manifestHash).toBe("manifest-1");
    expect(removeAttachment(attached, id).manifestHash).toBeNull();
  });

  it("leaves the set alone when the id matches nothing", () => {
    // A row can be pressed twice, and the second press is not an error.
    expect(removeAttachment(attached, "att_nothing_0")).toBe(attached);
  });

  it("keeps the manifest hash when it removed nothing", () => {
    // A no-op must not invalidate a set the host did confirm.
    expect(removeAttachment(attached, "att_nothing_0").manifestHash).toBe("manifest-1");
  });

  it("removes only one of two entries sharing a path spelling", () => {
    const doubled = adoptPreview(preview([
      { hash: "hash-a", path: "Src/App.tsx" },
      { hash: "hash-b", path: "src/app.tsx" },
    ]), "manual");
    const id = doubled.descriptors[0]?.attachmentId ?? "";

    expect(removeAttachment(doubled, id).descriptors).toHaveLength(1);
  });

  it("can empty the set", () => {
    const one = adoptPreview(preview([{ hash: "hash-a", path: "src/a.ts" }]), "manual");
    const id = one.descriptors[0]?.attachmentId ?? "";
    expect(removeAttachment(one, id).descriptors).toEqual([]);
  });

  it("does nothing to an empty set", () => {
    expect(removeAttachment(emptyTaskAttachments, "att_x_0")).toBe(emptyTaskAttachments);
  });

  it("keeps the selection mode", () => {
    // Removing a file from an automatic selection does not make it manual --
    // the user edited it, they did not choose it.
    const auto = adoptPreview(preview([
      { hash: "hash-a", path: "src/a.ts" },
      { hash: "hash-b", path: "src/b.ts" },
    ]), "automatic");
    const id = auto.descriptors[0]?.attachmentId ?? "";
    expect(removeAttachment(auto, id).selectionMode).toBe("automatic");
  });
});

describe("clearAttachments", () => {
  it("leaves nothing armed for the next message", () => {
    // Leaving them armed re-sent, re-billed and re-entered every file into the
    // reviewed-context manifest: consent given once, acted on every turn.
    expect(clearAttachments().descriptors).toEqual([]);
    expect(clearAttachments().manifestHash).toBeNull();
  });
});
