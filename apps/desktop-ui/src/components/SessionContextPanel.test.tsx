import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { SessionContextPanel, type SessionContextPanelProps } from "./SessionContextPanel";
import type { ModelContextState } from "../features/model-context/modelContextTypes";

afterEach(cleanup);

function props(
  overrides: Partial<SessionContextPanelProps> = {},
): SessionContextPanelProps {
  return { messages: [], sessionTitle: "A task", ...overrides };
}

/// A measurement the host reported for an assembled request.
function measured(
  inputTokens: number,
  windowTokens: number | null,
  kind: "measured" | "stale" = "measured",
): ModelContextState {
  return {
    kind,
    snapshot: {
      inputTokens,
      measuredAt: 1_784_731_394_657,
      key: {
        modelRoute: "openai/gpt-5",
        promptEpoch: 1,
        taskId: "task-1",
        workspaceGrantEpoch: 1,
        workspaceId: "workspace-1",
      },
      windowTokens,
    },
  };
}

/** The value rendered beside a stat label. */
function statValue(label: string): string | undefined {
  const cell = screen.getByText(label);
  return cell.parentElement?.textContent?.slice(label.length);
}

/**
 * A number as the panel would render it.
 *
 * Built with the same formatter rather than written out, because the grouping
 * separator and whether four digits are grouped at all are decided by the
 * environment's locale. Hardcoding "1,024" asserts the test machine is en-US,
 * which is not the behaviour under test and fails on a machine that is not.
 */
function shown(value: number): string {
  return new Intl.NumberFormat().format(value);
}

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

/**
 * The panel and the composer's Context surface read one projection.
 *
 * This panel used to answer "how much context is in use" for itself, against a
 * constant 65,536 that was the workspace broker's file-read cap. Two
 * independently-written answers to one question is how two surfaces come to
 * contradict each other about the same session, so these assert the panel
 * takes its verdict from `projectContextUsage` rather than re-deriving one.
 */
describe("context usage", () => {
  it("never states a window the host did not give it", () => {
    // The host exposes no model context window anywhere. Absent is the
    // truthful state, and a placeholder in that slot reads like a measurement.
    render(<SessionContextPanel {...props({ inputTokens: 1_024 })} />);

    expect(screen.queryByText("Local Review Budget")).toBeNull();
    expect(screen.queryByText("Review Budget Used")).toBeNull();
    expect(screen.queryByText("Model Context Window")).toBeNull();
    expect(screen.queryByText("Context Used")).toBeNull();
  });

  it("shows the window and the fraction once a measurement carries one", () => {
    render(<SessionContextPanel {...props({
      inputTokens: 1_024,
      modelContext: measured(50_000, 200_000),
    })} />);

    expect(statValue("Model Context Window")).toBe(shown(200_000));
    expect(statValue("Context Used")).toBe("25.0%");
  });

  it("calls an unmeasured figure an estimate", () => {
    render(<SessionContextPanel {...props({ inputTokens: 1_024 })} />);

    expect(statValue("Input Estimate")).toBe(shown(1_024));
    expect(screen.queryByText("Input (reported)")).toBeNull();
  });

  it("stops calling it an estimate once the host has measured it", () => {
    // The same label over a measured figure would be a lie, and the row is the
    // only place the user learns which kind of number they are reading.
    render(<SessionContextPanel {...props({
      inputTokens: 1_024,
      modelContext: measured(4_096, null),
    })} />);

    expect(statValue("Input (reported)")).toBe(shown(4_096));
    expect(screen.queryByText("Input Estimate")).toBeNull();
  });

  /// A measurement the route has moved out from under is still the best number
  /// available. Its age is what decides whether to trust it, so the age is
  /// what the label carries -- not a blank, and not a bare "reported".
  it("says a measurement is the last one when it has gone stale", () => {
    render(<SessionContextPanel {...props({
      inputTokens: 1_024,
      modelContext: measured(4_096, null, "stale"),
    })} />);

    expect(statValue("Input (last reported)")).toBe(shown(4_096));
  });

  it("supersedes the local estimate rather than adding to it", () => {
    // The host measures the whole assembled request, attachments included, so
    // summing the byte-derived figure on top counts the same files twice.
    render(<SessionContextPanel {...props({
      inputTokens: 1_024,
      modelContext: measured(4_096, null),
      outputTokens: 500,
    })} />);

    expect(statValue("Total Tokens")).toBe(shown(4_596));
  });

  it("marks the total as estimated only while the input half is", () => {
    render(<SessionContextPanel {...props({ inputTokens: 1_024, outputTokens: 500 })} />);
    expect(statValue("Total Tokens (input estimated)")).toBe(shown(1_524));

    cleanup();
    render(<SessionContextPanel {...props({
      inputTokens: 1_024,
      modelContext: measured(4_096, null),
      outputTokens: 500,
    })} />);
    expect(screen.queryByText("Total Tokens (input estimated)")).toBeNull();
  });

  it("says nothing rather than zero when there is no figure at all", () => {
    render(<SessionContextPanel {...props()} />);

    expect(statValue("Total Tokens")).toBe("—");
    expect(statValue("Input Estimate")).toBe("—");
  });

  it("refuses a window of zero rather than dividing by it", () => {
    // `windowFraction` owns that judgement; the panel must not second-guess it
    // by rendering a fraction the projection declined to compute.
    render(<SessionContextPanel {...props({
      inputTokens: 1_024,
      modelContext: measured(4_096, 0),
    })} />);

    expect(screen.queryByText("Model Context Window")).toBeNull();
    expect(screen.queryByText("Context Used")).toBeNull();
  });
});
