import { Button } from "@sapphirus/ui";
import { X } from "lucide-react";
import { useId, type ReactNode } from "react";

export type ContextDrawerKind =
  | "files"
  | "changes"
  | "run-details"
  | "methods";

export type ContextDrawerPresentation = "pane" | "overlay";

export interface ContextDrawerProps {
  children?: ReactNode;
  kind: ContextDrawerKind;
  onClose: () => void;
  presentation?: ContextDrawerPresentation;
}

const drawerTitles = {
  files: "Files",
  changes: "Changes",
  "run-details": "Run details",
  methods: "Skills and agents",
} satisfies Record<ContextDrawerKind, string>;

export function ContextDrawer({
  children,
  kind,
  onClose,
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
      <div className="context-drawer__content">{children}</div>
    </aside>
  );
}
