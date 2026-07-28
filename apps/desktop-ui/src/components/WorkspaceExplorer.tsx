/**
 * The workspace read surface: files, search and the BMAD scan.
 *
 * FE-21. The Files tab is no longer written here. Its tree used to be a
 * recursive component that decided visibility as it rendered, which put
 * `aria-level`, `aria-posinset` and every keyboard question behind jsdom; it
 * now lives in `./files`, where the shape of the tree is a value and this
 * component's job is reduced to what it was always actually good at -- holding
 * the host state and guarding the async continuations that fill it.
 */

import { Button, Checkbox, Tab, TabList, TabPanel, Tabs } from "@sapphirus/ui";
import {
  AlertTriangle,
  FileCode2,
  FileSearch,
  FileWarning,
  ListChecks,
  LoaderCircle,
  Search,
  ShieldCheck,
  Workflow,
} from "lucide-react";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type FormEvent,
} from "react";
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
import { FilesInspector } from "./files/FilesInspector";
import {
  emptyExpansion,
  expandedForGrant,
  formatBytes,
  withExpanded,
  type DirectoryPage,
  type TreeExpansion,
} from "./files/fileTreeModel";

type ExplorerSection = "files" | "search" | "bmad";

const bmadAssetLabels: Record<BmadAssetKind, string> = {
  method_configuration: "Method configuration",
  agent: "Agent",
  workflow: "Workflow",
  builder_build_draft: "Builder Build draft",
  builder_edit_draft: "Builder Edit draft",
  builder_analyze_draft: "Builder Analyze draft",
};

