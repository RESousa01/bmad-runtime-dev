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

    /*
     * The budget rows are gone, and this test used to require them.
     *
     * "Local Review Budget" was a constant 65,536 -- the workspace broker's
     * file-read cap -- and "Review Budget Used" was this session's tokens over
     * it. Neither is a property of the model, so the percentage answered a
     * question nobody asked while looking exactly like the answer to the one
     * they did. A test named "labels local estimates honestly" was pinning the
     * opposite.
     */
    expect(screen.queryByText("Local Review Budget")).toBeNull();
    expect(screen.queryByText("Review Budget Used")).toBeNull();
    expect(screen.queryByText("Context Limit")).toBeNull();

    // What survives says where its numbers came from. The sum inherits the
    // estimate-ness of its parts rather than laundering it into a measurement.
    expect(screen.getByText("Input Estimate")).toBeTruthy();
    expect(screen.getByText("Total Tokens (input estimated)")).toBeTruthy();
    expect(screen.getByText("Total Cost").parentElement?.textContent).toBe("Total Cost—");
    expect(screen.getByText("Raw messages")).toBeTruthy();
    expect(screen.getByText("Greeting and quick check-in")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /user.*msg_user_01/i }));
    expect(screen.getByText(/Review this workspace/u)).toBeTruthy();
  });
});
