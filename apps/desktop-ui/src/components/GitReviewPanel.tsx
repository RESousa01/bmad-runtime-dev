import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import type {
  GitComparison,
  GitFileComparisonProjection,
  GitFileState,
  GitStatusEntryProjection,
  GitStatusProjection,
} from "../lib/hostClient";
import {
  ChangeDiff,
  ChangeMagnitude,
  diffCounts,
  FilePathLabel,
  SplitChangeDiff,
} from "./ChangeDiff";
import { OpenCodeIcon } from "./OpenCodeIcon";

export interface GitReviewSource {
  readStatus(): Promise<GitStatusProjection>;
  readDiff(
    relativePath: string,
    comparison: GitComparison,
  ): Promise<GitFileComparisonProjection>;
}

export interface GitReviewPanelProps {
  readonly availabilityMessage?: string | undefined;
  readonly secondaryAction?: ReactNode | undefined;
  readonly source: GitReviewSource | null;
}

interface ComparisonOption {
  readonly id: GitComparison;
  readonly label: string;
  readonly description: string;
}

const comparisonOptions: readonly ComparisonOption[] = [
  {
    id: "head_to_worktree",
    label: "All",
    description: "HEAD to working tree",
  },
  {
    id: "head_to_index",
    label: "Staged",
    description: "HEAD to index",
  },
  {
    id: "index_to_worktree",
    label: "Unstaged",
    description: "Index to working tree",
  },
];

const stateCodes: Record<GitFileState, string> = {
  modified: "M",
  added: "A",
  deleted: "D",
  untracked: "?",
  renamed: "R",
};

function entriesForComparison(
  entries: readonly GitStatusEntryProjection[],
  comparison: GitComparison,
): readonly GitStatusEntryProjection[] {
  if (comparison === "head_to_index") {
    return entries.filter((entry) => entry.indexState !== null);
  }
  if (comparison === "index_to_worktree") {
    return entries.filter((entry) => entry.worktreeState !== null);
  }
  return entries;
}

function statusLabels(entry: GitStatusEntryProjection): string[] {
  const labels: string[] = [];
  if (entry.indexState !== null) {
    labels.push(`staged ${entry.indexState}`);
  }
  if (entry.worktreeState !== null) {
    labels.push(entry.worktreeState === "untracked"
      ? "untracked"
      : `unstaged ${entry.worktreeState}`);
  }
  return labels;
}

function FileStateBadge({
  kind,
  state,
}: {
  readonly kind: "staged" | "unstaged";
  readonly state: GitFileState;
}) {
  return (
    <span
      aria-label={`${kind} ${state}`}
      className={`git-review__state git-review__state--${kind}`}
      title={`${kind} ${state}`}
    >
      {stateCodes[state]}
    </span>
  );
}

interface ReviewTreeDirectory {
  readonly directories: Map<string, ReviewTreeDirectory>;
  readonly files: GitStatusEntryProjection[];
  readonly name: string;
  readonly relativePath: string;
}

function buildReviewTree(entries: readonly GitStatusEntryProjection[]): ReviewTreeDirectory {
  const root: ReviewTreeDirectory = {
    directories: new Map(),
    files: [],
    name: "",
    relativePath: "",
  };
  for (const entry of entries) {
    const segments = entry.relativePath.split("/");
    let directory = root;
    for (const segment of segments.slice(0, -1)) {
      const relativePath = directory.relativePath
        ? `${directory.relativePath}/${segment}`
        : segment;
      let child = directory.directories.get(segment);
      if (!child) {
        child = { directories: new Map(), files: [], name: segment, relativePath };
        directory.directories.set(segment, child);
      }
      directory = child;
    }
    directory.files.push(entry);
  }
  return root;
}

function ReviewTree({
  activePath,
  directory,
  depth = 0,
  onSelect,
}: {
  readonly activePath: string | null;
  readonly depth?: number;
  readonly directory: ReviewTreeDirectory;
  readonly onSelect: (relativePath: string) => void;
}) {
  const directories = [...directory.directories.values()]
    .sort((left, right) => left.name.localeCompare(right.name));
  const files = [...directory.files]
    .sort((left, right) => left.relativePath.localeCompare(right.relativePath));
  return (
    <>
      {directories.map((child) => (
        <div className="git-review__tree-group" key={child.relativePath}>
          <div className="git-review__folder" style={{ "--tree-depth": depth } as CSSProperties}>
            <OpenCodeIcon name="chevron-down" size={14} />
            <OpenCodeIcon name="folder" size={14} />
            <span>{child.name}</span>
          </div>
          <ReviewTree
            activePath={activePath}
            depth={depth + 1}
            directory={child}
            onSelect={onSelect}
          />
        </div>
      ))}
      {files.map((entry) => {
        const name = entry.relativePath.split("/").at(-1) ?? entry.relativePath;
        return (
          <div key={entry.relativePath} role="listitem">
            <button
              aria-label={`${entry.relativePath}, ${statusLabels(entry).join(", ")}`}
              aria-pressed={entry.relativePath === activePath}
              className="git-review__file"
              onClick={() => onSelect(entry.relativePath)}
              style={{ "--tree-depth": depth } as CSSProperties}
              title={entry.relativePath}
              type="button"
            >
              <span className="git-review__file-name">{name}</span>
              <span className="git-review__states">
                {entry.indexState === null ? null : <FileStateBadge kind="staged" state={entry.indexState} />}
                {entry.worktreeState === null ? null : <FileStateBadge kind="unstaged" state={entry.worktreeState} />}
              </span>
            </button>
          </div>
        );
      })}
    </>
  );
}

