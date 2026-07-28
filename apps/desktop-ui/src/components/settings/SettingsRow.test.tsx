// @vitest-environment jsdom
import "../../test/setup";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { SettingsRow } from "./SettingsRow";

describe("SettingsRow", () => {
  it("keeps the label and its value inside one .settings-row", () => {
    render(<SettingsRow detail="What it means." label="UI build" value="2026-07-28T00:00:00Z" />);

    // The relationship five sections and the About test depend on: a lookup by
    // label has to be able to reach the value from the row, which only works
    // while both are inside the same `.settings-row`. Asserting the label and
    // the value exist separately would pass even if they were split across two
    // rows.
    const row = screen.getByText("UI build").closest(".settings-row");
    expect(row).not.toBeNull();
    expect(row?.textContent).toContain("2026-07-28T00:00:00Z");
    expect(row?.textContent).toContain("What it means.");
  });

  it("distinguishes the value from the label and the explanation", () => {
    render(<SettingsRow detail="Detail text" label="Label text" value="Value text" />);

    const row = screen.getByText("Label text").closest(".settings-row");
    expect(row?.querySelector("strong")?.textContent).toBe("Label text");
    expect(row?.querySelector("small")?.textContent).toBe("Detail text");
    expect(row?.querySelector("em")?.textContent).toBe("Value text");
  });
});
