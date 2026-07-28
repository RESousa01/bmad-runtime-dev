// @vitest-environment jsdom
import "../../test/setup";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { GeneralSection } from "./GeneralSection";

describe("GeneralSection", () => {
  it("names its pane with the heading it points at", () => {
    const { container } = render(<GeneralSection runtimeLabel="Local host ready" />);

    const pane = container.querySelector("section");
    const title = document.getElementById(pane?.getAttribute("aria-labelledby") ?? "");
    expect(title?.tagName).toBe("H3");
    expect(title?.textContent).toBe("General");
  });

  it("reports the runtime the host supplied", () => {
    render(<GeneralSection runtimeLabel="Browser demo" />);

    const row = screen.getByText("Desktop runtime").closest(".settings-row");
    expect(row?.textContent).toContain("Browser demo");
  });

  /// This pane is where a user goes to check provenance. It used to claim "The
  /// signed host owns workspace authority" while nothing in the product
  /// verifies a code signature and the binary is not signed at all.
  it("does not claim a code signature it never verifies", () => {
    render(<GeneralSection runtimeLabel="Local host ready" />);

    expect(screen.getByText(/does not verify this build's code signature/u)).toBeTruthy();
    expect(document.body.textContent).not.toContain("The signed host owns");
  });

  it("says attached files are sent explicitly, with no second review step", () => {
    render(<GeneralSection runtimeLabel="Local host ready" />);

    expect(screen.getByText("Only explicitly added files are sent.")).toBeTruthy();
    expect(screen.queryByText("Confirm once")).toBeNull();
    expect(screen.queryByText(/need one confirmation/iu)).toBeNull();
  });

  it("offers nothing to press, because nothing here is changeable", () => {
    render(<GeneralSection runtimeLabel="Local host ready" />);

    expect(screen.queryAllByRole("button")).toHaveLength(0);
  });
});
