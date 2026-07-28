/**
 * The tree, as thin a shell over `fileTreeModel` and `fileTreeKeys` as it can be.
 *
 * FE-21. Everything here is DOM: attributes, focus and event plumbing. Which
 * rows exist, what level they sit at and what a key means are all decided in
 * the two modules beside this one, because jsdom is a poor place to assert any
 * of it and a good place to assert none of it.
 *
 * The one decision that has to live here is the roving tab stop, because it is
 * a statement about the document rather than about the tree: exactly one row
 * carries `tabIndex=0`, so Tab lands in the tree once and the arrow keys take
 * over. Every control inside a row is out of the sequential order, and Tab on
 * a focused row steps into that row's controls instead.
 */

import { Button, Checkbox } from "@sapphirus/ui";
import {
  Binary,
  ChevronDown,
  ChevronRight,
  FileCode2,
  FileWarning,
  Folder,
  FolderOpen,
  LoaderCircle,
  Plus,
} from "lucide-react";
import { useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import type { WorkspaceTreeEntry } from "../../lib/hostClient";
import { treeKeyIntent } from "./fileTreeKeys";
import {
  entryName,
  formatBytes,
  resolveActivePath,
  type EntryRow,
  type FileTreeRow,
} from "./fileTreeModel";

export interface FileTreeProps {
  readonly label: string;
  readonly onLoadMore: (relativeDirectory: string, cursor: string) => void;
  readonly onReadFile: (relativePath: string) => void;
  readonly onSelectionChange: (relativePath: string, selected: boolean) => void;
  /**
   * Directional, rather than "toggle this directory".
   *
   * A filter opens whatever it must to reach a match, so a row can be shown
   * open while the caller's expanded set does not hold it. A toggle would ask
   * that set which way to go, read "closed", and record an expansion for a row
   * the user just asked to close -- invisible while the filter holds the row
   * open, and waiting for them the moment they cleared it. Every caller here
   * already knows which way it meant, so it says so.
   */
  readonly onSetDirectoryExpanded: (entry: WorkspaceTreeEntry, expanded: boolean) => void;
  readonly previewPath: string | null;
  readonly rows: readonly FileTreeRow[];
  readonly selectedPaths: ReadonlySet<string>;
  readonly selectionLimitReached: boolean;
}

function EntryIcon({ row }: { readonly row: EntryRow }) {
  if (row.entry.kind === "directory") {
    return row.isExpanded
      ? <FolderOpen aria-hidden="true" size={16} />
      : <Folder aria-hidden="true" size={16} />;
  }
  if (row.entry.kind === "binary_file") {
    return <Binary aria-hidden="true" size={16} />;
  }
  if (row.entry.kind === "blocked") {
    return <FileWarning aria-hidden="true" size={16} />;
  }
  return <FileCode2 aria-hidden="true" size={16} />;
}

export function FileTree({
  label,
  onLoadMore,
  onReadFile,
  onSelectionChange,
  onSetDirectoryExpanded,
  previewPath,
  rows,
  selectedPaths,
  selectionLimitReached,
}: FileTreeProps) {
  const [activePath, setActivePath] = useState<string | null>(null);
  const rowNodes = useRef(new Map<string, HTMLDivElement>());
  const pendingFocus = useRef<string | null>(null);
  const activeKey = resolveActivePath(rows, activePath);

  // The move and the focus are one action to the user, so they happen in one
  // frame: a passive effect would let the browser paint the new tab stop
  // before the caret got there.
  useLayoutEffect(() => {
    const target = pendingFocus.current;
    if (target === null) {
      return;
    }
    pendingFocus.current = null;
    rowNodes.current.get(target)?.focus();
  });

  function moveTo(path: string): void {
    pendingFocus.current = path;
    setActivePath(path);
  }

  function activate(row: FileTreeRow): void {
    if (row.kind === "load_more") {
      onLoadMore(row.directory, row.cursor);
      return;
    }
    if (row.isExpandable) {
      onSetDirectoryExpanded(row.entry, !row.isExpanded);
      return;
    }
    if (row.isSelectable) {
      onReadFile(row.entry.relativePath);
    }
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>, row: FileTreeRow): void {
    // Keys typed into a row's own controls belong to those controls. The
    // checkbox already stops Space before this sees it, but nothing stops the
    // arrow keys: without this, ArrowDown from inside the checkbox would move
    // the tree's tab stop and drag focus out of the control the user is
    // standing in, mid-selection.
    if (event.target !== event.currentTarget) {
      return;
    }
    const intent = treeKeyIntent(event.key, rows, row.key, event);
    if (intent === null) {
      return;
    }
    if (intent.kind === "enter_row") {
      const control = event.currentTarget.querySelector<HTMLElement>("input:not([disabled])");
      if (control === null) {
        // Nothing to step into, so Tab stays the browser's: it leaves the tree.
        return;
      }
      control.focus();
      event.preventDefault();
      return;
    }
    if (intent.kind === "focus") {
      moveTo(intent.path);
    } else if (intent.kind === "activate") {
      activate(row);
    } else if (row.kind === "entry") {
      onSetDirectoryExpanded(row.entry, intent.kind === "expand");
    }
    event.preventDefault();
  }

  return (
    <div aria-label={label} className="workspace-tree__list files-tree" role="tree">
      {rows.map((row) => (
        <div
          aria-busy={row.kind === "entry" && row.isLoading ? true : undefined}
          aria-expanded={row.kind === "entry" && row.isExpandable ? row.isExpanded : undefined}
          aria-level={row.level}
          aria-posinset={row.posInSet}
          aria-setsize={row.setSize}
          className={
            `workspace-tree__row files-tree__row${
              row.kind === "entry" && !row.matchesFilter ? " files-tree__row--context" : ""
            }`
          }
          key={row.key}
          onFocus={() => setActivePath(row.key)}
          onKeyDown={(event) => handleKeyDown(event, row)}
          ref={(node) => {
            if (node === null) {
              rowNodes.current.delete(row.key);
            } else {
              rowNodes.current.set(row.key, node);
            }
          }}
          role="treeitem"
          /*
           * Required by the role, and it is not merely a formality here: a
           * `treeitem` without it leaves a screen reader unable to say whether
           * the row the user is on is the chosen one. This tree has a single
           * roving tab stop, so "selected" is the row that carries it -- the
           * same fact `tabIndex` below encodes for the keyboard.
           */
          aria-selected={row.key === activeKey}
          style={{ paddingInlineStart: `${(row.level - 1) * 16 + 6}px` }}
          tabIndex={row.key === activeKey ? 0 : -1}
        >
          {row.kind === "load_more" ? (
            <Button
              className="workspace-tree__entry files-tree__more"
              excludeFromTabOrder
              onPress={() => onLoadMore(row.directory, row.cursor)}
              variant="quiet"
            >
              <Plus aria-hidden="true" size={15} />
              <span>Load more in {row.directoryName}</span>
            </Button>
          ) : row.entry.kind === "directory" ? (
            <Button
              className="workspace-tree__entry workspace-tree__entry--directory"
              excludeFromTabOrder
              onPress={() => onSetDirectoryExpanded(row.entry, !row.isExpanded)}
              variant="quiet"
            >
              {row.isLoading ? (
                <LoaderCircle aria-hidden="true" className="spin" size={15} />
              ) : row.isExpandable && row.isExpanded ? (
                <ChevronDown aria-hidden="true" size={15} />
              ) : (
                <ChevronRight aria-hidden="true" size={15} />
              )}
              <EntryIcon row={row} />
              <span>{entryName(row.entry.relativePath)}</span>
              {row.isLoading ? <small>Loading…</small> : null}
            </Button>
          ) : (
            <>
              <Checkbox
                aria-label={`Include ${row.entry.relativePath} in context`}
                className="workspace-tree__checkbox"
                excludeFromTabOrder
                isDisabled={
                  !row.isSelectable
                  || (selectionLimitReached && !selectedPaths.has(row.entry.relativePath))
                }
                isSelected={row.isSelectable && selectedPaths.has(row.entry.relativePath)}
                onChange={(selected) => onSelectionChange(row.entry.relativePath, selected)}
              >
                <span className="sr-only">Include in context</span>
              </Checkbox>
              <Button
                {...(previewPath === row.entry.relativePath
                  ? { "aria-current": "true" as const }
                  : {})}
                className="workspace-tree__entry workspace-tree__entry--file"
                excludeFromTabOrder
                isDisabled={!row.isSelectable}
                onPress={() => onReadFile(row.entry.relativePath)}
                variant="quiet"
              >
                <EntryIcon row={row} />
                <span>{entryName(row.entry.relativePath)}</span>
                <small>
                  {row.isSelectable ? formatBytes(row.entry.sizeBytes) : "Unavailable"}
                </small>
              </Button>
            </>
          )}
        </div>
      ))}
    </div>
  );
}
