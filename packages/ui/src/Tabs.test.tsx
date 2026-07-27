import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { Tab, TabList, TabPanel, Tabs } from "./Tabs";

function Fixture() {
  return (
    <Tabs>
      <TabList aria-label="Review lanes">
        <Tab id="changes">Changes</Tab>
        <Tab id="history">History</Tab>
      </TabList>
      <TabPanel id="changes">Pending change set</TabPanel>
      <TabPanel id="history">Earlier decisions</TabPanel>
    </Tabs>
  );
}

describe("Tabs", () => {
  it("exposes a named tablist and selects the first tab by default", () => {
    render(<Fixture />);

    const tablist = screen.getByRole("tablist", { name: "Review lanes" });
    expect([...tablist.classList]).toContain("sapphirus-tab-list");
    expect(screen.getByRole("tab", { name: "Changes" }).getAttribute("aria-selected")).toBe(
      "true",
    );
    expect(screen.getByRole("tab", { name: "History" }).getAttribute("aria-selected")).toBe(
      "false",
    );
  });

  it("renders only the selected panel", () => {
    render(<Fixture />);

    expect(screen.getByRole("tabpanel").textContent).toBe("Pending change set");
    expect(screen.queryByText("Earlier decisions")).toBeNull();
  });

  it("switches panels when another tab is chosen", async () => {
    render(<Fixture />);

    await userEvent.click(screen.getByRole("tab", { name: "History" }));

    expect(screen.getByRole("tab", { name: "History" }).getAttribute("aria-selected")).toBe(
      "true",
    );
    expect(screen.getByRole("tabpanel").textContent).toBe("Earlier decisions");
  });

  it("moves selection with the arrow keys", async () => {
    // A tablist uses a roving tabindex, so the arrow keys are the only way to
    // reach a tab that is not currently selected.
    render(<Fixture />);

    await userEvent.tab();
    await userEvent.keyboard("{ArrowRight}");

    expect(screen.getByRole("tab", { name: "History" }).getAttribute("aria-selected")).toBe(
      "true",
    );
  });

  it("carries the design-system classes onto the tab and the panel", () => {
    render(<Fixture />);

    expect([...screen.getByRole("tab", { name: "Changes" }).classList]).toContain(
      "sapphirus-tab",
    );
    expect([...screen.getByRole("tabpanel").classList]).toContain("sapphirus-tab-panel");
  });
});
