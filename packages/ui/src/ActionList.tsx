import type { ReactNode } from "react";
import {
  ListBox as AriaListBox,
  ListBoxItem as AriaListBoxItem,
  Text,
  type ListBoxProps as AriaListBoxProps,
} from "react-aria-components";

export interface ActionListItem {
  id: string;
  label: string;
  /** Trailing metadata: a shortcut, a count, a timestamp. */
  detail?: string;
  isDisabled?: boolean;
  /**
   * Why this item cannot be used.
   *
   * FE-06's third acceptance criterion is that inactive items explain
   * themselves. A greyed row with no reason is the worst version of this: the
   * user can see something is unavailable and has no idea what to change.
   *
   * It is rendered in the description slot rather than folded into the label,
   * which is where ARIA puts this kind of text: the name stays the action, and
   * react-aria wires `aria-describedby` so a screen reader still receives the
   * reason. The grey alone conveys nothing there.
   */
  disabledReason?: string;
}

export interface ActionListProps
  extends Omit<AriaListBoxProps<ActionListItem>, "children" | "items" | "className"> {
  items: readonly ActionListItem[];
  /** Names the list. Required: an unlabelled listbox announces as nothing. */
  label: string;
  className?: string;
  emptyState?: ReactNode;
}

export function ActionList({
  items,
  label,
  className = "",
  emptyState,
  ...props
}: ActionListProps) {
  return (
    <AriaListBox
      aria-label={label}
      className={["sapphirus-action-list", className].filter(Boolean).join(" ")}
      items={items}
      // Spread rather than pass undefined: exactOptionalPropertyTypes treats
      // "absent" and "present and undefined" as different types.
      {...(emptyState === undefined ? {} : { renderEmptyState: () => emptyState })}
      {...props}
    >
      {(item: ActionListItem) => (
        <AriaListBoxItem
          className="sapphirus-action-list__item"
          id={item.id}
          isDisabled={item.isDisabled ?? false}
        >
          <Text className="sapphirus-action-list__label" slot="label">
            {item.label}
          </Text>
          {item.detail === undefined ? null : (
            <Text className="sapphirus-action-list__detail" slot="description">
              {item.detail}
            </Text>
          )}
          {item.isDisabled === true && item.disabledReason !== undefined ? (
            <Text className="sapphirus-action-list__reason" slot="description">
              {item.disabledReason}
            </Text>
          ) : null}
        </AriaListBoxItem>
      )}
    </AriaListBox>
  );
}