export function GitReviewPanel({
  availabilityMessage = "Git review is unavailable for this workspace.",
  secondaryAction,
  source,
}: GitReviewPanelProps) {
  const headingId = useId();
  const [comparison, setComparison] = useState<GitComparison>("head_to_worktree");
  const [diffStyle, setDiffStyle] = useState<"split" | "unified">("split");
  const [expandMode, setExpandMode] = useState<"collapse" | "expand">("collapse");
  const [filter, setFilter] = useState("");
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [refreshEpoch, setRefreshEpoch] = useState(0);
  const [status, setStatus] = useState<
    | { kind: "loading" }
    | { kind: "ready"; value: GitStatusProjection; revision: number }
    | { kind: "error" }
  >({ kind: "loading" });
  const [diff, setDiff] = useState<
    | { kind: "idle" }
    | { kind: "loading"; relativePath: string }
    | { kind: "ready"; value: GitFileComparisonProjection }
    | { kind: "error"; relativePath: string }
  >({ kind: "idle" });
  const statusGeneration = useRef(0);
  const diffGeneration = useRef(0);

  // `loading` cannot be derived during render: it is the state of an
  // outstanding host read, and the read is started by this effect. Announcing
  // it synchronously is what the rule's own guidance calls subscribing to an
  // external system, and the generation guard below is what actually keeps the
  // result correct. The alternative the rule implies — leaving the previous
  // workspace's status on screen until the new one resolves — would show one
  // repository's file list under another repository's name.
  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    const generation = ++statusGeneration.current;
    ++diffGeneration.current;
    if (source === null) {
      setStatus({ kind: "error" });
      setDiff({ kind: "idle" });
      return;
    }
    setStatus({ kind: "loading" });
    setDiff({ kind: "idle" });
    void source.readStatus().then((value) => {
      if (generation !== statusGeneration.current) return;
      setStatus({ kind: "ready", value, revision: generation });
    }).catch(() => {
      if (generation !== statusGeneration.current) return;
      setStatus({ kind: "error" });
    });
  }, [refreshEpoch, source]);
  /* eslint-enable react-hooks/set-state-in-effect */

  const comparisonEntries = useMemo(
    () => status.kind === "ready"
      ? entriesForComparison(status.value.entries, comparison)
      : [],
    [comparison, status],
  );
  const visibleEntries = useMemo(() => {
    const query = filter.trim().toLocaleLowerCase();
    return query.length === 0
      ? comparisonEntries
      : comparisonEntries.filter((entry) => entry.relativePath.toLocaleLowerCase().includes(query));
  }, [comparisonEntries, filter]);
  const reviewTree = useMemo(() => buildReviewTree(visibleEntries), [visibleEntries]);
  const activePath = visibleEntries.some((entry) => entry.relativePath === selectedPath)
    ? selectedPath
    : visibleEntries[0]?.relativePath ?? null;
  const statusRevision = status.kind === "ready" ? status.revision : 0;

  /* eslint-disable react-hooks/set-state-in-effect -- Same shape as the status
     read above: the diff belongs to whichever path is active, the read is
     asynchronous, and showing the previous file's diff under the new file's
     name while it loads is the failure this synchronous transition prevents. */
  useEffect(() => {
    const generation = ++diffGeneration.current;
    if (source === null || activePath === null || status.kind !== "ready") {
      setDiff({ kind: "idle" });
      return;
    }
    setDiff({ kind: "loading", relativePath: activePath });
    void source.readDiff(activePath, comparison).then((value) => {
      if (generation !== diffGeneration.current) return;
      if (value.relativePath !== activePath || value.comparison !== comparison) {
        setDiff({ kind: "error", relativePath: activePath });
        return;
      }
      setDiff({ kind: "ready", value });
    }).catch(() => {
      if (generation !== diffGeneration.current) return;
      setDiff({ kind: "error", relativePath: activePath });
    });
  }, [activePath, comparison, source, status.kind, statusRevision]);
  /* eslint-enable react-hooks/set-state-in-effect */

  const activeComparison = comparisonOptions.find((option) => option.id === comparison)
    ?? comparisonOptions[0]!;
  const counts = diff.kind === "ready"
    ? diffCounts(diff.value.beforeContent, diff.value.afterContent)
    : null;
  const activeIndex = activePath === null
    ? -1
    : visibleEntries.findIndex((entry) => entry.relativePath === activePath);
  const previousPath = activeIndex > 0 ? visibleEntries[activeIndex - 1]?.relativePath : undefined;
  const nextPath = activeIndex >= 0 && activeIndex < visibleEntries.length - 1
    ? visibleEntries[activeIndex + 1]?.relativePath
    : undefined;

  return (
    <section
      aria-labelledby={headingId}
      className="git-review"
      data-component="session-review-v2"
      data-source="opencode"
    >
      <h2 className="sr-only" id={headingId}>Working tree</h2>
      {status.kind === "ready" || !secondaryAction ? null : (
        <div className="git-review__secondary-fallback">{secondaryAction}</div>
      )}
      {status.kind === "loading" ? (
        <div aria-live="polite" className="git-review__empty">Reading Git status…</div>
      ) : status.kind === "error" ? (
        <div className="git-review__empty" role="status">
          <OpenCodeIcon name="review" size={20} />
          <p>{source === null ? availabilityMessage : "Forge couldn't read Git status for this workspace."}</p>
        </div>
      ) : (
        <div className="git-review__window">
          <aside className="git-review__sidebar" data-slot="session-review-v2-sidebar">
            <header className="git-review__sidebar-header" data-slot="session-review-v2-sidebar-header">
              <label className="git-review__comparison-select" title={activeComparison.description}>
                <span className="sr-only">Git comparison</span>
                <select
                  aria-label="Git comparison"
                  onChange={(event) => setComparison(event.target.value as GitComparison)}
                  value={comparison}
                >
                  {comparisonOptions.map((option) => (
                    <option key={option.id} value={option.id}>
                      {option.id === "head_to_worktree" ? "Git changes" : `${option.label} changes`}
                    </option>
                  ))}
                </select>
                <OpenCodeIcon name="chevron-down" size={14} />
              </label>
              <span className="sr-only">{status.value.branch ?? "Detached HEAD"}</span>
              <span className="git-review__summary">
                {counts ? <><b>+{counts.added}</b><i>−{counts.removed}</i></> : `${comparisonEntries.length} files`}
              </span>
              {secondaryAction}
              <button
                aria-label="Refresh Git changes"
                className="git-review__refresh"
                disabled={source === null}
                onClick={() => setRefreshEpoch((current) => current + 1)}
                title={`Refresh ${status.value.branch ?? "Git changes"}`}
                type="button"
              >
                <OpenCodeIcon name="reset" size={16} />
              </button>
            </header>
            <label className="git-review__filter">
              <OpenCodeIcon name="magnifying-glass" size={14} />
              <span className="sr-only">Filter files</span>
              <input
                aria-label="Filter files"
                onChange={(event) => setFilter(event.target.value)}
                placeholder="Filter files"
                type="search"
                value={filter}
              />
              {filter ? (
                <button aria-label="Clear file filter" onClick={() => setFilter("")} type="button">
                  <OpenCodeIcon name="close" size={14} />
                </button>
              ) : null}
            </label>
            {status.value.truncated ? (
              <p className="git-review__notice">Showing the first 500 changed files.</p>
            ) : null}
            <div
              aria-label={`${activeComparison.label} changed files`}
              className="git-review__files"
              data-slot="session-review-v2-sidebar-tree"
              role="list"
            >
              {visibleEntries.length === 0 ? (
                <div className="git-review__tree-empty">
                  {filter ? "No matching files." : `No ${activeComparison.label.toLocaleLowerCase()} changes.`}
                </div>
              ) : (
                <ReviewTree
                  activePath={activePath}
                  directory={reviewTree}
                  onSelect={setSelectedPath}
                />
              )}
            </div>
          </aside>

          <div className="git-review__main" data-slot="session-review-v2-main">
            <div className="git-review__toolbar" data-slot="session-review-v2-toolbar">
              <div className="git-review__toolbar-group">
                <button aria-label="Previous file" disabled={!previousPath} onClick={() => previousPath && setSelectedPath(previousPath)} type="button">
                  <OpenCodeIcon name="arrow-left" size={16} />
                </button>
                <button aria-label="Next file" disabled={!nextPath} onClick={() => nextPath && setSelectedPath(nextPath)} type="button">
                  <OpenCodeIcon name="arrow-right" size={16} />
                </button>
              </div>
              <div className="git-review__toolbar-controls">
                <div aria-label="Line display" className="git-review__segmented" role="group">
                  <button aria-pressed={expandMode === "expand"} aria-label="Show all lines" onClick={() => setExpandMode("expand")} type="button">
                    <OpenCodeIcon name="expand" size={16} />
                  </button>
                  <button aria-pressed={expandMode === "collapse"} aria-label="Hide non-diff lines" onClick={() => setExpandMode("collapse")} type="button">
                    <OpenCodeIcon name="collapse" size={16} />
                  </button>
                </div>
                <div aria-label="Diff view" className="git-review__segmented" role="group">
                  <button aria-pressed={diffStyle === "unified"} aria-label="Unified diff" onClick={() => setDiffStyle("unified")} type="button">
                    <OpenCodeIcon name="unified" size={16} />
                  </button>
                  <button aria-pressed={diffStyle === "split"} aria-label="Split diff" onClick={() => setDiffStyle("split")} type="button">
                    <OpenCodeIcon name="split" size={16} />
                  </button>
                </div>
              </div>
            </div>
            <div className="git-review__diff">
              {diff.kind === "loading" ? (
                <div aria-live="polite" className="git-review__empty">Reading difference…</div>
              ) : diff.kind === "error" ? (
                <div className="git-review__empty" role="status">Forge couldn't read this Git difference.</div>
              ) : diff.kind === "ready" && counts !== null ? (
                <>
                  <header className="git-review__diff-header">
                    <FilePathLabel relativePath={diff.value.relativePath} />
                    <span className="git-review__stats" aria-label={`${counts.added} lines added and ${counts.removed} lines removed`}>
                      <span className="git-review__added">+{counts.added}</span>
                      <span className="git-review__removed">−{counts.removed}</span>
                      <ChangeMagnitude added={counts.added} removed={counts.removed} />
                    </span>
                  </header>
                  {diff.value.truncated ? (
                    <p className="git-review__notice">Large file preview truncated at 64 KiB per side.</p>
                  ) : null}
                  <div aria-label={`Changes to ${diff.value.relativePath}`} className="git-review__diff-region" role="region">
                    {diffStyle === "split" ? (
                      <SplitChangeDiff
                        afterContent={diff.value.afterContent}
                        beforeContent={diff.value.beforeContent}
                        relativePath={diff.value.relativePath}
                        showAllLines={expandMode === "expand"}
                      />
                    ) : (
                      <ChangeDiff
                        afterContent={diff.value.afterContent}
                        beforeContent={diff.value.beforeContent}
                        relativePath={diff.value.relativePath}
                        showAllLines={expandMode === "expand"}
                      />
                    )}
                  </div>
                </>
              ) : (
                <div className="git-review__empty">Select a changed file.</div>
              )}
            </div>
          </div>
        </div>
      )}
    </section>
  );
}

