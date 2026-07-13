import { Button, Checkbox, Tab, TabList, TabPanel, Tabs } from "@sapphirus/ui";
import {
  AlertTriangle,
  Binary,
  ChevronDown,
  ChevronRight,
  FileCode2,
  FileSearch,
  FileWarning,
  Folder,
  FolderOpen,
  ListChecks,
  LoaderCircle,
  RefreshCw,
  Search,
  ShieldCheck,
  Workflow,
} from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import type {
  BmadAssetKind,
  BmadScanProjection,
  ContextPreviewProjection,
  WorkspaceSearchMatch,
  WorkspaceTextProjection,
  WorkspaceTreeEntry,
} from "../lib/hostClient";
import { getSafeHostMessage, workspaceReadLimits } from "../lib/hostClient";
import type {
  ReadonlyWorkspaceSource,
  WorkspaceProjectionProvenance,
} from "../lib/workspaceReadSource";

type ExplorerSection = "files" | "search" | "bmad";

interface DirectoryPage {
  entries: WorkspaceTreeEntry[];
  nextCursor: string | null;
}

interface TreeRowsProps {
  childPages: ReadonlyMap<string, DirectoryPage>;
  expandedPaths: ReadonlySet<string>;
  loadingDirectories: ReadonlySet<string>;
  onLoadMore: (relativeDirectory: string, cursor: string) => void;
  onReadFile: (relativePath: string) => void;
  onSelectionChange: (relativePath: string, selected: boolean) => void;
  onToggleDirectory: (entry: WorkspaceTreeEntry) => void;
  previewPath: string | null;
  selectionLimitReached: boolean;
  selectedPaths: ReadonlySet<string>;
  entries: WorkspaceTreeEntry[];
  depth?: number;
}

const bmadAssetLabels: Record<BmadAssetKind, string> = {
  method_configuration: "Method configuration",
  agent: "Agent",
  workflow: "Workflow",
  builder_build_draft: "Builder Build draft",
  builder_edit_draft: "Builder Edit draft",
  builder_analyze_draft: "Builder Analyze draft",
};

