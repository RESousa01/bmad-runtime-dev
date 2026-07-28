import { Button } from "@sapphirus/ui";
import { useId, type ReactNode } from "react";
import { OpenCodeIcon } from "../OpenCodeIcon";

export type ContextDrawerKind = "files" | "changes" | "context" | "activity" | "skills";

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
  changes: "Review",
  context: "Context",
  activity: "Activity",
  skills: "Skills and agents",
} satisfies Record<ContextDrawerKind, string>;

/*
 * Text, as 3a draws this row: Files, Review, Activity, Skills. The icons that
 * were here made the case against themselves -- "context" and "activity"
 * carried the same "status" glyph, so the picture distinguished four tabs
 * where there are five, and the label was doing the work either way.
 */
const drawerTabs: Array<{
  id: ContextDrawerKind;
  label: string;
}> = [
  { id: "files", label: "Files" },
  { id: "changes", label: "Review" },
  { id: "context", label: "Context" },
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
  const tabs = onSelectTab ? (
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
  ) : null;

  return (
    <aside
      aria-labelledby={titleId}
      aria-modal={isOverlay ? "true" : undefined}
      className={`context-drawer context-drawer--${presentation} context-drawer--opencode-v2`}
      data-window-system="opencode"
      role={isOverlay ? "dialog" : undefined}
    >
      <header className="context-drawer__header">
        <h2 className="context-drawer__title sr-only" id={titleId}>{title}</h2>
        {tabs}
        <Button
          aria-label={`Close ${title}`}
          className="context-drawer__close"
          onPress={onClose}
          size="icon"
          variant="quiet"
        >
          <OpenCodeIcon name="close" size={20} />
        </Button>
      </header>
      <div className="context-drawer__content">{children}</div>
    </aside>
  );
}
