import { Button } from "@sapphirus/ui";
import { ListFilter, Pin, Plus, X } from "lucide-react";
import type { SessionSummary } from "../data/demo";
import { containModalPanelFocus, useModalPanelFocus } from "../lib/panelFocus";

export interface SessionRailProps {
  isOpen: boolean;
  isInert?: boolean;
  isOverlay: boolean;
  isSessionCreationEnabled: boolean;
  onClose: () => void;
  onNewSession: () => void;
  onSelect: (id: string) => void;
  selectedId: string;
  sessions: SessionSummary[];
  workspaceDescription: string;
  workspaceName: string;
}

export function SessionRail({
  isOpen,
  isInert = false,
  isOverlay,
  isSessionCreationEnabled,
  onClose,
  onNewSession,
  onSelect,
  selectedId,
  sessions,
  workspaceDescription,
  workspaceName,
}: SessionRailProps) {
  const isModal = isOverlay && isOpen;
  const isHidden = isOverlay && !isOpen;
  const panelRef = useModalPanelFocus(isModal);

  return (
    <aside
      aria-hidden={isHidden || undefined}
      aria-label="Sessions"
      aria-modal={isModal || undefined}
      className={`session-rail ${isOpen ? "is-open" : ""}`}
      inert={isHidden || isInert}
      onKeyDown={(event) => containModalPanelFocus(event, panelRef, isModal)}
      ref={panelRef}
      role={isOverlay ? "dialog" : undefined}
    >
      <div className="session-rail__actions">
        <Button
          className="new-session-button"
          isDisabled={!isSessionCreationEnabled}
          onPress={onNewSession}
          size="large"
          variant="primary"
        >
          <Plus aria-hidden="true" size={17} />
          New session
        </Button>
        <Button aria-label="Filter sessions" isDisabled size="icon" variant="secondary">
          <ListFilter aria-hidden="true" size={18} />
        </Button>
        <Button
          aria-label="Close sessions"
          className="session-close"
          onPress={onClose}
          size="icon"
          variant="quiet"
        >
          <X aria-hidden="true" size={18} />
        </Button>
      </div>
      <div className="session-list-section">
        <h2>Sessions</h2>
        <div className="session-list">
          {sessions.map((session) => (
            <Button
              {...(selectedId === session.id ? { "aria-current": "page" as const } : {})}
              className="session-row"
              key={session.id}
              onPress={() => onSelect(session.id)}
              variant="quiet"
            >
              <span className="session-row__title">{session.title}</span>
              <span className="session-row__meta">
                <time>{session.updatedAt}</time>
                {session.unread ? <span aria-label="Unread" className="unread-dot" /> : null}
              </span>
            </Button>
          ))}
        </div>
      </div>
      <div className="pinned-workspace">
        <div className="pinned-workspace__label">
          <span>Pinned workspace</span>
          <Pin aria-hidden="true" size={14} />
        </div>
        <strong>{workspaceName}</strong>
        <span className="pinned-workspace__path">{workspaceDescription}</span>
      </div>
    </aside>
  );
}
