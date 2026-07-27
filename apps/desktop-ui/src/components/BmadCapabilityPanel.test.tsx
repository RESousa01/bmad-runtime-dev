// @vitest-environment jsdom
import "../test/setup";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { describe, expect, it, vi } from "vitest";
import {
  BmadCapabilityPanel,
  type BmadCapabilityPanelProps,
  type CapabilityRunPhase,
} from "./BmadCapabilityPanel";

const manifestHash = `sha256:${"a".repeat(64)}`;

/// Far enough out that no session could outlive it, so the panel arms no timer
/// and the "still live" branches are deterministic.
const LIVE_UNTIL = 2_000_000_000_000;
/// Already gone. The fixtures used to carry a 2024 timestamp for a deadline the
/// panel never read, which is how an expired consent kept an enabled Send.
const ALREADY_LAPSED = 1_725_000_300_000;

const review = {
  capabilityId: "bmm:bmad-product-brief",
  runId: "caprun_01ARZ3NDEKTSV4RRFFQ69G5FAV",
  manifestHash,
  expiresAt: LIVE_UNTIL,
  architecture: null,
};

const approved = {
  capabilityId: "bmm:bmad-product-brief",
  manifestHash,
  decisionId: "decision_01ARZ3NDEKTSV4RRFFQ69G5FAV",
  expiresAt: LIVE_UNTIL,
};

function createProps(phase: CapabilityRunPhase): BmadCapabilityPanelProps {
  return {
    capabilityId: "bmm:bmad-product-brief",
    capabilityLabel: "Create Brief",
    destinationLabel: "Deterministic (local)",
    onApprove: vi.fn(),
    onCancel: vi.fn(),
    onClose: vi.fn(),
    onPrepare: vi.fn(),
    onSubmit: vi.fn(),
    phase,
  };
}

async function expectAccessible(container: HTMLElement) {
  const results = await axe.run(container, {
    rules: { "color-contrast": { enabled: false } },
  });
  expect(results.violations).toEqual([]);
}

