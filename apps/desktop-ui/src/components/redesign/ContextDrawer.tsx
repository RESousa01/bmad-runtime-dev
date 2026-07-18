import { Button } from "@sapphirus/ui";
import { X } from "lucide-react";
import { useId, type ReactNode } from "react";

export type ContextDrawerKind = "files" | "changes" | "activity" | "skills";

export type ContextDrawerPresentation = "pane" | "overlay";

export interface ContextDrawerProps {
  children?: ReactNode;
  kind: ContextDrawerKind;
  onClose: () => void;
  onSelectTab?: (kind: ContextDrawerKind) => void;
  presentation?: ContextDrawerPresentation;
}

const drawerTitles = {
  files: "Files",
  changes: "Changes",
  activity: "Activity",
  skills: "Skills and agents",
} satisfies Record<ContextDrawerKind, string>;

const drawerTabs: Array<{ id: ContextDrawerKind; label: string }> = [
  { id: "files", label: "Files" },
  { id: "changes", label: "Changes" },
  { id: "activity", label: "Activity" },
  { id: "skills", label: "Skills" },
];

export function ContextDrawer({
  children,
  kind,
  onClose,
  onSelectTab,
  presentation = "pane",
}: ContextDrawerProps) {
  const titleId = useId();
  const title = drawerTitles[kind];
  const isOverlay = presentation === "overlay";

  return (
    <aside
      aria-labelledby={titleId}
      aria-modal={isOverlay ? "true" : undefined}
      className={`context-drawer context-drawer--${presentation}`}
      role={isOverlay ? "dialog" : undefined}
    >
      <header className="context-drawer__header">
        <h2 className="context-drawer__title" id={titleId}>{title}</h2>
        <Button
          aria-label={`Close ${title}`}
          className="context-drawer__close"
          onPress={onClose}
          size="icon"
          variant="quiet"
        >
          <X aria-hidden="true" size={18} />
        </Button>
      </header>
      {onSelectTab ? (
        <nav aria-label="Panel views" className="context-drawer__tabs">
          {drawerTabs.map((tab) => (
            <button
              {...(kind === tab.id ? { "aria-current": "page" as const } : {})}
              className="context-drawer__tab"
              key={tab.id}
              onClick={() => onSelectTab(tab.id)}
              type="button"
            >
              {tab.label}
            </button>
          ))}
        </nav>
      ) : null}
      <div className="context-drawer__content">{children}</div>
    </aside>
  );
}
