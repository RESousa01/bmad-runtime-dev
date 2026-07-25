import { useMemo, useState } from "react";
import { computeLineDiff, type DiffLine } from "../lib/lineDiff";

export interface ChangeDiffProps {
  readonly relativePath: string;
  readonly beforeContent: string | null;
  readonly afterContent: string | null;
  readonly showAllLines?: boolean | undefined;
}

export function diffCounts(
  beforeContent: string | null,
  afterContent: string | null,
): { added: number; removed: number } {
  const diff = computeLineDiff(beforeContent ?? "", afterContent ?? "");
  return { added: diff.addedCount, removed: diff.removedCount };
}

const METER_BLOCKS = 5;

/**
 * Compact five-block magnitude meter for a file change, in the style of
 * GitHub and OpenCode diff summaries: proportional green/red blocks with a
 * neutral remainder, never overstating small edits.
 */
export function ChangeMagnitude({ added, removed }: { added: number; removed: number }) {
  const total = added + removed;
  let addBlocks = 0;
  let removeBlocks = 0;
  if (total > 0) {
    const colorBlocks = total < 20 ? METER_BLOCKS - 1 : METER_BLOCKS;
    addBlocks = added > 0 ? Math.max(1, Math.round((added / total) * colorBlocks)) : 0;
    removeBlocks = removed > 0 ? Math.max(1, Math.round((removed / total) * colorBlocks)) : 0;
    if (added > 0 && added <= 5) {
      addBlocks = 1;
    }
    if (removed > 0 && removed <= 5) {
      removeBlocks = 1;
    }
    while (addBlocks + removeBlocks > METER_BLOCKS) {
      if (addBlocks >= removeBlocks) {
        addBlocks -= 1;
      } else {
        removeBlocks -= 1;
      }
    }
  }
  const neutralBlocks = METER_BLOCKS - addBlocks - removeBlocks;
  const blocks = [
    ...Array.from({ length: addBlocks }, () => "added"),
    ...Array.from({ length: removeBlocks }, () => "removed"),
    ...Array.from({ length: neutralBlocks }, () => "neutral"),
  ];
  return (
    <span aria-hidden="true" className="diff-magnitude">
      {blocks.map((kind, index) => (
        <span className={`diff-magnitude__block diff-magnitude__block--${kind}`} key={index} />
      ))}
    </span>
  );
}

/** Renders a workspace path with the directory dimmed and the name bright. */
export function FilePathLabel({ relativePath }: { readonly relativePath: string }) {
  const separator = relativePath.lastIndexOf("/");
  const directory = separator === -1 ? "" : relativePath.slice(0, separator + 1);
  const name = separator === -1 ? relativePath : relativePath.slice(separator + 1);
  return (
    <code className="diff-file-path">
      {directory.length > 0 ? (
        <span className="diff-file-path__dir">{directory}</span>
      ) : null}
      <span className="diff-file-path__name">{name}</span>
    </code>
  );
}

function DiffRow({ line }: { readonly line: DiffLine }) {
  const marker = line.kind === "added" ? "+" : line.kind === "removed" ? "-" : " ";
  return (
    <div className={`diff-line diff-line--${line.kind}`}>
      <span aria-hidden="true" className="diff-line__no">
        {line.kind === "added" ? "" : line.beforeLine}
      </span>
      <span aria-hidden="true" className="diff-line__no">
        {line.kind === "removed" ? "" : line.afterLine}
      </span>
      <span aria-hidden="true" className="diff-line__marker">{marker}</span>
      <span className="diff-line__text">{line.text.length === 0 ? " " : line.text}</span>
    </div>
  );
}

interface SplitCell {
  readonly kind: "added" | "context" | "removed";
  readonly line: number;
  readonly text: string;
}

interface SplitRowValue {
  readonly after: SplitCell | null;
  readonly before: SplitCell | null;
}