describe("BmadCapabilityPanel", () => {
  it("prepares only deduplicated non-empty context paths", async () => {
    const props = createProps({ kind: "selecting" });
    const user = userEvent.setup();
    const { container } = render(<BmadCapabilityPanel {...props} />);

    const prepare = screen.getByRole("button", { name: /prepare reviewed context/i });
    expect(prepare.hasAttribute("disabled")).toBe(true);

    await user.type(
      screen.getByRole("textbox", { name: "Context file paths" }),
      "docs/brief.md\n\n docs/brief.md \nnotes/context.md",
    );
    expect(prepare.hasAttribute("disabled")).toBe(false);
    await user.click(prepare);
    expect(props.onPrepare).toHaveBeenCalledWith(["docs/brief.md", "notes/context.md"]);
    await expectAccessible(container);
  });

  it("shows the exact reviewed context and consent disclosure before approval", async () => {
    const props = createProps({
      kind: "review",
      review,
      contextPaths: ["docs/brief.md"],
    });
    const user = userEvent.setup();
    const { container } = render(<BmadCapabilityPanel {...props} />);

    expect(screen.getByText("docs/brief.md")).toBeTruthy();
    expect(screen.getByText(manifestHash)).toBeTruthy();
    expect(
      screen.getByText(/Only the exact reviewed context shown here will be sent once/u),
    ).toBeTruthy();
    await user.click(
      screen.getByRole("button", { name: /approve this exact context/i }),
    );
    expect(props.onApprove).toHaveBeenCalledWith(manifestHash);
    await expectAccessible(container);
  });

  it("sends once or cancels from the approved state", async () => {
    const props = createProps({
      kind: "approved",
      review,
      approved,
      contextPaths: ["docs/brief.md"],
      sending: false,
    });
    const user = userEvent.setup();
    const { container } = render(<BmadCapabilityPanel {...props} />);

    await user.click(screen.getByRole("button", { name: /send once and run/i }));
    expect(props.onSubmit).toHaveBeenCalledWith(manifestHash, approved.decisionId);
    await user.click(screen.getByRole("button", { name: /cancel consent/i }));
    expect(props.onCancel).toHaveBeenCalledWith(manifestHash, approved.decisionId);
    await expectAccessible(container);
  });

  it("renders a completed document artifact without action controls", async () => {
    const resultJson = JSON.stringify({
      resultKind: "document_artifact",
      documentArtifact: {
        schemaVersion: "sapphirus.bmad-document-artifact.v1",
        title: "Product brief: example",
        sections: [{ heading: "Problem", body: "A bounded problem statement." }],
        evidenceRefs: [],
        openQuestions: ["Which launch market?"],
      },
    });
    const props = createProps({
      kind: "completed",
      completed: {
        capabilityId: "bmm:bmad-product-brief",
        runId: review.runId,
        resultKind: "document_artifact",
        architecture: null,
      },
      resultJson,
    });
    const { container } = render(<BmadCapabilityPanel {...props} />);

    expect(screen.getByRole("heading", { name: "Product brief: example" })).toBeTruthy();
    expect(screen.getByText("A bounded problem statement.")).toBeTruthy();
    expect(screen.getByText("Which launch market?")).toBeTruthy();
    // No apply/install/execute affordances in a completed artifact.
    expect(document.body.textContent).not.toMatch(
      /\b(?:Apply|Install|Execute|Activate)\b/u,
    );
    await expectAccessible(container);
  });

  it("routes governed change sets to the Changes review, never an apply button", async () => {
    const props = createProps({
      kind: "completed",
      completed: {
        capabilityId: "bmm:bmad-dev-story",
        runId: review.runId,
        resultKind: "governed_change_set",
        architecture: null,
      },
      resultJson: null,
    });
    const { container } = render(<BmadCapabilityPanel {...props} />);

    expect(
      screen.getByRole("heading", { name: /candidate change set produced/i }),
    ).toBeTruthy();
    expect(screen.getByText(/review and approve it in Governed changes/iu)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /apply/i })).toBeNull();
    await expectAccessible(container);
  });

  it("renders builder drafts as inert data with no activation affordance", async () => {
    const props = createProps({
      kind: "completed",
      completed: {
        capabilityId: "builder:agent.analyze",
        runId: review.runId,
        resultKind: "inactive_builder_draft",
        architecture: null,
      },
      resultJson: null,
    });
    const { container } = render(<BmadCapabilityPanel {...props} />);
    expect(
      screen.getByRole("heading", { name: /inactive draft produced/i }),
    ).toBeTruthy();
    expect(document.body.textContent).not.toMatch(
      /\b(?:Install|Activate|Register|Execute|Apply)\b/u,
    );
    expect(screen.queryByRole("button", { name: /install|activate|register/i })).toBeNull();
    await expectAccessible(container);
  });

  it("surfaces errors and stays accessible", async () => {
    const props = createProps({
      kind: "error",
      message: "The reviewed consent expired before it was used.",
    });
    const { container } = render(<BmadCapabilityPanel {...props} />);
    expect(screen.getByRole("alert").textContent).toContain("consent expired");
    await expectAccessible(container);
  });

  /// Regression guard: a failed run used to render the message and a Close
  /// button and nothing else, so the only route onward was to close the panel,
  /// find the capability in the agent list again, and retype every path.
  it("keeps a failed run recoverable by re-preparing the context in place", async () => {
    const props = createProps({ kind: "error", message: "The provider is rate limited." });
    const user = userEvent.setup();
    render(<BmadCapabilityPanel {...props} />);

    const retry = screen.getByRole("button", { name: /prepare reviewed context again/i });
    expect(retry.hasAttribute("disabled")).toBe(true);

    await user.type(
      screen.getByRole("textbox", { name: "Context file paths" }),
      "docs/brief.md",
    );
    expect(retry.hasAttribute("disabled")).toBe(false);
    await user.click(retry);
    expect(props.onPrepare).toHaveBeenCalledWith(["docs/brief.md"]);
  });

  /// Regression guard: a document result that failed to decode rendered the
  /// same "stored locally under run" line as a healthy result of an unknown
  /// kind, so a run whose output never arrived read as a success.
  it("reports a document result it cannot decode as not shown, not as completed", () => {
    const props = createProps({
      kind: "completed",
      completed: {
        capabilityId: "bmm:bmad-product-brief",
        runId: review.runId,
        resultKind: "document_artifact",
        architecture: null,
      },
      resultJson: '{"resultKind":"document_artifact","documentArtifact":{"title":7}}',
    });
    render(<BmadCapabilityPanel {...props} />);

    expect(
      screen.getByRole("heading", { name: /result stored, but not shown/i }),
    ).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain(
      "did not come back in the shape this panel can read",
    );
    expect(screen.queryByRole("heading", { name: "Run completed" })).toBeNull();
  });

  it("names an unknown result kind as unviewable rather than claiming a document", () => {
    const props = createProps({
      kind: "completed",
      completed: {
        capabilityId: "bmm:bmad-product-brief",
        runId: review.runId,
        resultKind: "sealed_receipt",
        architecture: null,
      },
      resultJson: null,
    });
    render(<BmadCapabilityPanel {...props} />);

    expect(screen.getByRole("heading", { name: "Run completed" })).toBeTruthy();
    expect(screen.getByText(/this panel has no view for/u)).toBeTruthy();
  });

  /// Regression guard: the panel renders at the top of the scrolling skills
  /// drawer while the control that opens it sits far below in the agent list.
  /// Without a focus move, starting a run looked exactly like a dead button.
  it("takes focus when the run opens and hands it back when the run closes", () => {
    const opener = document.createElement("button");
    opener.textContent = "Start reviewed run";
    document.body.append(opener);
    opener.focus();
    expect(document.activeElement).toBe(opener);

    const { unmount } = render(
      <BmadCapabilityPanel {...createProps({ kind: "selecting" })} />,
    );
    const panel = screen.getByRole("dialog", { name: "Create Brief capability run" });
    expect(document.activeElement).toBe(panel);

    unmount();
    expect(document.activeElement).toBe(opener);
    opener.remove();
  });

  it("discloses the paths dropped past the context cap instead of silently slicing", async () => {
    const props = createProps({ kind: "selecting" });
    const user = userEvent.setup();
    render(<BmadCapabilityPanel {...props} />);

    const paths = Array.from({ length: 102 }, (_, index) => `docs/file-${index}.md`);
    await user.click(screen.getByRole("textbox", { name: "Context file paths" }));
    await user.paste(paths.join("\n"));

    expect(screen.getByRole("alert").textContent).toContain(
      "The remaining 2 paths are not included",
    );
    expect(screen.getByRole("status").textContent).toBe("100 files selected.");
    await user.click(screen.getByRole("button", { name: /prepare reviewed context/i }));
    expect(props.onPrepare).toHaveBeenCalledWith(paths.slice(0, 100));
  });

  /// Regression guard: the approved phase said the consent "expires shortly"
  /// and left Send enabled forever, so the standard way to meet this feature
  /// was a rejected send with no route back.
  it("refuses to offer a send once the single-use consent has lapsed", async () => {
    const props = createProps({
      kind: "approved",
      review,
      approved: { ...approved, expiresAt: ALREADY_LAPSED },
      contextPaths: ["docs/brief.md"],
      sending: false,
    });
    const user = userEvent.setup();
    const { container } = render(<BmadCapabilityPanel {...props} />);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /send once and run/i }).hasAttribute("disabled"),
      ).toBe(true);
    });
    expect(screen.getByRole("alert").textContent).toContain("That consent has lapsed");

    await user.click(screen.getByRole("button", { name: /send once and run/i }));
    expect(props.onSubmit).not.toHaveBeenCalled();

    // The lapse is recoverable in place, on the same context.
    await user.click(screen.getByRole("button", { name: "Prepare this context again" }));
    expect(props.onPrepare).toHaveBeenCalledWith(["docs/brief.md"]);
    await expectAccessible(container);
  });

  it("keeps the send enabled while the consent is still live", () => {
    const props = createProps({
      kind: "approved",
      review,
      approved,
      contextPaths: ["docs/brief.md"],
      sending: false,
    });
    render(<BmadCapabilityPanel {...props} />);

    expect(
      screen.getByRole("button", { name: /send once and run/i }).hasAttribute("disabled"),
    ).toBe(false);
    expect(screen.queryByRole("button", { name: "Prepare this context again" })).toBeNull();
    expect(screen.getByRole("status").textContent).toContain("lapses at");
  });

  it("refuses to offer an approval once the reviewed manifest has expired", async () => {
    const props = createProps({
      kind: "review",
      review: { ...review, expiresAt: ALREADY_LAPSED },
      contextPaths: ["docs/brief.md"],
    });
    const user = userEvent.setup();
    render(<BmadCapabilityPanel {...props} />);

    await waitFor(() => {
      expect(
        screen
          .getByRole("button", { name: /approve this exact context/i })
          .hasAttribute("disabled"),
      ).toBe(true);
    });
    await user.click(screen.getByRole("button", { name: /approve this exact context/i }));
    expect(props.onApprove).not.toHaveBeenCalled();
    expect(screen.getByRole("alert").textContent).toContain("expired before it was approved");
  });

  /// A review over nothing would send nothing; rendering an empty list beside
  /// "the exact reviewed context" made that look like a broken panel instead.
  it("names an empty reviewed context rather than rendering an empty list", () => {
    const props = createProps({ kind: "review", review, contextPaths: [] });
    render(<BmadCapabilityPanel {...props} />);

    expect(screen.getByRole("alert").textContent).toContain("This review covers no files");
    expect(screen.queryByRole("list", { name: "Reviewed context files" })).toBeNull();
    expect(
      screen.getByRole("button", { name: /approve this exact context/i }).hasAttribute("disabled"),
    ).toBe(true);
  });

  it("says what a reviewed run does and where the context goes", () => {
    render(<BmadCapabilityPanel {...createProps({ kind: "selecting" })} />);

    expect(
      screen.getByText(/A reviewed run never sends your workspace/u).textContent,
    ).toContain("sent once to Deterministic (local)");
  });
});