function filename(relativePath: string): string {
  return relativePath.split("/").at(-1) ?? relativePath;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function TreeRows({
  childPages,
  depth = 0,
  entries,
  expandedPaths,
  loadingDirectories,
  onLoadMore,
  onReadFile,
  onSelectionChange,
  onToggleDirectory,
  previewPath,
  selectionLimitReached,
  selectedPaths,
}: TreeRowsProps) {
  return (
    <ul className="workspace-tree__list" data-depth={depth}>
      {entries.map((entry) => {
        const isDirectory = entry.kind === "directory";
        const isExpanded = isDirectory && expandedPaths.has(entry.relativePath);
        const isLoading = loadingDirectories.has(entry.relativePath);
        const page = childPages.get(entry.relativePath);
        const canSelect = entry.kind === "text_file";
        return (
          <li className="workspace-tree__item" key={entry.relativePath}>
            <div className="workspace-tree__row" style={{ paddingInlineStart: `${depth * 16 + 6}px` }}>
              {isDirectory ? (
                <Button
                  aria-expanded={isExpanded}
                  className="workspace-tree__entry workspace-tree__entry--directory"
                  isDisabled={isLoading}
                  onPress={() => onToggleDirectory(entry)}
                  variant="quiet"
                >
                  {isLoading ? (
                    <LoaderCircle aria-hidden="true" className="spin" size={15} />
                  ) : isExpanded ? (
                    <ChevronDown aria-hidden="true" size={15} />
                  ) : (
                    <ChevronRight aria-hidden="true" size={15} />
                  )}
                  {isExpanded ? (
                    <FolderOpen aria-hidden="true" size={16} />
                  ) : (
                    <Folder aria-hidden="true" size={16} />
                  )}
                  <span>{filename(entry.relativePath)}</span>
                </Button>
              ) : (
                <>
                  <Checkbox
                    aria-label={`Include ${entry.relativePath} in context`}
                    className="workspace-tree__checkbox"
                    isDisabled={
                      !canSelect
                      || (selectionLimitReached && !selectedPaths.has(entry.relativePath))
                    }
                    isSelected={canSelect && selectedPaths.has(entry.relativePath)}
                    onChange={(selected) => onSelectionChange(entry.relativePath, selected)}
                  >
                    <span className="sr-only">Include in context</span>
                  </Checkbox>
                  <Button
                    {...(previewPath === entry.relativePath ? { "aria-current": "true" as const } : {})}
                    className="workspace-tree__entry workspace-tree__entry--file"
                    isDisabled={!canSelect}
                    onPress={() => onReadFile(entry.relativePath)}
                    variant="quiet"
                  >
                    {entry.kind === "binary_file" ? (
                      <Binary aria-hidden="true" size={16} />
                    ) : entry.kind === "blocked" ? (
                      <FileWarning aria-hidden="true" size={16} />
                    ) : (
                      <FileCode2 aria-hidden="true" size={16} />
                    )}
                    <span>{filename(entry.relativePath)}</span>
                    <small>{canSelect ? formatBytes(entry.sizeBytes) : "Unavailable"}</small>
                  </Button>
                </>
              )}
            </div>
            {isDirectory && isExpanded && page ? (
              <>
                <TreeRows
                  childPages={childPages}
                  depth={depth + 1}
                  entries={page.entries}
                  expandedPaths={expandedPaths}
                  loadingDirectories={loadingDirectories}
                  onLoadMore={onLoadMore}
                  onReadFile={onReadFile}
                  onSelectionChange={onSelectionChange}
                  onToggleDirectory={onToggleDirectory}
                  previewPath={previewPath}
                  selectionLimitReached={selectionLimitReached}
                  selectedPaths={selectedPaths}
                />
                {page.nextCursor ? (
                  <Button
                    className="workspace-tree__load-more"
                    isDisabled={isLoading}
                    onPress={() => onLoadMore(entry.relativePath, page.nextCursor!)}
                    size="small"
                    variant="quiet"
                  >
                    Load more in {filename(entry.relativePath)}
                  </Button>
                ) : null}
              </>
            ) : null}
          </li>
        );
      })}
    </ul>
  );
}

export interface WorkspaceExplorerProps {
  availabilityMessage: string;
  isInert?: boolean;
  onContextReview: (
    preview: ContextPreviewProjection,
    provenance: WorkspaceProjectionProvenance,
  ) => void;
  source: ReadonlyWorkspaceSource | null;
  workspaceName: string;
}

export function WorkspaceExplorer({
  availabilityMessage,
  isInert = false,
  onContextReview,
  source,
  workspaceName,
}: WorkspaceExplorerProps) {
  const [activeSection, setActiveSection] = useState<ExplorerSection>("files");
  const [bmad, setBmad] = useState<BmadScanProjection | null>(null);
  const [bmadBusy, setBmadBusy] = useState(false);
  const [childPages, setChildPages] = useState<ReadonlyMap<string, DirectoryPage>>(new Map());
  const [contextBusy, setContextBusy] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [expandedPaths, setExpandedPaths] = useState<ReadonlySet<string>>(new Set());
  const [loadingDirectories, setLoadingDirectories] = useState<ReadonlySet<string>>(new Set());
  const [preview, setPreview] = useState<WorkspaceTextProjection | null>(null);
  const [previewBusy, setPreviewBusy] = useState(false);
  const [rootPage, setRootPage] = useState<DirectoryPage | null>(null);
  const [searchBusy, setSearchBusy] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<WorkspaceSearchMatch[] | null>(null);
  const [selectedPaths, setSelectedPaths] = useState<ReadonlySet<string>>(new Set());
  const contextRequestActive = useRef(false);
  const directoryRequestTokens = useRef(new Map<string, symbol>());
  const latestContextRequest = useRef(0);
  const latestPreviewRequest = useRef(0);
  const latestSearchRequest = useRef(0);
  const refreshRequestToken = useRef<symbol | null>(null);
  const sourceRef = useRef(source);
  const traversalEpoch = useRef(0);
  sourceRef.current = source;

  function safeReadError(error: unknown): string {
    if (source?.provenance === "browser_demo") {
      return "The browser demo could not load that fixture. No local workspace was accessed.";
    }
    return getSafeHostMessage(error);
  }

  useEffect(() => {
    let current = true;
    const invalidateRequests = () => {
      current = false;
      traversalEpoch.current += 1;
      directoryRequestTokens.current.clear();
      refreshRequestToken.current = null;
      latestContextRequest.current += 1;
      latestPreviewRequest.current += 1;
      latestSearchRequest.current += 1;
      contextRequestActive.current = false;
    };
    traversalEpoch.current += 1;
    directoryRequestTokens.current.clear();
    refreshRequestToken.current = null;
    latestContextRequest.current += 1;
    latestPreviewRequest.current += 1;
    latestSearchRequest.current += 1;
    contextRequestActive.current = false;
    setBmadBusy(false);
    setContextBusy(false);
    setLoadingDirectories(new Set());
    setPreviewBusy(false);
    setSearchBusy(false);
    setRootPage(null);
    setChildPages(new Map());
    setExpandedPaths(new Set());
    setSelectedPaths(new Set());
    setPreview(null);
    setSearchResults(null);
    setErrorMessage(null);
    setBmad(null);
    if (!source) {
      return invalidateRequests;
    }

    setBmadBusy(true);
    void source.scanBmad()
      .then((projection) => {
        if (current) {
          setBmad(projection);
        }
      })
      .catch((error: unknown) => {
        if (current) {
          setErrorMessage(safeReadError(error));
        }
      })
      .finally(() => {
        if (current) {
          setBmadBusy(false);
        }
      });

    setLoadingDirectories(new Set(["."]));
    void source.listEntries(null, 100)
      .then((projection) => {
        if (current) {
          setRootPage({ entries: projection.entries, nextCursor: projection.nextCursor });
        }
      })
      .catch((error: unknown) => {
        if (current) {
          setErrorMessage(safeReadError(error));
        }
      })
      .finally(() => {
        if (current) {
          setLoadingDirectories(new Set());
        }
      });

    return invalidateRequests;
  }, [source]);

  async function refreshWorkspace() {
    if (!source || refreshRequestToken.current) {
      return;
    }
    const activeSource = source;
    const epoch = traversalEpoch.current + 1;
    traversalEpoch.current = epoch;
    directoryRequestTokens.current.clear();
    latestContextRequest.current += 1;
    latestPreviewRequest.current += 1;
    latestSearchRequest.current += 1;
    contextRequestActive.current = false;
    const refreshRequestId = Symbol("workspace-refresh");
    refreshRequestToken.current = refreshRequestId;
    setErrorMessage(null);
    setLoadingDirectories(new Set(["."]));
    setBmadBusy(true);
    setChildPages(new Map());
    setExpandedPaths(new Set());
    setPreview(null);
    setPreviewBusy(false);
    setRootPage(null);
    setSearchResults(null);
    setSearchBusy(false);
    setSelectedPaths(new Set());
    try {
      const [entriesResult, bmadResult] = await Promise.allSettled([
        activeSource.listEntries(null, 100),
        activeSource.scanBmad(),
      ]);
      if (sourceRef.current !== activeSource || traversalEpoch.current !== epoch) {
        return;
      }
      if (entriesResult.status === "fulfilled") {
        setRootPage({
          entries: entriesResult.value.entries,
          nextCursor: entriesResult.value.nextCursor,
        });
      } else {
        setErrorMessage(safeReadError(entriesResult.reason));
      }
      if (bmadResult.status === "fulfilled") {
        setBmad(bmadResult.value);
      } else {
        setErrorMessage(safeReadError(bmadResult.reason));
      }
    } finally {
      if (refreshRequestToken.current === refreshRequestId) {
        refreshRequestToken.current = null;
        if (sourceRef.current === activeSource && traversalEpoch.current === epoch) {
          setLoadingDirectories(new Set());
          setBmadBusy(false);
        }
      }
    }
  }

  function beginDirectoryRequest(relativeDirectory: string): symbol | null {
    if (directoryRequestTokens.current.has(relativeDirectory)) {
      return null;
    }
    const token = Symbol(relativeDirectory);
    directoryRequestTokens.current.set(relativeDirectory, token);
    setLoadingDirectories((current) => new Set(current).add(relativeDirectory));
    return token;
  }

  function finishDirectoryRequest(relativeDirectory: string, token: symbol): void {
    if (directoryRequestTokens.current.get(relativeDirectory) !== token) {
      return;
    }
    directoryRequestTokens.current.delete(relativeDirectory);
    setLoadingDirectories((current) => {
      const next = new Set(current);
      next.delete(relativeDirectory);
      return next;
    });
  }

  async function toggleDirectory(entry: WorkspaceTreeEntry) {
    if (!source || !entry.childCursor) {
      return;
    }
    if (expandedPaths.has(entry.relativePath)) {
      setExpandedPaths((current) => {
        const next = new Set(current);
        next.delete(entry.relativePath);
        return next;
      });
      return;
    }
    setExpandedPaths((current) => new Set(current).add(entry.relativePath));
    if (childPages.has(entry.relativePath)) {
      return;
    }
    const requestToken = beginDirectoryRequest(entry.relativePath);
    if (!requestToken) {
      return;
    }
    const activeSource = source;
    const epoch = traversalEpoch.current;
    try {
      const page = await activeSource.listEntries(entry.childCursor, 100);
      if (sourceRef.current !== activeSource || traversalEpoch.current !== epoch) {
        return;
      }
      setChildPages((current) => {
        const next = new Map(current);
        next.set(entry.relativePath, { entries: page.entries, nextCursor: page.nextCursor });
        return next;
      });
    } catch (error) {
      if (sourceRef.current === activeSource && traversalEpoch.current === epoch) {
        setExpandedPaths((current) => {
          const next = new Set(current);
          next.delete(entry.relativePath);
          return next;
        });
        setErrorMessage(safeReadError(error));
      }
    } finally {
      finishDirectoryRequest(entry.relativePath, requestToken);
    }
  }

  async function loadMore(relativeDirectory: string, cursor: string) {
    if (!source) {
      return;
    }
    const requestToken = beginDirectoryRequest(relativeDirectory);
    if (!requestToken) {
      return;
    }
    const activeSource = source;
    const epoch = traversalEpoch.current;
    try {
      const page = await activeSource.listEntries(cursor, 100);
      if (sourceRef.current !== activeSource || traversalEpoch.current !== epoch) {
        return;
      }
      if (relativeDirectory === ".") {
        setRootPage((current) => ({
          entries: [...(current?.entries ?? []), ...page.entries],
          nextCursor: page.nextCursor,
        }));
      } else {
        setChildPages((current) => {
          const previous = current.get(relativeDirectory);
          const next = new Map(current);
          next.set(relativeDirectory, {
            entries: [...(previous?.entries ?? []), ...page.entries],
            nextCursor: page.nextCursor,
          });
          return next;
        });
      }
    } catch (error) {
      if (sourceRef.current === activeSource && traversalEpoch.current === epoch) {
        setErrorMessage(safeReadError(error));
      }
    } finally {
      finishDirectoryRequest(relativeDirectory, requestToken);
    }
  }

  async function readFile(relativePath: string) {
    if (!source) {
      return;
    }
    const request = latestPreviewRequest.current + 1;
    latestPreviewRequest.current = request;
    const activeSource = source;
    setPreviewBusy(true);
    setPreview(null);
    setErrorMessage(null);
    try {
      const projection = await activeSource.readText(relativePath, 128 * 1024);
      if (request === latestPreviewRequest.current && sourceRef.current === activeSource) {
        setPreview(projection);
      }
    } catch (error) {
      if (request === latestPreviewRequest.current && sourceRef.current === activeSource) {
        setErrorMessage(safeReadError(error));
      }
    } finally {
      if (request === latestPreviewRequest.current && sourceRef.current === activeSource) {
        setPreviewBusy(false);
      }
    }
  }

  function changeSelection(relativePath: string, selected: boolean) {
    setSelectedPaths((current) => {
      const next = new Set(current);
      if (selected) {
        if (next.size >= workspaceReadLimits.contextPaths) {
          return current;
        }
        next.add(relativePath);
      } else {
        next.delete(relativePath);
      }
      return next;
    });
  }

  async function submitSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const queryBytes = new TextEncoder().encode(searchQuery.trim()).byteLength;
    if (
      !source
      || queryBytes === 0
      || queryBytes > workspaceReadLimits.searchQueryBytes
    ) {
      return;
    }
    const request = latestSearchRequest.current + 1;
    latestSearchRequest.current = request;
    const activeSource = source;
    setSearchBusy(true);
    setSearchResults(null);
    setErrorMessage(null);
    try {
      const matches = await activeSource.search(searchQuery, 100);
      if (request === latestSearchRequest.current && sourceRef.current === activeSource) {
        setSearchResults(matches);
      }
    } catch (error) {
      if (request === latestSearchRequest.current && sourceRef.current === activeSource) {
        setErrorMessage(safeReadError(error));
      }
    } finally {
      if (request === latestSearchRequest.current && sourceRef.current === activeSource) {
        setSearchBusy(false);
      }
    }
  }

  async function reviewContext() {
    if (!source || selectedPaths.size === 0 || contextRequestActive.current) {
      return;
    }
    const activeSource = source;
    const request = latestContextRequest.current + 1;
    latestContextRequest.current = request;
    contextRequestActive.current = true;
    setContextBusy(true);
    setErrorMessage(null);
    try {
      const projection = await activeSource.previewContext([...selectedPaths]);
      if (request === latestContextRequest.current && sourceRef.current === activeSource) {
        onContextReview(projection, activeSource.provenance);
      }
    } catch (error) {
      if (request === latestContextRequest.current && sourceRef.current === activeSource) {
        setErrorMessage(safeReadError(error));
      }
    } finally {
      if (request === latestContextRequest.current) {
        contextRequestActive.current = false;
        if (sourceRef.current === activeSource) {
          setContextBusy(false);
        }
      }
    }
  }

  const provenanceLabel = source?.provenance === "local_host"
    ? "Validated local projection"
    : "Browser demo data";
  const searchQueryBytes = new TextEncoder().encode(searchQuery.trim()).byteLength;
  const searchQueryValid = searchQueryBytes > 0
    && searchQueryBytes <= workspaceReadLimits.searchQueryBytes;
  const searchStatus = searchBusy
    ? "Searching visible text."
    : searchResults === null
      ? ""
      : `${searchResults.length} ${searchResults.length === 1 ? "match" : "matches"} found.`;

  return (
    <main className="workspace-explorer" inert={isInert}>
      <header className="workspace-explorer__header">
        <div>
          <span>Local workspace</span>
          <h1>{workspaceName}</h1>
          <p>Browse bounded text projections without exposing the selected root.</p>
        </div>
        <div className={`projection-provenance projection-provenance--${source?.provenance ?? "unavailable"}`}>
          <ShieldCheck aria-hidden="true" size={15} />
          {source ? provenanceLabel : "Read surface unavailable"}
        </div>
      </header>

      {source?.provenance === "browser_demo" ? (
        <div className="browser-demo-banner" role="note">
          <AlertTriangle aria-hidden="true" size={17} />
          <div>
            <strong>Browser demo</strong>
            <span>Sample relative paths only. No folder, host grant, or local file has been read.</span>
          </div>
        </div>
      ) : null}

      {!source ? (
        <section className="workspace-unavailable" aria-labelledby="workspace-unavailable-title">
          <FileWarning aria-hidden="true" size={27} />
          <h2 id="workspace-unavailable-title">Explorer is read only and unavailable</h2>
          <p>{availabilityMessage}</p>
        </section>
      ) : (
        <>
          {errorMessage ? (
            <div className="workspace-read-error" role="alert">
              <AlertTriangle aria-hidden="true" size={17} />
              <span>{errorMessage}</span>
            </div>
          ) : null}
          <div className="workspace-explorer__body">
            <section className="workspace-browser" aria-label="Workspace browser">
              <div className="workspace-browser__topbar">
                <Tabs
                  className="workspace-browser__tabs"
                  onSelectionChange={(key) => setActiveSection(key as ExplorerSection)}
                  selectedKey={activeSection}
                >
                  <TabList aria-label="Explorer views">
                    <Tab id="files">Files</Tab>
                    <Tab id="search">Search</Tab>
                    <Tab id="bmad">BMAD</Tab>
                  </TabList>
                  <TabPanel id="files">
                    <div className="workspace-panel-heading">
                      <div>
                        <strong>Workspace files</strong>
                        <span>UTF-8 text only</span>
                      </div>
                      <Button
                        aria-label="Refresh workspace projections"
                        isDisabled={loadingDirectories.has(".") || bmadBusy}
                        onPress={() => void refreshWorkspace()}
                        size="icon"
                        variant="quiet"
                      >
                        <RefreshCw aria-hidden="true" size={16} />
                      </Button>
                    </div>
                    <div className="workspace-tree" aria-busy={loadingDirectories.has(".")}>
                      {rootPage ? (
                        <TreeRows
                          childPages={childPages}
                          entries={rootPage.entries}
                          expandedPaths={expandedPaths}
                          loadingDirectories={loadingDirectories}
                          onLoadMore={(directory, cursor) => void loadMore(directory, cursor)}
                          onReadFile={(path) => void readFile(path)}
                          onSelectionChange={changeSelection}
                          onToggleDirectory={(entry) => void toggleDirectory(entry)}
                          previewPath={preview?.relativePath ?? null}
                          selectionLimitReached={
                            selectedPaths.size >= workspaceReadLimits.contextPaths
                          }
                          selectedPaths={selectedPaths}
                        />
                      ) : loadingDirectories.has(".") ? (
                        <div className="workspace-loading-state" role="status">
                          <LoaderCircle aria-hidden="true" className="spin" size={18} />
                          Loading a bounded file page…
                        </div>
                      ) : (
                        <p className="workspace-inline-empty">No visible entries were projected.</p>
                      )}
                      {rootPage?.nextCursor ? (
                        <Button
                          className="workspace-tree__root-more"
                          isDisabled={loadingDirectories.has(".")}
                          onPress={() => void loadMore(".", rootPage.nextCursor!)}
                          size="small"
                          variant="secondary"
                        >
                          Load more files
                        </Button>
                      ) : null}
                    </div>
                  </TabPanel>
                  <TabPanel id="search">
                    <form className="workspace-search" onSubmit={submitSearch}>
                      <label htmlFor="workspace-search-query">Search visible text</label>
                      <div>
                        <Search aria-hidden="true" size={16} />
                        <input
                          id="workspace-search-query"
                          maxLength={workspaceReadLimits.searchQueryBytes}
                          onChange={(event) => setSearchQuery(event.target.value)}
                          placeholder="Search up to 4 MiB of text…"
                          type="search"
                          value={searchQuery}
                        />
                        <Button
                          isDisabled={searchBusy || !searchQueryValid}
                          size="small"
                          type="submit"
                          variant="primary"
                        >
                          {searchBusy ? "Searching…" : "Search"}
                        </Button>
                      </div>
                    </form>
                    <p className="sr-only" role="status">{searchStatus}</p>
                    <div className="workspace-search-results">
                      {searchResults === null ? (
                        <div className="workspace-inline-empty workspace-inline-empty--centered">
                          <FileSearch aria-hidden="true" size={24} />
                          <p>Search returns bounded line previews and relative paths.</p>
                        </div>
                      ) : searchResults.length === 0 ? (
                        <p className="workspace-inline-empty">No matching visible text.</p>
                      ) : (
                        <ol>
                          {searchResults.map((match) => (
                            <li key={`${match.relativePath}:${match.line}`}>
                              <div className="search-result__heading">
                                <Checkbox
                                  aria-label={`Include ${match.relativePath} in context`}
                                  isDisabled={
                                    selectedPaths.size >= workspaceReadLimits.contextPaths
                                    && !selectedPaths.has(match.relativePath)
                                  }
                                  isSelected={selectedPaths.has(match.relativePath)}
                                  onChange={(selected) => changeSelection(match.relativePath, selected)}
                                >
                                  <span className="sr-only">Include in context</span>
                                </Checkbox>
                                <Button
                                  className="search-result__path"
                                  onPress={() => void readFile(match.relativePath)}
                                  variant="quiet"
                                >
                                  <code>{match.relativePath}</code>
                                  <span>Line {match.line}</span>
                                </Button>
                              </div>
                              <p>{match.preview}</p>
                            </li>
                          ))}
                        </ol>
                      )}
                    </div>
                  </TabPanel>
                  <TabPanel id="bmad">
                    <div className="bmad-panel">
                      <div className="workspace-panel-heading">
                        <div>
                          <strong>BMAD detection</strong>
                          <span>Assets remain non-executable</span>
                        </div>
                        {bmadBusy ? <LoaderCircle aria-hidden="true" className="spin" size={17} /> : null}
                      </div>
                      {bmad ? (
                        <>
                          <div className="bmad-help" role="note">
                            <Workflow aria-hidden="true" size={18} />
                            <div>
                              <strong>{bmad.status === "not_detected" ? "No BMAD assets detected" : "BMAD assets detected"}</strong>
                              <span>
                                Method assets are inspectable read-only. Builder Build, Edit, and Analyze outputs remain inactive drafts.
                              </span>
                            </div>
                          </div>
                          {bmad.assets.length > 0 ? (
                            <ul className="bmad-assets">
                              {bmad.assets.map((asset) => (
                                <li key={asset.relativePath}>
                                  <FileCode2 aria-hidden="true" size={17} />
                                  <div>
                                    <code>{asset.relativePath}</code>
                                    <span>{bmadAssetLabels[asset.assetKind]}</span>
                                  </div>
                                  <em>{asset.activation === "read_only" ? "Read only" : "Inactive draft"}</em>
                                </li>
                              ))}
                            </ul>
                          ) : null}
                          {bmad.truncated ? (
                            <p className="bmad-truncated">More assets exist; the host projection reached its 256-item limit.</p>
                          ) : null}
                        </>
                      ) : (
                        <p className="workspace-inline-empty">BMAD scan has not completed.</p>
                      )}
                    </div>
                  </TabPanel>
                </Tabs>
              </div>
            </section>

            <section className="text-preview" aria-labelledby="text-preview-title">
              <header>
                <div>
                  <span>Text preview</span>
                  <h2 id="text-preview-title">{preview ? preview.relativePath : "Select a text file"}</h2>
                </div>
                {preview ? (
                  <div className="text-preview__metrics">
                    <span>{formatBytes(preview.byteCount)}</span>
                    {preview.truncated ? <em>Preview truncated</em> : <em>Complete preview</em>}
                  </div>
                ) : null}
              </header>
              {previewBusy ? (
                <div className="text-preview__empty" role="status">
                  <LoaderCircle aria-hidden="true" className="spin" size={20} />
                  Reading a bounded UTF-8 projection…
                </div>
              ) : preview ? (
                <>
                  <pre aria-label={`Read-only preview of ${preview.relativePath}`} tabIndex={0}>
                    <code>{preview.content}</code>
                  </pre>
                  <footer>
                    <span>Content hash</span>
                    <code>{preview.contentHash}</code>
                  </footer>
                </>
              ) : (
                <div className="text-preview__empty">
                  <FileCode2 aria-hidden="true" size={29} />
                  <h2>Bounded UTF-8 preview</h2>
                  <p>Binary, blocked, secret-like, and oversized content remains unavailable.</p>
                </div>
              )}
            </section>
          </div>

          <footer className="context-selection-bar">
            <div>
              <ListChecks aria-hidden="true" size={18} />
              <span>
                <strong>{selectedPaths.size}</strong> {selectedPaths.size === 1 ? "item" : "items"} selected for context review
              </span>
            </div>
            <p>No model request is available in this build.</p>
            <Button
              isDisabled={contextBusy || selectedPaths.size === 0}
              onPress={() => void reviewContext()}
              variant="primary"
            >
              {contextBusy ? "Preparing review…" : "Review context"}
            </Button>
          </footer>
        </>
      )}
    </main>
  );
}
