import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { Checkbox } from "./Checkbox";

/// The rendered control is a visually hidden `<input type="checkbox">` inside a
/// `<label>`; the accessible name comes from the label's text, not from any
/// attribute on the input.
function checkbox(): HTMLInputElement {
  return screen.getByRole("checkbox", {
    name: "Include workspace context",
  }) as HTMLInputElement;
}

describe("Checkbox", () => {
  it("names the control from its children rather than the decorative box", () => {
    render(<Checkbox>Include workspace context</Checkbox>);

    // The painted box carries `aria-hidden`, so if the label span were ever
    // dropped the control would be exposed with no accessible name at all.
    expect(checkbox().checked).toBe(false);
    expect(
      screen.getByText("Include workspace context").closest("label")
        ?.querySelector(".sapphirus-checkbox__box")
        ?.getAttribute("aria-hidden"),
    ).toBe("true");
  });

  it("toggles when activated", async () => {
    const onChange = vi.fn();
    render(<Checkbox onChange={onChange}>Include workspace context</Checkbox>);

    await userEvent.click(checkbox());

    expect(onChange).toHaveBeenCalledWith(true);
  });

  it("reflects a controlled selection", () => {
    render(
      <Checkbox isSelected onChange={() => undefined}>
        Include workspace context
      </Checkbox>,
    );

    expect(checkbox().checked).toBe(true);
  });

  it("does not toggle while disabled", async () => {
    const onChange = vi.fn();
    render(
      <Checkbox isDisabled onChange={onChange}>
        Include workspace context
      </Checkbox>,
    );

    const control = checkbox();
    expect(control.disabled).toBe(true);
    await userEvent.click(control);

    expect(onChange).not.toHaveBeenCalled();
  });

  it("appends a caller class to the design-system class", () => {
    render(<Checkbox className="context-drawer__toggle">Include workspace context</Checkbox>);

    const label = screen.getByText("Include workspace context").closest("label");
    const classes = [...(label?.classList ?? [])];
    expect(classes).toContain("sapphirus-checkbox");
    expect(classes).toContain("context-drawer__toggle");
  });
});
