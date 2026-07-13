import type { ReactNode } from "react";
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
}

export function Button({
  children,
  className = "",
  size = "medium",
  variant = "secondary",
  ...props
}: ButtonProps) {
  const classes = [
    "sapphirus-button",
    `sapphirus-button--${variant}`,
    `sapphirus-button--${size}`,
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <AriaButton className={classes} {...props}>
      {children}
    </AriaButton>
  );
}
