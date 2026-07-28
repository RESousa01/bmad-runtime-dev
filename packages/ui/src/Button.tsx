import { useCallback, type ReactNode } from "react";
import {
  Button as AriaButton,
  type ButtonProps as AriaButtonProps,
} from "react-aria-components";

type ButtonVariant = "primary" | "secondary" | "quiet" | "danger";
type ButtonSize = "small" | "medium" | "large" | "icon";

export interface ButtonProps extends Omit<AriaButtonProps, "className" | "children"> {
  children: ReactNode;
  className?: string;
  size?: ButtonSize;
  title?: string;
  variant?: ButtonVariant;
  /**
   * The action is already running.
   *
   * A pending control must not invoke again -- a second press during an
   * in-flight request is the double-submit this exists to prevent -- so this
   * refuses activation as well as announcing it. It is separate from
   * `isDisabled` because the two mean different things to a reader: disabled
   * says "not available to you", pending says "already happening", and
   * `aria-busy` is what carries the difference.
   */
  isPending?: boolean;
}

export function Button({
  children,
  className = "",
  size = "medium",
  variant = "secondary",
  isPending = false,
  isDisabled = false,
  title,
  ...props
}: ButtonProps) {
  const classes = [
    "sapphirus-button",
    `sapphirus-button--${variant}`,
    `sapphirus-button--${size}`,
    isPending ? "sapphirus-button--pending" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  /*
   * react-aria-components filters the props it forwards to the DOM: `data-*`,
   * `class`, `type`, `tabindex`, `aria-label` and `id` survive, and everything
   * else is dropped. `title` and `aria-busy` are both dropped, which is why
   * `title` has never reached the DOM despite being declared on this interface
   * since the component was written.
   *
   * Setting them on the node is the reliable route. The callback identity
   * changes with the values, so React re-runs it when either moves rather than
   * only on mount.
   */
  const applyFilteredAttributes = useCallback(
    (node: HTMLButtonElement | null) => {
      if (!node) return;
      if (isPending) node.setAttribute("aria-busy", "true");
      else node.removeAttribute("aria-busy");
      if (title === undefined) node.removeAttribute("title");
      else node.setAttribute("title", title);
    },
    [isPending, title],
  );

  return (
    <AriaButton
      className={classes}
      isDisabled={isDisabled || isPending}
      ref={applyFilteredAttributes}
      {...props}
    >
      {children}
    </AriaButton>
  );
}
