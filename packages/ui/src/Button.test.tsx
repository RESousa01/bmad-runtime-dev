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

  // FE-04 acceptance: Space and Enter each activate exactly once. Both are
  // asserted because a control that fires on keydown *and* on the synthesised
  // click is the classic double-submit, and it only shows up from the keyboard.
  it("activates exactly once on Enter and exactly once on Space", async () => {
    const user = userEvent.setup();
    const onPress = vi.fn();
    render(<Button onPress={onPress}>Send</Button>);

    await user.tab();
    expect(screen.getByRole("button", { name: "Send" })).toBe(document.activeElement);

    await user.keyboard("{Enter}");
    expect(onPress).toHaveBeenCalledTimes(1);

    await user.keyboard(" ");
    expect(onPress).toHaveBeenCalledTimes(2);
  });

  // FE-04 acceptance: pending controls cannot invoke actions. Pending is not a
  // synonym for disabled -- it says the work is already happening -- so it is
  // announced with aria-busy while still refusing the press.
  it("cannot be invoked while pending, and says why", async () => {
    const user = userEvent.setup();
    const onPress = vi.fn();
    render(<Button isPending onPress={onPress}>Send</Button>);

    const button = screen.getByRole("button", { name: "Send" });
    expect(button.getAttribute("aria-busy")).toBe("true");
    expect([...button.classList]).toContain("sapphirus-button--pending");

    await user.click(button);
    expect(onPress).not.toHaveBeenCalled();

    await user.keyboard("{Enter}");
    expect(onPress).not.toHaveBeenCalled();
  });

  it("is not busy when it is merely disabled", () => {
    render(<Button isDisabled>Send</Button>);
    const button = screen.getByRole("button", { name: "Send" });
    // Disabled says "not available to you"; busy would say "already running".
    expect(button.hasAttribute("aria-busy")).toBe(false);
  });
});
