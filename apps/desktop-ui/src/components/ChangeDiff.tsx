import { useMemo, useState } from "react";
import { computeLineDiff, type DiffLine } from "../lib/lineDiff";

export interface ChangeDiffProps {
  readonly relativePath: string;
  readonly beforeContent: string | null;
  readonly afterContent: string | null;
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

export function ChangeDiff({ relativePath, beforeContent, afterContent }: ChangeDiffProps) {
  const diff = useMemo(
    () => computeLineDiff(beforeContent ?? "", afterContent ?? ""),
    [beforeContent, afterContent],
  );
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(new Set());

  return (
    <div aria-label={`Changes to ${relativePath}`} className="diff-view" role="region">
      {diff.segments.map((segment, index) =>
        segment.kind === "collapsed" && !expanded.has(index) ? (
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
