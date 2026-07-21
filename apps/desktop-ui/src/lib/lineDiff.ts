/**
 * Bounded line diff for the governed-changes review. Pure and dependency
 * free: common prefix/suffix lines are trimmed first, the remaining core is
 * aligned with a longest-common-subsequence table, and cores too large to
 * table fall back to a whole-block replacement so review time stays bounded.
 */

export type DiffLine =
  | { kind: "context"; text: string; beforeLine: number; afterLine: number }
  | { kind: "removed"; text: string; beforeLine: number }
  | { kind: "added"; text: string; afterLine: number };

export type DiffSegment =
  | { kind: "visible"; lines: readonly DiffLine[] }
  | { kind: "collapsed"; lines: readonly DiffLine[] };

export interface LineDiff {
  readonly segments: readonly DiffSegment[];
  readonly addedCount: number;
  readonly removedCount: number;
}

const LCS_CELL_LIMIT = 250_000;
const CONTEXT_LINES = 3;

function splitLines(content: string): string[] {
  if (content.length === 0) {
    return [];
  }
  const lines = content.split("\n");
  if (lines.at(-1) === "") {
    lines.pop();
  }
  return lines;
}

function alignCore(before: readonly string[], after: readonly string[]): DiffLine[] {
  if (before.length * after.length > LCS_CELL_LIMIT) {
    return [
      ...before.map((text, index): DiffLine => ({ kind: "removed", text, beforeLine: index + 1 })),
      ...after.map((text, index): DiffLine => ({ kind: "added", text, afterLine: index + 1 })),
    ];
  }
  const width = after.length + 1;
  const table = new Uint32Array((before.length + 1) * width);
  for (let i = before.length - 1; i >= 0; i -= 1) {
    for (let j = after.length - 1; j >= 0; j -= 1) {
      table[i * width + j] = before[i] === after[j]
        ? table[(i + 1) * width + j + 1]! + 1
        : Math.max(table[(i + 1) * width + j]!, table[i * width + j + 1]!);
    }
  }
  const lines: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < before.length && j < after.length) {
    if (before[i] === after[j]) {
      lines.push({ kind: "context", text: before[i]!, beforeLine: i + 1, afterLine: j + 1 });
      i += 1;
      j += 1;
    } else if (table[(i + 1) * width + j]! >= table[i * width + j + 1]!) {
      lines.push({ kind: "removed", text: before[i]!, beforeLine: i + 1 });
      i += 1;
    } else {
      lines.push({ kind: "added", text: after[j]!, afterLine: j + 1 });
      j += 1;
    }
  }
  while (i < before.length) {
    lines.push({ kind: "removed", text: before[i]!, beforeLine: i + 1 });
    i += 1;
  }
  while (j < after.length) {
    lines.push({ kind: "added", text: after[j]!, afterLine: j + 1 });
    j += 1;
  }
  return lines;
}

function renumber(lines: DiffLine[], beforeOffset: number, afterOffset: number): DiffLine[] {
  return lines.map((line) => {
    if (line.kind === "context") {
      return {
        ...line,
        beforeLine: line.beforeLine + beforeOffset,
        afterLine: line.afterLine + afterOffset,
      };
    }
    if (line.kind === "removed") {
      return { ...line, beforeLine: line.beforeLine + beforeOffset };
    }
    return { ...line, afterLine: line.afterLine + afterOffset };
  });
}

function segment(lines: readonly DiffLine[]): DiffSegment[] {
  const segments: DiffSegment[] = [];
  let contextRun: DiffLine[] = [];
  let visibleRun: DiffLine[] = [];

  const flushContext = (isEnd: boolean) => {
    const isStart = segments.length === 0 && visibleRun.length === 0;
    const leading = isStart ? 0 : CONTEXT_LINES;
    const trailing = isEnd ? 0 : CONTEXT_LINES;
    if (contextRun.length > leading + trailing + 2) {
      visibleRun.push(...contextRun.slice(0, leading));
      if (visibleRun.length > 0) {
        segments.push({ kind: "visible", lines: visibleRun });
        visibleRun = [];
      }
      segments.push({
        kind: "collapsed",
        lines: contextRun.slice(leading, contextRun.length - trailing),
      });
      visibleRun.push(...contextRun.slice(contextRun.length - trailing));
    } else {
      visibleRun.push(...contextRun);
    }
    contextRun = [];
  };

  for (const line of lines) {
    if (line.kind === "context") {
      contextRun.push(line);
    } else {
      flushContext(false);
      visibleRun.push(line);
    }
  }
  flushContext(true);
  if (visibleRun.length > 0) {
    segments.push({ kind: "visible", lines: visibleRun });
  }
  return segments;
}

export function computeLineDiff(before: string, after: string): LineDiff {
  const beforeLines = splitLines(before);
  const afterLines = splitLines(after);

  let prefix = 0;
  while (
    prefix < beforeLines.length &&
    prefix < afterLines.length &&
    beforeLines[prefix] === afterLines[prefix]
  ) {
    prefix += 1;
  }
  let suffix = 0;
  while (
    suffix < beforeLines.length - prefix &&
    suffix < afterLines.length - prefix &&
    beforeLines[beforeLines.length - 1 - suffix] === afterLines[afterLines.length - 1 - suffix]
  ) {
    suffix += 1;
  }

  const lines: DiffLine[] = [];
  for (let index = 0; index < prefix; index += 1) {
    lines.push({
      kind: "context",
      text: beforeLines[index]!,
      beforeLine: index + 1,
      afterLine: index + 1,
    });
  }
  lines.push(
    ...renumber(
      alignCore(
        beforeLines.slice(prefix, beforeLines.length - suffix),
        afterLines.slice(prefix, afterLines.length - suffix),
      ),
      prefix,
      prefix,
    ),
  );
  for (let index = 0; index < suffix; index += 1) {
    const beforeLine = beforeLines.length - suffix + index + 1;
    const afterLine = afterLines.length - suffix + index + 1;
    lines.push({
      kind: "context",
      text: beforeLines[beforeLine - 1]!,
      beforeLine,
      afterLine,
    });
  }

  return {
    segments: segment(lines),
    addedCount: lines.filter((line) => line.kind === "added").length,
    removedCount: lines.filter((line) => line.kind === "removed").length,
  };
}