export function ReviewPanel({
  availabilityMessage,
  governedReview,
  proposalAttention = false,
  source,
}: {
  readonly availabilityMessage?: string;
  readonly governedReview: ReactNode;
  readonly proposalAttention?: boolean;
  readonly source: GitReviewSource | null;
}) {
  const [view, setView] = useState<"git" | "proposals">("git");
  const previousProposalAttention = useRef(false);
  useEffect(() => {
    if (proposalAttention && !previousProposalAttention.current) {
      setView("proposals");
    }
    previousProposalAttention.current = proposalAttention;
  }, [proposalAttention]);
  return (
    <div className="review-panel">
      {view === "git" ? (
        <GitReviewPanel
          availabilityMessage={availabilityMessage}
          secondaryAction={(
            <button
              aria-label="Agent proposals"
              className="git-review__proposals"
              onClick={() => setView("proposals")}
              title="BMAD agent proposals"
              type="button"
            >
              <OpenCodeIcon name="edit" size={14} />
              {proposalAttention ? <span aria-label="New proposal" className="git-review__attention" /> : null}
            </button>
          )}
          source={source}
        />
      ) : (
        <div className="review-panel__governed-shell">
          <header className="review-panel__governed-header">
            <button aria-label="Back to Git changes" onClick={() => setView("git")} title="Back to Git changes" type="button">
              <OpenCodeIcon name="arrow-left" size={16} />
            </button>
            <strong>Agent proposals</strong>
          </header>
          <div className="review-panel__governed">{governedReview}</div>
        </div>
      )}
    </div>
  );
}
