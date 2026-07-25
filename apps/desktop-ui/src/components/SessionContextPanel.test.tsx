import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { SessionContextPanel } from "./SessionContextPanel";

afterEach(cleanup);

describe("SessionContextPanel", () => {
  it("labels local estimates honestly and leaves unreported cost unknown", () => {
    render(
      <SessionContextPanel
        createdAt={1_784_731_394_657}
        inputTokens={1_024}
        localReviewBudget={65_536}
        messages={[
          {
            id: "msg_user_01",
            role: "user",
            createdAt: 1_784_731_394_657,
            value: { message: { role: "user" }, parts: [{ type: "text", text: "Review this workspace" }] },
          },
        ]}
        model="Deterministic development"
        provider="Forge BMAD"
        sessionTitle="Greeting and quick check-in"
      />,
    );

    expect(screen.getByText("Local Review Budget")).toBeTruthy();
    expect(screen.getByText("Review Budget Used")).toBeTruthy();
    expect(screen.getByText("Input Estimate")).toBeTruthy();
    expect(screen.queryByText("Context Limit")).toBeNull();
    expect(screen.getByText("Total Cost").parentElement?.textContent).toBe("Total Cost—");
    expect(screen.getByText("Raw messages")).toBeTruthy();
    expect(screen.getByText("Greeting and quick check-in")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /user.*msg_user_01/i }));
    expect(screen.getByText(/Review this workspace/u)).toBeTruthy();
  });
});
