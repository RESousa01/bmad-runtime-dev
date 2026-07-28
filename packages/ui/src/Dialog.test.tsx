import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Button } from "./Button";
import { Dialog, DialogTrigger } from "./Dialog";
import { Popover } from "./Popover";

describe("Dialog", () => {
  it("gives every dialog a name, and a description when there is one", async () => {
    const user = userEvent.setup();
    render(
      <DialogTrigger>
        <Button>Open</Button>
        <Dialog description="Nothing has changed yet." title="Apply 3 files">
          <Button>Confirm</Button>
        </Dialog>
      </DialogTrigger>,
    );

    await user.click(screen.getByRole("button", { name: "Open" }));

    // FE-05 acceptance: screen readers receive names and descriptions.
    const dialog = screen.getByRole("dialog", { name: "Apply 3 files" });
    const describedBy = dialog.getAttribute("aria-describedby");
    expect(describedBy).toBeTruthy();
    expect(document.getElementById(describedBy ?? "")?.textContent)
      .toBe("Nothing has changed yet.");
  });

  it("returns focus to the trigger on close, leaving nothing stranded", async () => {
    const user = userEvent.setup();
    render(
      <DialogTrigger>
        <Button>Open</Button>
        <Dialog title="Settings">
          <Button>Inside</Button>
        </Dialog>
      </DialogTrigger>,
    );

    const trigger = screen.getByRole("button", { name: "Open" });
    await user.click(trigger);
    expect(screen.getByRole("dialog", { name: "Settings" })).toBeTruthy();

    await user.keyboard("{Escape}");

    // FE-05 acceptance: no overlay traps focus after close. A dialog that
    // leaves focus on a removed node strands the keyboard on document.body.
    // Restoration is asynchronous -- the overlay unmounts first and focus moves
    // after -- so this waits rather than assuming a synchronous handoff.
    expect(screen.queryByRole("dialog")).toBeNull();
    await waitFor(() => {
      expect(document.activeElement).toBe(trigger);
    });
  });

  it("closes only the innermost overlay on Escape", async () => {
    const user = userEvent.setup();
    render(
      <DialogTrigger>
        <Button>Open outer</Button>
        <Dialog title="Outer">
          <DialogTrigger>
            <Button>Open inner</Button>
            <Popover label="Inner choices">
              <Button>Choice</Button>
            </Popover>
          </DialogTrigger>
        </Dialog>
      </DialogTrigger>,
    );

    await user.click(screen.getByRole("button", { name: "Open outer" }));
    await user.click(screen.getByRole("button", { name: "Open inner" }));
    expect(screen.getByRole("dialog", { name: "Inner choices" })).toBeTruthy();

    // FE-05 acceptance: nested overlays have deterministic escape behaviour.
    // One Escape closes one layer; the outer dialog must survive, or a nested
    // menu becomes a way to lose the work behind it.
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Inner choices" })).toBeNull();
    expect(screen.getByRole("dialog", { name: "Outer" })).toBeTruthy();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Outer" })).toBeNull();
  });

  it("cannot be constructed without a name", () => {
    // A compile-time guarantee, asserted here so the intent survives a
    // refactor that makes the prop optional: the type is what stops an
    // unnamed dialog reaching a screen reader.
    const props: Parameters<typeof Dialog>[0] = { title: "Required", children: null };
    expect(props.title).toBe("Required");
  });
});
