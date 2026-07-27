import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { Button } from "./Button";

describe("Button", () => {
  it("renders a native button carrying the default variant and size classes", () => {
    render(<Button>Approve</Button>);

    const button = screen.getByRole("button", { name: "Approve" });
    expect(button.tagName).toBe("BUTTON");
    expect([...button.classList]).toEqual([
      "sapphirus-button",
      "sapphirus-button--secondary",
      "sapphirus-button--medium",
    ]);
  });

  it("applies the requested variant and size", () => {
    render(
      <Button size="icon" variant="danger">
        Discard
      </Button>,
    );

    const button = screen.getByRole("button", { name: "Discard" });
    expect([...button.classList]).toContain("sapphirus-button--danger");
    expect([...button.classList]).toContain("sapphirus-button--icon");
  });

  it("appends a caller class instead of replacing the design-system classes", () => {
    // A caller class that replaced the base classes would silently strip the
    // token-driven styling, so the component joins rather than overwrites.
    render(<Button className="task-header__action">Rename</Button>);

    const classes = [...screen.getByRole("button", { name: "Rename" }).classList];
    expect(classes).toContain("sapphirus-button");
    expect(classes).toContain("task-header__action");
  });

  it("emits a press when activated", async () => {
    const onPress = vi.fn();
    render(<Button onPress={onPress}>Send</Button>);

    await userEvent.click(screen.getByRole("button", { name: "Send" }));

    expect(onPress).toHaveBeenCalledTimes(1);
  });

  it("does not emit a press while disabled", async () => {
    const onPress = vi.fn();
    render(
      <Button isDisabled onPress={onPress}>
        Send
      </Button>,
    );

    const button = screen.getByRole("button", { name: "Send" });
    expect(button.hasAttribute("disabled")).toBe(true);
    await userEvent.click(button);

    expect(onPress).not.toHaveBeenCalled();
  });
});