describe("BmadCapabilityPanel — ordered architecture progression", () => {
  const architectureReview = {
    capabilityId: "bmm:bmad-architecture",
    runId: "caprun_01ARZ3NDEKTSV4RRFFQ69G5FAV",
    manifestHash,
    expiresAt: LIVE_UNTIL,
    architecture: {
      stepCode: "trust_boundaries",
      stepTitle: "Trust boundaries",
      stepNumber: 3,
      stepCount: 8,
      finalized: false,
      spine: null,
    },
  };

  function architectureProps(phase: CapabilityRunPhase): BmadCapabilityPanelProps {
    return {
      ...createProps(phase),
      capabilityId: "bmm:bmad-architecture",
      capabilityLabel: "Create Architecture",
    };
  }

  it("announces which host-selected step the review covers", async () => {
    const props = architectureProps({
      kind: "review",
      review: architectureReview,
      contextPaths: ["docs/design-notes.md"],
    });
    const { container } = render(<BmadCapabilityPanel {...props} />);

    expect(screen.getByText("Step 3 of 8 — Trust boundaries")).toBeTruthy();
    await expectAccessible(container);
  });

  it("reports an intermediate step as recorded, with no skip affordance", async () => {
    const props = architectureProps({
      kind: "completed",
      completed: {
        capabilityId: "bmm:bmad-architecture",
        runId: architectureReview.runId,
        resultKind: "document_artifact",
        architecture: architectureReview.architecture,
      },
      resultJson: null,
    });
    const { container } = render(<BmadCapabilityPanel {...props} />);

    expect(screen.getByRole("heading", { name: /step answered/i })).toBeTruthy();
    expect(screen.getByText(/will not let a step be skipped or repeated/iu)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /skip|finalize|apply/i })).toBeNull();
    await expectAccessible(container);
  });

  it("renders the distilled spine inertly on the final step", async () => {
    const props = architectureProps({
      kind: "completed",
      completed: {
        capabilityId: "bmm:bmad-architecture",
        runId: architectureReview.runId,
        resultKind: "document_artifact",
        architecture: {
          stepCode: "verification",
          stepTitle: "Verification",
          stepNumber: 8,
          stepCount: 8,
          finalized: true,
          spine: {
            title: "Architecture spine",
            sections: [
              { heading: "1. System context", body: "AD-1 — Scope\nBounded to one workspace." },
              { heading: "8. Verification", body: "AD-8 — Evidence\nEach decision is testable." },
            ],
            decisionLog: [
              { kind: "decision", subject: "System context", text: "AD-1 — Scope: one workspace." },
              { kind: "event", subject: null, text: "spine finalized" },
            ],
            openQuestions: ["Which tenant model?"],
            mermaidText: null,
          },
        },
      },
      resultJson: null,
    });
    const { container } = render(<BmadCapabilityPanel {...props} />);

    expect(screen.getByText("All 8 steps answered — spine distilled.")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Architecture spine" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "1. System context" })).toBeTruthy();
    expect(screen.getByText(/No model was contacted for this artifact/iu)).toBeTruthy();
    expect(screen.getByText("Which tenant model?")).toBeTruthy();
    // The spine is a document, not an effect: nothing here can apply or publish.
    expect(document.body.textContent).not.toMatch(
      /\b(?:Apply|Install|Execute|Activate|Publish)\b/u,
    );
    await expectAccessible(container);
  });

  it("shows no progression for a one-shot capability", () => {
    const props = createProps({
      kind: "review",
      review,
      contextPaths: ["docs/brief.md"],
    });
    render(<BmadCapabilityPanel {...props} />);

    expect(screen.queryByText(/Step \d+ of \d+/u)).toBeNull();
  });
});
