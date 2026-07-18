import { Button } from "@sapphirus/ui";
import { Bot, Check, ChevronDown, ShieldCheck } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type {
  BmadBlockerCode,
  BmadLibraryUiState,
  BmadMethodAgentProjection,
} from "../../lib/bmadProjection";

export interface AgentSelectorProps {
  isBrowserDemo: boolean;
  library: BmadLibraryUiState;
  methodGuidanceAvailable: boolean;
  methodGuidanceView: boolean;
  modelAccessDetail: string;
  modelAccessLabel: string;
}

function blockerLabel(code: BmadBlockerCode): string {
  switch (code) {
    case "bmad_capability_disabled":
      return "Capability disabled";
    case "bmad_dependency_unavailable":
      return "Dependency unavailable";
    case "bmad_help_catalog_orphan":
      return "Catalog entry unavailable";
    case "bmad_network_reference_unavailable":
      return "Network reference unavailable";
    case "bmad_source_prompt_unavailable":
      return "Source prompt unavailable";
  }
}

function agentBlockerSummary(agent: BmadMethodAgentProjection): string {
  if (agent.blockerCodes.length === 0) {
    return "Not selectable in this build";
  }
  return agent.blockerCodes.map(blockerLabel).join(" · ");
}

export function AgentSelector({
  isBrowserDemo,
  library,
  methodGuidanceAvailable,
  methodGuidanceView,
  modelAccessDetail,
  modelAccessLabel,
}: AgentSelectorProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);

  function close(restoreFocus = false) {
    const trigger = rootRef.current?.querySelector<HTMLButtonElement>(
      ".agent-control__trigger",
    );
    setOpen(false);
    if (restoreFocus) {
      window.requestAnimationFrame(() => trigger?.isConnected && trigger.focus());
    }
  }

  useEffect(() => {
    if (!open) return undefined;
    function closeOnOutsidePointer(event: PointerEvent) {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("pointerdown", closeOnOutsidePointer);
    return () => document.removeEventListener("pointerdown", closeOnOutsidePointer);
  }, [open]);

  const statusLabel = methodGuidanceAvailable
    ? "Available"
    : isBrowserDemo
      ? "Read only"
      : "Unavailable";

  return (
    <div
      className="agent-control"
      onKeyDown={(event) => {
        if (event.key === "Escape" && open) {
          event.preventDefault();
          event.stopPropagation();
          close(true);
        }
      }}
      ref={rootRef}
    >
      <Button
        aria-expanded={open}
        aria-controls="agent-model-access"
        aria-label="Agent and model settings"
        className="agent-control__trigger"
        onPress={() => setOpen((current) => !current)}
        size="small"
        variant="quiet"
      >
        <Bot aria-hidden="true" size={15} />
        <span>Agent</span>
        <span className="agent-control__summary">
          {methodGuidanceView ? "BMAD Help" : modelAccessLabel}
        </span>
        <ChevronDown aria-hidden="true" size={14} />
      </Button>
      {open ? (
        <section
          aria-label="Agent and model"
          className="agent-control__popover agent-selector"
          id="agent-model-access"
          role="region"
        >
          <header>
            <h2>Agent</h2>
            <span className="agent-control__status">
              <span
                className={`status-dot ${methodGuidanceAvailable ? "" : "status-dot--warning"}`}
              />
              {statusLabel}
            </span>
          </header>
          <ul aria-label="Agent capabilities" className="agent-selector__list">
            <li>
              <div
                aria-current={methodGuidanceView || methodGuidanceAvailable ? "true" : undefined}
                className={`agent-selector__option ${
                  methodGuidanceAvailable ? "" : "agent-selector__option--disabled"
                }`}
              >
                <span className="agent-selector__option-main">
                  <strong>BMAD Help</strong>
                  <small>Skill-guided request flow with review before send.</small>
                </span>
                {methodGuidanceAvailable ? (
                  <Check aria-hidden="true" size={15} />
                ) : (
                  <small className="agent-selector__blocker">{statusLabel}</small>
                )}
              </div>
            </li>
            {library.kind === "ready"
              ? library.projection.methodAgents.map((agent) => (
                <li key={`${agent.moduleCode}:${agent.agentCode}`}>
                  <div
                    className={`agent-selector__option ${
                      agent.availability === "available"
                        ? ""
                        : "agent-selector__option--disabled"
                    }`}
                  >
                    <span aria-hidden="true" className="agent-selector__icon">
                      {agent.icon}
                    </span>
                    <span className="agent-selector__option-main">
                      <strong>{agent.name}</strong>
                      <small>{agent.title}</small>
                    </span>
                    {agent.availability === "available" ? null : (
                      <small className="agent-selector__blocker">
                        {agentBlockerSummary(agent)}
                      </small>
                    )}
                  </div>
                </li>
              ))
              : null}
          </ul>
          {library.kind === "loading" || library.kind === "idle" ? (
            <p className="agent-selector__note" role="status">
              Loading installed agents…
            </p>
          ) : library.kind === "unavailable" ? (
            <p className="agent-selector__note" role="status">
              Installed agents are unavailable right now.
            </p>
          ) : null}
          <footer className="agent-selector__footer">
            <span className="agent-selector__model" title={modelAccessDetail}>
              {modelAccessLabel}
            </span>
            <span className="agent-selector__policy">
              <ShieldCheck aria-hidden="true" size={13} /> Review before send
            </span>
          </footer>
        </section>
      ) : null}
    </div>
  );
}
