import { describe, expect, it } from "vitest";
import { computeLineDiff, type DiffLine } from "./lineDiff";

function flatten(diff: ReturnType<typeof computeLineDiff>): DiffLine[] {
  return diff.segments.flatMap((segment) => [...segment.lines]);
}

describe("computeLineDiff", () => {
  it("reports a pure creation as only added lines", () => {
    const diff = computeLineDiff("", "one\ntwo\n");
    expect(diff.addedCount).toBe(2);
    expect(diff.removedCount).toBe(0);
    expect(flatten(diff).map((line) => line.kind)).toEqual(["added", "added"]);
  });

  it("reports a pure deletion as only removed lines", () => {
    const diff = computeLineDiff("one\ntwo\n", "");
    expect(diff.addedCount).toBe(0);
    expect(diff.removedCount).toBe(2);
  });

  it("aligns an interior edit and keeps surrounding context", () => {
    const diff = computeLineDiff("a\nb\nc\n", "a\nB\nc\n");
    const lines = flatten(diff);
    expect(diff.addedCount).toBe(1);
    expect(diff.removedCount).toBe(1);
    expect(lines.map((line) => line.kind)).toEqual([
      "context",
      "removed",
      "added",
      "context",
    ]);
    const removed = lines.find((line) => line.kind === "removed");
    const added = lines.find((line) => line.kind === "added");
    expect(removed?.text).toBe("b");
    expect(added?.text).toBe("B");
  });

  it("collapses long unchanged runs while keeping edge context", () => {
    const body = Array.from({ length: 40 }, (_, index) => `line-${index}`).join("\n");
    const diff = computeLineDiff(`first\n${body}\nlast\n`, `FIRST\n${body}\nlast\n`);
    const collapsed = diff.segments.filter((segment) => segment.kind === "collapsed");
    expect(collapsed.length).toBe(1);
    expect(collapsed[0]!.lines.length).toBeGreaterThan(20);
    const visibleTexts = diff.segments
      .filter((segment) => segment.kind === "visible")
      .flatMap((segment) => segment.lines.map((line) => line.text));
    expect(visibleTexts).toContain("line-0");
    expect(visibleTexts).not.toContain("line-20");
  });

  it("numbers lines from each side independently", () => {
    const diff = computeLineDiff("keep\nold\n", "keep\nnew\nextra\n");
    const lines = flatten(diff);
    const removed = lines.find((line) => line.kind === "removed");
    const added = lines.filter((line) => line.kind === "added");
    expect(removed).toMatchObject({ beforeLine: 2 });
    expect(added).toEqual([
      expect.objectContaining({ afterLine: 2 }),
      expect.objectContaining({ afterLine: 3 }),
    ]);
  });

  it("falls back to block replacement for oversized cores without losing lines", () => {
    const before = Array.from({ length: 600 }, (_, index) => `b-${index}`).join("\n");
    const after = Array.from({ length: 600 }, (_, index) => `a-${index}`).join("\n");
    const diff = computeLineDiff(before, after);
    expect(diff.removedCount).toBe(600);
    expect(diff.addedCount).toBe(600);
  });
});
