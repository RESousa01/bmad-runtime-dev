import type { ReactNode } from "react";
import {
  Checkbox as AriaCheckbox,
  type CheckboxProps as AriaCheckboxProps,
} from "react-aria-components";

export interface CheckboxProps extends Omit<AriaCheckboxProps, "children" | "className"> {
  children: ReactNode;
  className?: string;
}

export function Checkbox({ children, className = "", ...props }: CheckboxProps) {
  return (
    <AriaCheckbox className={`sapphirus-checkbox ${className}`} {...props}>
      <span aria-hidden="true" className="sapphirus-checkbox__box" />
      <span className="sapphirus-checkbox__label">{children}</span>
    </AriaCheckbox>
  );
}
