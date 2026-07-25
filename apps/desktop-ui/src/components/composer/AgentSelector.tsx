import { Button } from "@sapphirus/ui";
import { ChevronDown, ChevronRight } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { BmadLibraryUiState } from "../../lib/bmadProjection";

export interface AgentSelectorProps {
  library: BmadLibraryUiState;
  modelAccessDetail: string;
  modelAccessLabel: string;
  onOpenMethodLibrary?: (() => void) | undefined;
  onOpenProviders?: (() => void) | undefined;
  /// The agent bound to this conversation, or `null` for plain chat.
  selectedAgentCode: string | null;
  /// Bind or clear the persona. Clearing is always offered: BMAD is optional,
  /// so "no agent" must stay reachable in one click.
  onSelectAgent: (agentCode: string | null) => void;
  /// Ask the BMAD Help advisor which skill fits. Explicit, because an ordinary
  /// message is an ordinary message.
  onAskMethodGuidance?: (() => void) | undefined;
}

export function AgentSelector({
  library,
  modelAccessDetail,
  modelAccessLabel,
  onOpenMethodLibrary,
  onOpenProviders,
  selectedAgentCode,
  onSelectAgent,
  onAskMethodGuidance,
}: AgentSelectorProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const installedAgentCount = library.kind === "ready"
    ? library.projection.methodAgents.length
    : null;
  const agents = library.kind === "ready" ? library.projection.methodAgents : [];
  const selectedAgent = agents.find((agent) => agent.agentCode === selectedAgentCode) ?? null;
  // "Plain chat" rather than a bare "Chat": the trigger names the *mode*, and
  // an unqualified "Chat" reads as a tab label rather than an agent choice.
  const triggerLabel = selectedAgent === null
    ? "Plain chat"
    : `${selectedAgent.icon} ${selectedAgent.name}`;

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
        title={modelAccessDetail}
        variant="quiet"
      >
        <span>{triggerLabel}</span>
        <ChevronDown aria-hidden="true" size={13} strokeWidth={1.8} />
      </Button>
      {open ? (
        <section
          aria-label="Agent and model"
          className="agent-control__popover agent-selector"
          id="agent-model-access"
          role="region"
        >
          <fieldset className="agent-selector__agents">
            <legend className="agent-selector__label">Agent</legend>
            {/* Plain chat is first and always available: BMAD is an overlay on
                the conversation, never a prerequisite for having one. */}
            <button
              aria-pressed={selectedAgentCode === null}
              className="agent-selector__row"
              onClick={() => {
                onSelectAgent(null);
                close(true);
              }}
              type="button"
            >
              <span className="agent-selector__value">No agent — plain chat</span>
            </button>
            {agents.map((agent) => (
              <button
                aria-pressed={selectedAgentCode === agent.agentCode}
                className="agent-selector__row"
                key={agent.agentCode}
                onClick={() => {
                  onSelectAgent(agent.agentCode);
                  close(true);
                }}
                title={`${agent.name} — ${agent.title}`}
                type="button"
              >
                <span className="agent-selector__value">
                  {agent.icon} {agent.name} — {agent.title}
                </span>
              </button>
            ))}
          </fieldset>
          {onAskMethodGuidance ? (
            <button
              className="agent-selector__row"
              onClick={() => {
                close();
                onAskMethodGuidance();
              }}
              type="button"
            >
              <span className="agent-selector__label">Ask</span>
              <span className="agent-selector__value">BMAD Help — what should I do next?</span>
            </button>
          ) : null}
          <button
            aria-label="Open skills and agents"
            className="agent-selector__row"
            disabled={onOpenMethodLibrary === undefined}
            onClick={() => {
              close();
              onOpenMethodLibrary?.();
            }}
            title={onOpenMethodLibrary === undefined
              ? "Installed BMAD agents require the Forge desktop host"
              : installedAgentCount === null
                ? "Open the installed BMAD agents"
                : `Open ${installedAgentCount} installed BMAD agents`}
            type="button"
          >
            <span className="agent-selector__label">Skills</span>
            <span className="agent-selector__value">Open skills and workflows</span>
            {onOpenMethodLibrary ? (
              <ChevronRight aria-hidden="true" size={14} strokeWidth={1.7} />
            ) : (
              <span aria-hidden="true" className="agent-selector__row-spacer" />
            )}
          </button>
          <button
            aria-label="Open model settings"
            className="agent-selector__row"
            disabled={onOpenProviders === undefined}
            onClick={() => {
              close();
              onOpenProviders?.();
            }}
            title={modelAccessDetail}
            type="button"
          >
            <span className="agent-selector__label">Model</span>
            <span className="agent-selector__value">{modelAccessLabel}</span>
            {onOpenProviders ? (
              <ChevronRight aria-hidden="true" size={14} strokeWidth={1.7} />
            ) : (
              <span aria-hidden="true" className="agent-selector__row-spacer" />
            )}
          </button>
        </section>
      ) : null}
    </div>
  );
}
