import { Button, Tab, TabList, TabPanel, Tabs } from "@sapphirus/ui";
import { FileCode2, ShieldCheck, X } from "lucide-react";
import type { InspectorTab } from "../data/demo";
import type { ContextPreviewProjection } from "../lib/hostClient";
import type { BmadHelpUiState, BmadLibraryUiState } from "../lib/bmadProjection";
import type { WorkspaceProjectionProvenance } from "../lib/workspaceReadSource";
import { containModalPanelFocus, useModalPanelFocus } from "../lib/panelFocus";
import { BmadHelpCard } from "./BmadHelpCard";
import { BmadLibraryPanel } from "./BmadLibraryPanel";
import {
  GovernedChangesPanel,
  type GovernedChangesPanelProps,
} from "./GovernedChangesPanel";

const inspectorTabs: Array<{ accessibleLabel: string; id: InspectorTab; label: string }> = [
  { accessibleLabel: "Context", id: "context", label: "Context" },
  { accessibleLabel: "Changes", id: "changes", label: "Changes" },
  { accessibleLabel: "Logs", id: "logs", label: "Logs" },
  { accessibleLabel: "Evidence", id: "evidence", label: "Evidence" },
  { accessibleLabel: "Method library", id: "method", label: "Method" },
];

export interface InspectorProps {
  bmadHelpState: BmadHelpUiState;
  bmadLibraryState: BmadLibraryUiState;
  changesPanel: GovernedChangesPanelProps;
  contextPreview: ContextPreviewProjection | null;
  contextProvenance: WorkspaceProjectionProvenance | null;
  isInert?: boolean;
  isOpen: boolean;
  isOverlay: boolean;
  methodLibraryAvailable: boolean;
  onClose: () => void;
  onReloadMethodLibrary: () => void;
  onTabChange: (tab: InspectorTab) => void;
  selectedTab: InspectorTab;
}

function formatContextBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`;
}

function ContextPanel({
  contextPreview,
  contextProvenance,
}: Pick<InspectorProps, "contextPreview" | "contextProvenance">) {
  if (!contextPreview || !contextProvenance) {
    return (
      <div className="context-panel">
        <div className="inspector-section-heading">
          <h2>Context review</h2>
          <span>No manifest prepared</span>
        </div>
        <div className="inspector-empty-state inspector-empty-state--inline">
          <FileCode2 aria-hidden="true" size={24} />
          <h3>No context selected</h3>
          <p>Select bounded UTF-8 files in Explorer, then choose Review context.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="context-panel">
      <div className="inspector-section-heading">
        <h2>Review selected context</h2>
        <span>
          {contextProvenance === "local_host" ? "Validated local projection" : "Browser demo data"}
          {` · ${contextPreview.items.length} ${contextPreview.items.length === 1 ? "item" : "items"}`}
        </span>
      </div>
      <div className="context-review-notice" role="note">
        <strong>No model request</strong>
        <span>This D1 review is local and read only. No context has been transmitted.</span>
      </div>
      <div className="context-list">
        {contextPreview.items.map((item, index) => (
          <details key={item.relativePath} open={index === 0}>
            <summary>
              <FileCode2 aria-hidden="true" size={18} />
              <div>
                <code>{item.relativePath}</code>
                <span>
                  Lines {item.startLine}–{item.endLine}
                  {` · ${formatContextBytes(item.byteCount)} · ${item.estimatedTokens.toLocaleString()} tokens`}
                </span>
              </div>
              <em>Source</em>
            </summary>
            <dl>
              <div><dt>Reason</dt><dd>{item.reason}</dd></div>
              <div><dt>Content hash</dt><dd><code>{item.contentHash}</code></dd></div>
              <div><dt>Redactions</dt><dd>No redactions</dd></div>
            </dl>
            <pre aria-label={`Exact context content for ${item.relativePath}`} tabIndex={0}>
              <code>{item.content}</code>
            </pre>
          </details>
        ))}
      </div>
      <dl className="context-metadata">
        <div><dt>Total bytes</dt><dd>{formatContextBytes(contextPreview.totalBytes)}</dd></div>
        <div><dt>Estimated tokens</dt><dd>{contextPreview.estimatedTokens.toLocaleString()}</dd></div>
        <div><dt>Manifest hash</dt><dd><code>{contextPreview.manifestHash}</code></dd></div>
        <div><dt>Model</dt><dd>Not available in D1</dd></div>
        <div><dt>Retention</dt><dd>No request sent</dd></div>
      </dl>
    </div>
  );
}

export function Inspector({
  bmadHelpState,
  bmadLibraryState,
  changesPanel,
  contextPreview,
  contextProvenance,
  isInert = false,
  isOpen,
  isOverlay,
  methodLibraryAvailable,
  onClose,
  onReloadMethodLibrary,
  onTabChange,
  selectedTab,
}: InspectorProps) {
  const isModal = isOverlay && isOpen;
  const isHidden = isOverlay && !isOpen;
  const panelRef = useModalPanelFocus(isModal);

  return (
    <aside
      aria-hidden={isHidden || undefined}
      aria-label="Inspector"
      aria-modal={isModal || undefined}
      className={`inspector ${isOpen ? "is-open" : ""} ${methodLibraryAvailable ? "has-method-library" : ""}`}
      inert={isHidden || isInert}
      onKeyDown={(event) => containModalPanelFocus(event, panelRef, isModal)}
      ref={panelRef}
      role={isOverlay ? "dialog" : undefined}
    >
      <Button
        aria-label="Close inspector"
        className="inspector-close"
        onPress={onClose}
        size="icon"
        variant="quiet"
      >
        <X aria-hidden="true" size={18} />
      </Button>
      <Tabs
        className="inspector-tabs"
        onSelectionChange={(key) => onTabChange(key as InspectorTab)}
        selectedKey={selectedTab}
      >
        <TabList
          aria-label="Inspector sections"
          items={inspectorTabs.filter((item) => item.id !== "method" || methodLibraryAvailable)}
        >
          {(item) => <Tab aria-label={item.accessibleLabel} id={item.id}>{item.label}</Tab>}
        </TabList>
        <TabPanel id="context">
          <ContextPanel contextPreview={contextPreview} contextProvenance={contextProvenance} />
        </TabPanel>
        <TabPanel className="changes-tab-panel" id="changes">
          <GovernedChangesPanel {...changesPanel} />
        </TabPanel>
        <TabPanel id="logs">
          <div className="log-panel">
            <div className="inspector-section-heading">
              <h2>Preview log</h2>
              <span>Demonstration events</span>
            </div>
            <ol>
              <li><time>10:41:55</time><span>Demo workspace state rendered</span></li>
              <li><time>10:41:56</time><span>Demo context rendered</span></li>
              <li><time>10:42:07</time><span>Demo proposal rendered</span></li>
            </ol>
          </div>
        </TabPanel>
        <TabPanel id="evidence">
          <div className="evidence-panel">
            <div className="inspector-section-heading">
              <h2>Evidence</h2>
              <span>Local records</span>
            </div>
            <div className="inspector-empty-state inspector-empty-state--inline">
              <ShieldCheck aria-hidden="true" size={24} />
              <h3>No evidence yet</h3>
              <p>This internal preview has not created a governed local action or evidence record.</p>
            </div>
          </div>
        </TabPanel>
        {methodLibraryAvailable ? (
          <TabPanel id="method">
            <div className="method-library-panel">
              <BmadHelpCard state={bmadHelpState} />
              <BmadLibraryPanel
                onReload={onReloadMethodLibrary}
                state={bmadLibraryState}
              />
            </div>
          </TabPanel>
        ) : null}
      </Tabs>
    </aside>
  );
}