function splitRows(lines: readonly DiffLine[]): SplitRowValue[] {
  const rows: SplitRowValue[] = [];
  let index = 0;
  while (index < lines.length) {
    const line = lines[index]!;
    if (line.kind === "context") {
      rows.push({
        before: { kind: "context", line: line.beforeLine, text: line.text },
        after: { kind: "context", line: line.afterLine, text: line.text },
      });
      index += 1;
      continue;
    }
    const removed: Extract<DiffLine, { kind: "removed" }>[] = [];
    const added: Extract<DiffLine, { kind: "added" }>[] = [];
    while (index < lines.length && lines[index]!.kind !== "context") {
      const changed = lines[index]!;
      if (changed.kind === "removed") removed.push(changed);
      if (changed.kind === "added") added.push(changed);
      index += 1;
    }
    const changeRows = Math.max(removed.length, added.length);
    for (let changeIndex = 0; changeIndex < changeRows; changeIndex += 1) {
      const before = removed[changeIndex];
      const after = added[changeIndex];
      rows.push({
        before: before
          ? { kind: "removed", line: before.beforeLine, text: before.text }
          : null,
        after: after
          ? { kind: "added", line: after.afterLine, text: after.text }
          : null,
      });
    }
  }
  return rows;
}

function SplitDiffCell({ cell, side }: {
  readonly cell: SplitCell | null;
  readonly side: "after" | "before";
}) {
  const kind = cell?.kind ?? "empty";
  const marker = cell?.kind === "added" ? "+" : cell?.kind === "removed" ? "−" : "";
  return (
    <div className={`split-diff__cell split-diff__cell--${kind} ${cell?.kind === "added" || cell?.kind === "removed" ? `diff-line--${cell.kind}` : ""}`} data-side={side}>
      <span aria-hidden="true" className="split-diff__line-number">{cell?.line ?? ""}</span>
      <span aria-hidden="true" className="split-diff__marker">{marker}</span>
      <span className="split-diff__text">{cell ? (cell.text || " ") : " "}</span>
    </div>
  );
}

/** OpenCode-shaped side-by-side file review with aligned before/after lines. */
export function SplitChangeDiff({ relativePath, beforeContent, afterContent, showAllLines = false }: ChangeDiffProps) {
  const diff = useMemo(
    () => computeLineDiff(beforeContent ?? "", afterContent ?? ""),
    [beforeContent, afterContent],
  );
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(new Set());

  return (
    <div aria-label={`Split changes to ${relativePath}`} className="split-diff" role="region">
      {diff.segments.map((segment, segmentIndex) => {
        if (segment.kind === "collapsed" && !showAllLines && !expanded.has(segmentIndex)) {
          return (
            <button
              className="split-diff__collapsed"
              key={`segment-${segmentIndex}`}
              onClick={() => setExpanded((current) => new Set(current).add(segmentIndex))}
              type="button"
            >
              {segment.lines.length} unchanged {segment.lines.length === 1 ? "line" : "lines"}
            </button>
          );
        }
        return (
          <div className="split-diff__segment" key={`segment-${segmentIndex}`}>
            {splitRows(segment.lines).map((row, rowIndex) => (
              <div className="split-diff__row" key={`${segmentIndex}-${rowIndex}`}>
                <SplitDiffCell cell={row.before} side="before" />
                <SplitDiffCell cell={row.after} side="after" />
              </div>
            ))}
          </div>
        );
      })}
    </div>
  );
}

export function ChangeDiff({ relativePath, beforeContent, afterContent, showAllLines = false }: ChangeDiffProps) {
  const diff = useMemo(
    () => computeLineDiff(beforeContent ?? "", afterContent ?? ""),
    [beforeContent, afterContent],
  );
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(new Set());

  return (
    <div aria-label={`Changes to ${relativePath}`} className="diff-view" role="region">
      {diff.segments.map((segment, index) =>
        segment.kind === "collapsed" && !showAllLines && !expanded.has(index) ? (
          <button
            className="diff-collapsed"
            key={`segment-${index}`}
            onClick={() => {
              setExpanded((current) => new Set(current).add(index));
            }}
            type="button"
          >
            {segment.lines.length} unchanged {segment.lines.length === 1 ? "line" : "lines"}
          </button>
        ) : (
          <div className="diff-segment" key={`segment-${index}`}>
            {segment.lines.map((line) => (
              <DiffRow
                key={`${line.kind}-${"beforeLine" in line ? line.beforeLine : ""}-${"afterLine" in line ? line.afterLine : ""}`}
                line={line}
              />
            ))}
          </div>
        ),
      )}
    </div>
  );
}
