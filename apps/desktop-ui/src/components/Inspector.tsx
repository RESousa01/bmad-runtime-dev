import { Button, Tab, TabList, TabPanel, Tabs } from "@sapphirus/ui";
import {
  ChevronRight,
  FileCode2,
  PencilLine,
  ShieldCheck,
  Trash2,
  X,
} from "lucide-react";
import type { InspectorTab, ProposalState } from "../data/demo";
import type { ContextPreviewProjection } from "../lib/hostClient";
import type { WorkspaceProjectionProvenance } from "../lib/workspaceReadSource";
import { containModalPanelFocus, useModalPanelFocus } from "../lib/panelFocus";
import { CodeDiff } from "./CodeDiff";

const inspectorTabs: Array<{ id: InspectorTab; label: string }> = [
  { id: "context", label: "Context" },
  { id: "changes", label: "Changes" },
  { id: "logs", label: "Logs" },
  { id: "evidence", label: "Evidence" },
];

export interface InspectorProps {
  contextPreview: ContextPreviewProjection | null;
  contextProvenance: WorkspaceProjectionProvenance | null;
  interactionDisabled: boolean;
  isInert?: boolean;
  isOpen: boolean;
  isOverlay: boolean;
  onApply: () => void;
  onClose: () => void;
  onDiscard: () => void;
  onRevise: () => void;
  onTabChange: (tab: InspectorTab) => void;
  proposalState: ProposalState;
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

function ChangesPanel({
  interactionDisabled,
  onApply,
  onDiscard,
  onRevise,
  proposalState,
}: Pick<InspectorProps, "interactionDisabled" | "onApply" | "onDiscard" | "onRevise" | "proposalState">) {
  if (proposalState === "discarded") {
    return (
      <div className="inspector-empty-state">
        <Trash2 aria-hidden="true" size={24} />
        <h3>No proposed changes</h3>
        <p>The previous proposal was discarded without changing your local workspace.</p>
      </div>
    );
  }

  return (
    <>
      <div className="inspector-preview-callout" role="note">
        <strong>Preview demo</strong>
        <span>No candidate, approval, checkpoint, or file write exists for these sample changes.</span>
      </div>
      <div className="proposal-files">
        <strong>2 preview files</strong>
        <div className="proposal-file-row">
          <ChevronRight aria-hidden="true" size={16} />
          <code>src/scan/workspace_scanner.ts</code>
          <span className="change-count change-count--added">+142</span>
        </div>
        <div className="proposal-file-row">
          <ChevronRight aria-hidden="true" size={16} />
          <code>tests/scan/workspace_scanner.test.ts</code>
          <span className="change-count"><b>+88</b> <i>−11</i></span>
        </div>
      </div>
      <CodeDiff />
      <div className="change-actions">
        <Button isDisabled={interactionDisabled} onPress={onDiscard} size="large" variant="secondary">
          <Trash2 aria-hidden="true" size={17} />
          Discard
        </Button>
        <Button isDisabled={interactionDisabled} onPress={onRevise} size="large" variant="secondary">
          <PencilLine aria-hidden="true" size={17} />
          Revise
        </Button>
        <Button isDisabled={interactionDisabled} onPress={onApply} size="large" variant="primary">
          <ShieldCheck aria-hidden="true" size={17} />
          Apply changes
        </Button>
      </div>
      <p className="inspector-footnote">Preview only — Apply changes is unavailable in this internal build.</p>
    </>
  );
}

export function Inspector({
  contextPreview,
  contextProvenance,
  interactionDisabled,
  isInert = false,
  isOpen,
  isOverlay,
  onApply,
  onClose,
  onDiscard,
  onRevise,
  onTabChange,
  proposalState,
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
      className={`inspector ${isOpen ? "is-open" : ""}`}
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
        <TabList aria-label="Inspector sections" items={inspectorTabs}>
          {(item) => <Tab id={item.id}>{item.label}</Tab>}
        </TabList>
        <TabPanel id="context">
          <ContextPanel contextPreview={contextPreview} contextProvenance={contextProvenance} />
        </TabPanel>
        <TabPanel className="changes-tab-panel" id="changes">
          <ChangesPanel
            interactionDisabled={interactionDisabled}
            onApply={onApply}
            onDiscard={onDiscard}
            onRevise={onRevise}
            proposalState={proposalState}
          />
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
      </Tabs>
    </aside>
  );
}
