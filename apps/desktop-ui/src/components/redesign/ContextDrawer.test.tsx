// @vitest-environment jsdom
import "../../test/setup";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  ContextDrawer,
  type ContextDrawerKind,
} from "./ContextDrawer";

const drawerTitles = {
  files: "Files",
  changes: "Changes",
  "run-details": "Run details",
  methods: "Skills and agents",
} satisfies Record<ContextDrawerKind, string>;

const drawerCases = Object.entries(drawerTitles) as Array<
  [ContextDrawerKind, string]
>;

afterEach(cleanup);

describe("ContextDrawer", () => {
  it.each(drawerCases)("names the %s desktop pane %s", (kind, title) => {
    render(<ContextDrawer kind={kind} onClose={vi.fn()} />);

    const drawer = screen.getByRole("complementary", { name: title });
    expect(within(drawer).getByRole("heading", { name: title })).toBeTruthy();
    expect(drawer.getAttribute("aria-modal")).toBeNull();
  });

  it("composes already-wired feature content without changing it", () => {
    render(
      <ContextDrawer kind="files" onClose={vi.fn()}>
        <section aria-label="Workspace file projection">Authenticated files</section>
      </ContextDrawer>,
    );

    expect(
      screen.getByRole("region", { name: "Workspace file projection" }).textContent,
    ).toBe("Authenticated files");
  });

  it("calls the close callback once from its single close control", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(<ContextDrawer kind="changes" onClose={onClose} />);

    const drawer = screen.getByRole("complementary", { name: "Changes" });
    const buttons = within(drawer).getAllByRole("button");
    expect(buttons).toHaveLength(1);

    await user.click(within(drawer).getByRole("button", { name: "Close Changes" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("renders a named modal dialog when presented as an overlay", () => {
    render(
      <ContextDrawer
        kind="run-details"
        onClose={vi.fn()}
        presentation="overlay"
      />,
    );

    const dialog = screen.getByRole("dialog", { name: "Run details" });
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(screen.queryByRole("complementary")).toBeNull();
  });

  it("does not recreate Inspector tabs or a fifth context destination", () => {
    render(<ContextDrawer kind="methods" onClose={vi.fn()} />);

    const drawer = screen.getByRole("complementary", { name: "Skills and agents" });
    expect(within(drawer).queryByRole("tablist")).toBeNull();
    expect(within(drawer).queryByRole("tab")).toBeNull();
    expect(
      within(drawer).getAllByRole("heading").map((heading) => heading.textContent),
    ).toEqual(["Skills and agents"]);

    for (const legacyLabel of ["Inspector", "Context", "Logs", "Evidence", "Method"]) {
      expect(within(drawer).queryByText(legacyLabel)).toBeNull();
    }
  });

  it("has no opening side effect", () => {
    const onClose = vi.fn();
    render(<ContextDrawer kind="files" onClose={onClose} />);

    expect(onClose).not.toHaveBeenCalled();
  });
});
