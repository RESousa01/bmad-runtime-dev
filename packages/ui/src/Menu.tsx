import type { Key } from "react";
import {
  Menu as AriaMenu,
  MenuItem as AriaMenuItem,
  MenuTrigger as AriaMenuTrigger,
  Popover as AriaPopover,
  Text,
  type MenuTriggerProps,
} from "react-aria-components";

export interface MenuAction {
  id: string;
  label: string;
  /** A keyboard shortcut, shown trailing. Display only. */
  shortcut?: string;
  isDisabled?: boolean;
  /** Why the action cannot be taken. Shown, and announced with the label. */
  disabledReason?: string;
}

export interface MenuProps {
  actions: readonly MenuAction[];
  /** Names the menu. Required. */
  label: string;
  onAction?: (id: string) => void;
  className?: string;
}

export function Menu({ actions, label, onAction, className = "" }: MenuProps) {
  return (
    <AriaPopover className="sapphirus-popover">
      <AriaMenu
        aria-label={label}
        className={["sapphirus-menu", className].filter(Boolean).join(" ")}
        items={actions}
        {...(onAction === undefined
          ? {}
          : { onAction: (key: Key) => onAction(String(key)) })}
      >
        {(action: MenuAction) => (
          <AriaMenuItem
            className="sapphirus-menu__item"
            id={action.id}
            isDisabled={action.isDisabled ?? false}
          >
            <Text className="sapphirus-menu__label" slot="label">
              {action.label}
            </Text>
            {action.shortcut === undefined ? null : (
              <kbd className="sapphirus-menu__shortcut">{action.shortcut}</kbd>
            )}
            {action.isDisabled === true && action.disabledReason !== undefined ? (
              <Text className="sapphirus-menu__reason" slot="description">
                {action.disabledReason}
              </Text>
            ) : null}
          </AriaMenuItem>
        )}
      </AriaMenu>
    </AriaPopover>
  );
}

/**
 * Pairs a trigger with a Menu.
 *
 * Re-exported rather than reimplemented, because RAC already gives the
 * keyboard contract FE-06 asks for: arrows move, Home and End jump, typing
 * matches, Escape closes and returns focus to the trigger, and a disabled item
 * is skipped by the arrows but still reachable to be read.
 */
export function MenuTrigger({ children, ...props }: MenuTriggerProps) {
  return <AriaMenuTrigger {...props}>{children}</AriaMenuTrigger>;
}
