import type { ReactNode } from "react";
import { Button, type ButtonProps } from "./Button";

/**
 * A button whose whole content is an icon.
 *
 * It exists as its own component rather than as `<Button size="icon">` for one
 * reason: an icon-only control has no accessible name unless something supplies
 * it, and a prop that is easy to forget will be forgotten. `label` is required
 * here, and it becomes both the accessible name and the tooltip, so the two can
 * never drift apart.
 */
export interface IconButtonProps
  extends Omit<ButtonProps, "children" | "size" | "aria-label" | "title"> {
  /** Names the action. Required: an icon alone is not a name. */
  label: string;
  children: ReactNode;
  /**
   * Suppresses the native tooltip when a richer one is supplied by a Tooltip
   * wrapper. The accessible name is unaffected.
   */
  suppressNativeTooltip?: boolean;
}

export function IconButton({
  label,
  children,
  suppressNativeTooltip = false,
  ...props
}: IconButtonProps) {
  return (
    <Button
      aria-label={label}
      size="icon"
      {...(suppressNativeTooltip ? {} : { title: label })}
      {...props}
    >
      {children}
    </Button>
  );
}
