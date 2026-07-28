import type { ReactNode } from "react";
import { Toolbar as AriaToolbar, type ToolbarProps as AriaToolbarProps } from "react-aria-components";

export interface ToolbarProps extends Omit<AriaToolbarProps, "className" | "children"> {
  /** Names the toolbar. Required: an unlabelled toolbar announces as nothing. */
  label: string;
  children: ReactNode;
  className?: string;
}

/**
 * A labelled group of related controls with arrow-key navigation.
 *
 * What this gives you over a div of buttons is the `toolbar` role, an
 * accessible name for the group, and left/right arrows moving between the
 * controls -- the behaviour surfaces otherwise reimplement inconsistently.
 *
 * It is *not* a roving tabindex: react-aria-components applies that in
 * collection components such as ListBox and Menu, not here, so every control
 * remains individually tabbable. If a group needs to be a single tab stop,
 * that is a different primitive, and claiming it here would be worse than not
 * having it -- a keyboard user would find out by pressing Tab six times.
 */
export function Toolbar({ label, children, className = "", ...props }: ToolbarProps) {
  return (
    <AriaToolbar
      aria-label={label}
      className={["sapphirus-toolbar", className].filter(Boolean).join(" ")}
      {...props}
    >
      {children}
    </AriaToolbar>
  );
}
