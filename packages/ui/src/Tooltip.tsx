import type { ReactNode } from "react";
import {
  Tooltip as AriaTooltip,
  TooltipTrigger as AriaTooltipTrigger,
  type TooltipTriggerComponentProps,
} from "react-aria-components";

export interface TooltipProps {
  children: ReactNode;
  className?: string;
}

/**
 * A tooltip.
 *
 * Deliberately not a way to name a control. A tooltip is supplementary: it is
 * not announced by every screen reader, it never appears for a touch user, and
 * it is absent until hover or focus. If the text is the only thing that says
 * what a control does, it belongs in the accessible name -- which is why
 * IconButton takes a required `label` rather than expecting one of these.
 */
export function Tooltip({ children, className = "" }: TooltipProps) {
  return (
    <AriaTooltip className={["sapphirus-tooltip", className].filter(Boolean).join(" ")}>
      {children}
    </AriaTooltip>
  );
}

export function TooltipTrigger({ children, ...props }: TooltipTriggerComponentProps) {
  // `delay` defaults to 1500ms in RAC, which reads as broken on a toolbar.
  return (
    <AriaTooltipTrigger delay={400} {...props}>
      {children}
    </AriaTooltipTrigger>
  );
}
