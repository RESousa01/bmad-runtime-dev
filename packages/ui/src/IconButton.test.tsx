import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { IconButton } from "./IconButton";

const Glyph = () => <svg aria-hidden="true" data-testid="glyph" />;

describe("IconButton", () => {
  it("names itself, because an icon is not a name", () => {
    render(<IconButton label="Close settings"><Glyph /></IconButton>);
    const button = screen.getByRole("button", { name: "Close settings" });
    expect(button).toBeTruthy();
    // The glyph must not contribute to the name, or a screen reader would
    // announce the label twice or read the SVG's own content.
    expect(screen.getByTestId("glyph").getAttribute("aria-hidden")).toBe("true");
  });

  it("uses the same string for the tooltip and the accessible name", () => {
    render(<IconButton label="Add folder"><Glyph /></IconButton>);
    const button = screen.getByRole("button", { name: "Add folder" });
    expect(button.getAttribute("title")).toBe("Add folder");
  });

  it("drops the native tooltip when a richer one is supplied, keeping the name", () => {
    render(
      <IconButton label="Add folder" suppressNativeTooltip><Glyph /></IconButton>,
    );
    const button = screen.getByRole("button", { name: "Add folder" });
    expect(button.getAttribute("title")).toBeNull();
  });

  it("activates once on Enter and once on Space", async () => {
    const user = userEvent.setup();
    const onPress = vi.fn();
    render(<IconButton label="Send" onPress={onPress}><Glyph /></IconButton>);

    await user.tab();
    expect(screen.getByRole("button", { name: "Send" })).toBe(document.activeElement);

    await user.keyboard("{Enter}");
    expect(onPress).toHaveBeenCalledTimes(1);

    await user.keyboard(" ");
    expect(onPress).toHaveBeenCalledTimes(2);
  });

  it("cannot be invoked while pending", async () => {
    const user = userEvent.setup();
    const onPress = vi.fn();
    render(<IconButton isPending label="Send" onPress={onPress}><Glyph /></IconButton>);

    const button = screen.getByRole("button", { name: "Send" });
    expect(button.getAttribute("aria-busy")).toBe("true");
    await user.click(button);
    expect(onPress).not.toHaveBeenCalled();
  });
});
