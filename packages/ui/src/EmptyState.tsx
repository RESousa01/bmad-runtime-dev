import type { ReactNode } from "react";

export interface EmptyStateProps {
  /** One line saying what would be here. Not "No data". */
  title: string;
  /** What to do about it. Optional, but usually the useful half. */
  description?: string;
  /** The action that resolves the emptiness. */
  action?: ReactNode;
  className?: string;
}

/**
 * The state a surface is in before it has anything.
 *
 * Rendered as a heading and a sentence on the canvas, not inside a dashed box:
 * a container drawn around the absence of content is still a container, and
 * the approved prototype draws none anywhere.
 */
export function EmptyState({ title, description, action, className = "" }: EmptyStateProps) {
  return (
    <div className={["sapphirus-empty-state", className].filter(Boolean).join(" ")}>
      <h2 className="sapphirus-empty-state__title">{title}</h2>
      {description === undefined ? null : (
        <p className="sapphirus-empty-state__description">{description}</p>
      )}
      {action === undefined ? null : (
        <div className="sapphirus-empty-state__action">{action}</div>
      )}
    </div>
  );
}
