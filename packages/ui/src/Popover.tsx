import type { ReactNode } from "react";
import {
  Dialog as AriaDialog,
  Popover as AriaPopover,
  type PopoverProps as AriaPopoverProps,
} from "react-aria-components";

export interface PopoverProps
  extends Omit<AriaPopoverProps, "children" | "className"> {
  /**
   * The popover's name.
   *
   * Required for the same reason as Dialog's: a popover is a dialog to
   * assistive technology, and an unnamed one is announced as nothing. It is
   * not rendered visually -- popovers usually take their visible heading from
   * their content -- so this is the accessible name only.
   */
  label: string;
  children: ReactNode;
  className?: string;
}

export function Popover({ label, children, className = "", ...props }: PopoverProps) {
  return (
    <AriaPopover
      className={["sapphirus-popover", className].filter(Boolean).join(" ")}
      {...props}
    >
      {/*
        The inner Dialog is not decoration. It is what gives the popover a role,
        a name, and focus containment that releases on close; a bare positioned
        div has none of those and is the usual reason focus is left stranded
        behind a dismissed overlay.
      */}
      <AriaDialog aria-label={label} className="sapphirus-popover__dialog">
        {children}
      </AriaDialog>
    </AriaPopover>
  );
}