export interface WorkspaceExplorerProps {
  asPanel?: boolean;
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
  asPanel = false,
  availabilityMessage,
  isInert = false,
  onContextReview,
  source,
  workspaceName,
}: WorkspaceExplorerProps) {
  const Root = asPanel ? "section" : "main";
  /*
   * Which workspace the expansion below belongs to.
   *
   * A workspace switch and a re-grant both arrive as a new `source`, and the
   * reset effect further down discards the pages that went with the old one --
   * but an effect runs a commit late, and for that one render the outgoing
   * workspace's expanded set would be applied to the incoming workspace's
   * entries. `src/` in one workspace is not `src/` in another and the paths
   * resolve either way, so the result would be a wrong answer that looked like
   * a right one. Reading the set back through this key during render means a
   * mismatch reads as "nothing is expanded" before any effect gets a turn.
   */
  const grantKey = source?.workspaceId ?? "unavailable";
  const [activeSection, setActiveSection] = useState<ExplorerSection>("files");
  const [bmad, setBmad] = useState<BmadScanProjection | null>(null);
  const [bmadBusy, setBmadBusy] = useState(false);
  const [childPages, setChildPages] = useState<ReadonlyMap<string, DirectoryPage>>(new Map());
  const [contextBusy, setContextBusy] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [expansion, setExpansion] = useState<TreeExpansion>(() => emptyExpansion(grantKey));
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
  const expandedPaths = expandedForGrant(expansion, grantKey);
  // `useLayoutEffect`, not an assignment in the component body.
  //
  // Every reader of this ref is an async continuation asking "is the workspace
  // I started reading from still the current one?", and they all run after a
  // commit. Writing during render answered that question from renders React
  // may throw away: a discarded render for a source that never commits left
  // the mirror pointing at it, and continuations belonging to the source that
  // *did* commit then failed their own identity check and dropped silently —
  // leaving `previewBusy` and the directory spinners stuck on.
  //
  // A layout effect runs after the commit and before any passive effect, so
  // the reset effect below still observes the new source, and the only window
  // where the mirror lags is the one before the new source is committed —
  // which is exactly when the old answer is the right one.
  useLayoutEffect(() => {
    sourceRef.current = source;
  }, [source]);

  /// Stable per `source`, so the effects and callbacks that report through it
  /// can declare it as a dependency instead of quietly closing over it.
  const safeReadError = useCallback((error: unknown): string => {
    if (source?.provenance === "browser_demo") {
      return "The browser demo could not load that fixture. No local workspace was accessed.";
    }
    return getSafeHostMessage(error);
  }, [source]);

  /// Hoisted above the source-change effect below and given a stable identity,
  /// so that effect can declare it. It reads nothing but `source` and refs, so
  /// its identity changes exactly when `source` does — which that effect
  /// already depends on, so declaring it adds no re-runs.
  const readFile = useCallback(async (relativePath: string) => {
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
  }, [safeReadError, source]);

  // Everything below discards a workspace's view and reloads it from scratch.
  // The synchronous `setState` burst is the discard — it is the effect's whole
  // purpose, not a cascade caused by one — and the loading flags that follow
  // exist because a host read is asynchronous and there is nothing to derive
  // "loading" from during render. `react-hooks/set-state-in-effect` cannot tell
  // the two apart; the deps below are the part that has to stay honest, and
  // they are.
  /* eslint-disable react-hooks/set-state-in-effect */
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
    // The expansion and the pages go together: a directory left expanded with
    // its page discarded is a disclosure that says "open" over nothing, and
    // nothing would ever fetch it again because the expand is what fetches.
    // This covers the case the grant key above cannot -- the same workspace
    // granted afresh, where the key is unchanged but the tree is not.
    setChildPages(new Map());
    setExpansion(emptyExpansion(grantKey));
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
          if (source.provenance === "browser_demo") {
            const firstFile = projection.entries.find((entry) => entry.kind === "text_file");
            if (firstFile) {
              void readFile(firstFile.relativePath);
            }
          }
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
    // Both callbacks and the grant key are rooted at `source`, so this array
    // still re-runs exactly once per workspace change.
  }, [grantKey, readFile, safeReadError, source]);
  /* eslint-enable react-hooks/set-state-in-effect */

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
    setExpansion(emptyExpansion(grantKey));
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

  /*
   * Directional, rather than a toggle that works out which way to go from
   * `expandedPaths`.
   *
   * The filter in the inspector opens whatever it must to reach a match, so a
   * directory can be shown open while `expandedPaths` does not hold it. Asking
   * the set which way to go would answer "closed" for a row the user has just
   * asked to close, and record an expansion instead -- invisible while the
   * filter holds the row open, and waiting for them the moment they cleared it.
   */
  async function setDirectoryExpanded(entry: WorkspaceTreeEntry, expanded: boolean) {
    if (!source || !entry.childCursor) {
      return;
    }
    setExpansion((current) => withExpanded(current, grantKey, entry.relativePath, expanded));
    // Closing never talks to the host. The in-flight token below would refuse
    // a second request anyway, so today this changes nothing -- but "a collapse
    // is free" should be true here rather than two hops away in a token map
    // that exists for a different reason.
    if (!expanded || childPages.has(entry.relativePath)) {
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
        // The page never arrived, so the disclosure goes back to closed rather
        // than sitting open over an empty directory the user cannot retry.
        setExpansion((current) => withExpanded(current, grantKey, entry.relativePath, false));
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
    <Root
      className={`workspace-explorer ${asPanel ? "workspace-explorer--panel" : ""}`}
      inert={isInert}
    >
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
                    <FilesInspector
                      childPages={childPages}
                      expandedPaths={expandedPaths}
                      isRootLoading={loadingDirectories.has(".")}
                      loadingDirectories={loadingDirectories}
                      onLoadMore={(directory, cursor) => void loadMore(directory, cursor)}
                      onReadFile={(path) => void readFile(path)}
                      onRefresh={() => void refreshWorkspace()}
                      onSelectionChange={changeSelection}
                      onSetDirectoryExpanded={
                        (entry, expanded) => void setDirectoryExpanded(entry, expanded)
                      }
                      previewPath={preview?.relativePath ?? null}
                      refreshDisabled={loadingDirectories.has(".") || bmadBusy}
                      rootPage={rootPage}
                      selectedPaths={selectedPaths}
                      selectionLimitReached={
                        selectedPaths.size >= workspaceReadLimits.contextPaths
                      }
                    />
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
    </Root>
  );
}
