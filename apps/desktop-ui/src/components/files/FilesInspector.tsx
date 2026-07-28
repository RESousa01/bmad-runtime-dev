/**
 * The Files tab: a filter, what the filter is doing, and the tree.
 *
 * FE-21. The filter lives here rather than in the explorer because it is a
 * view of the pages already in hand -- it starts no host request and survives
 * nothing. What it must not do is look like an empty workspace: a filter with
 * no hits and a folder with no readable files are the same blank box, and the
 * difference decides whether the user's next move is to clear the filter or to
 * open a different folder. The status line says which one they are looking at,
 * every time, including when it found everything.
 */

import { Button } from "@sapphirus/ui";
import { LoaderCircle, RefreshCw, Search, X } from "lucide-react";
import { useMemo, useState } from "react";
import type { WorkspaceTreeEntry } from "../../lib/hostClient";
import { FileTree } from "./FileTree";
import {
  buildVisibleTree,
  describeFilter,
  type DirectoryPage,
} from "./fileTreeModel";
import "./files-inspector.css";

export interface FilesInspectorProps {
  readonly childPages: ReadonlyMap<string, DirectoryPage>;
  readonly expandedPaths: ReadonlySet<string>;
  readonly isRootLoading: boolean;
  readonly loadingDirectories: ReadonlySet<string>;
  readonly onLoadMore: (relativeDirectory: string, cursor: string) => void;
  readonly onReadFile: (relativePath: string) => void;
  readonly onRefresh: () => void;
  readonly onSelectionChange: (relativePath: string, selected: boolean) => void;
  readonly onToggleDirectory: (entry: WorkspaceTreeEntry) => void;
  readonly previewPath: string | null;
  readonly refreshDisabled: boolean;
  readonly rootPage: DirectoryPage | null;
  readonly selectedPaths: ReadonlySet<string>;
  readonly selectionLimitReached: boolean;
}

export function FilesInspector({
  childPages,
  expandedPaths,
  isRootLoading,
  loadingDirectories,
  onLoadMore,
  onReadFile,
  onRefresh,
  onSelectionChange,
  onToggleDirectory,
  previewPath,
  refreshDisabled,
  rootPage,
  selectedPaths,
  selectionLimitReached,
}: FilesInspectorProps) {
  const [filter, setFilter] = useState("");
  const tree = useMemo(
    () => buildVisibleTree({ childPages, expandedPaths, filter, loadingDirectories, rootPage }),
    [childPages, expandedPaths, filter, loadingDirectories, rootPage],
  );
  const status = describeFilter(filter, tree.matchCount, tree.loadedCount);

  return (
    <>
      <div className="workspace-panel-heading">
        <div>
          <strong>Workspace files</strong>
          <span>UTF-8 text only</span>
        </div>
        <Button
          aria-label="Refresh workspace projections"
          isDisabled={refreshDisabled}
          onPress={onRefresh}
          size="icon"
          variant="quiet"
        >
          <RefreshCw aria-hidden="true" size={16} />
        </Button>
      </div>

      <div className="files-inspector__filter">
        <label htmlFor="workspace-file-filter">Filter loaded files</label>
        <div>
          <Search aria-hidden="true" size={15} />
          <input
            autoComplete="off"
            id="workspace-file-filter"
            onChange={(event) => setFilter(event.target.value)}
            placeholder="Name contains…"
            type="text"
            value={filter}
          />
          {filter.length > 0 ? (
            <Button
              aria-label="Clear the file filter"
              onPress={() => setFilter("")}
              size="icon"
              variant="quiet"
            >
              <X aria-hidden="true" size={14} />
            </Button>
          ) : null}
        </div>
      </div>

      {/*
        Mounted whether or not it has anything to say. A live region that
        appears at the same moment as its first message is announced
        inconsistently -- the region has to already be there for the change to
        register as a change.
      */}
      <p className="files-inspector__status" role="status">{status}</p>

      <div aria-busy={isRootLoading} className="workspace-tree">
        {tree.rows.length > 0 ? (
          <FileTree
            label="Workspace files"
            onLoadMore={onLoadMore}
            onReadFile={onReadFile}
            onSelectionChange={onSelectionChange}
            onToggleDirectory={onToggleDirectory}
            previewPath={previewPath}
            rows={tree.rows}
            selectedPaths={selectedPaths}
            selectionLimitReached={selectionLimitReached}
          />
        ) : tree.isFiltered ? (
          <p className="workspace-inline-empty">
            Every loaded entry is hidden by this filter. Clear it to see the workspace again.
          </p>
        ) : rootPage === null && isRootLoading ? (
          <div className="workspace-loading-state" role="status">
            <LoaderCircle aria-hidden="true" className="spin" size={18} />
            Loading a bounded file page…
          </div>
        ) : (
          <p className="workspace-inline-empty">No visible entries were projected.</p>
        )}
        {rootPage?.nextCursor != null ? (
          <Button
            className="workspace-tree__root-more"
            isDisabled={isRootLoading}
            onPress={() => onLoadMore(".", rootPage.nextCursor ?? "")}
            size="small"
            variant="secondary"
          >
            Load more files
          </Button>
        ) : null}
      </div>
    </>
  );
}
