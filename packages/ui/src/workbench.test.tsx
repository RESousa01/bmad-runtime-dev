import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Button } from "./Button";
import { EmptyState } from "./EmptyState";
import { InlineMessage } from "./InlineMessage";
import { Progress } from "./Progress";
import { StatusText } from "./StatusText";
import { Toolbar } from "./Toolbar";

describe("Toolbar", () => {
  it("moves between its controls with the arrow keys", async () => {
    const user = userEvent.setup();
    render(
      <>
        <Toolbar label="Task actions">
          <Button>Review</Button>
          <Button>Rename</Button>
          <Button>Delete</Button>
        </Toolbar>
        <Button>After</Button>
      </>,
    );

    await user.tab();
    expect(screen.getByRole("button", { name: "Review" })).toBe(document.activeElement);

    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Rename" })).toBe(document.activeElement);

    await user.keyboard("{ArrowLeft}");
    expect(screen.getByRole("button", { name: "Review" })).toBe(document.activeElement);

    /*
     * FE-07 acceptance: no surface invents its own toolbar behaviour.
     *
     * This asserts what the primitive actually provides -- arrow navigation
     * and a named group -- and deliberately does not assert a roving tabindex.
     * react-aria-components applies that in collection components, not here,
     * so every control stays individually tabbable. An earlier version of this
     * test claimed one tab stop and failed, which is the useful outcome: the
     * doc comment had promised something the component did not do.
     */
    const stillTabbable = ["Review", "Rename", "Delete"]
      .map((name) => screen.getByRole("button", { name }))
      .filter((button) => button.getAttribute("tabindex") !== "-1");
    expect(stillTabbable).toHaveLength(3);
  });

  it("names itself", () => {
    render(<Toolbar label="Task actions"><Button>Review</Button></Toolbar>);
    expect(screen.getByRole("toolbar", { name: "Task actions" })).toBeTruthy();
  });
});

describe("InlineMessage", () => {
  it("interrupts only for an error", () => {
    const { rerender } = render(<InlineMessage title="Saved" tone="success" />);
    // A success that interrupts a screen reader is how one becomes unusable.
    expect(screen.getByRole("status")).toBeTruthy();

    rerender(<InlineMessage title="Apply failed" tone="error" />);
    expect(screen.getByRole("alert")).toBeTruthy();
  });

  it("says the tone in words, not only in colour", () => {
    render(<InlineMessage title="Approval required" tone="warning" />);
    // Specification 3.5: colour is never the only carrier.
    expect(screen.getByText("warning")).toBeTruthy();
  });
});

describe("Progress", () => {
  it("reports a known value", () => {
    render(<Progress label="Applying changes" value={40} />);
    const bar = screen.getByRole("progressbar", { name: "Applying changes" });
    expect(bar.getAttribute("aria-valuenow")).toBe("40");
  });

  it("is indeterminate rather than stuck at zero when the end is unknown", () => {
    render(<Progress label="Working" />);
    const bar = screen.getByRole("progressbar", { name: "Working" });
    // A bar sitting at 0% reads as broken; absent valuenow reads as unknown.
    expect(bar.hasAttribute("aria-valuenow")).toBe(false);
  });
});

describe("StatusText", () => {
  it("cannot render a bare coloured dot", () => {
    render(<StatusText tone="active">Running</StatusText>);
    // `children` is a required string, so the word always accompanies the dot.
    expect(screen.getByText("Running")).toBeTruthy();
  });
});

describe("EmptyState", () => {
  it("is a heading and a sentence, with the action that resolves it", () => {
    render(
      <EmptyState
        action={<Button>New task</Button>}
        description="Describe what you want to accomplish."
        title="What do you want to build?"
      />,
    );
    expect(screen.getByRole("heading", { name: "What do you want to build?" })).toBeTruthy();
    expect(screen.getByText("Describe what you want to accomplish.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "New task" })).toBeTruthy();
  });
});
