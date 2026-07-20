// @vitest-environment jsdom
import "../test/setup";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { describe, expect, it, vi } from "vitest";
import {
  BmadCapabilityPanel,
  type BmadCapabilityPanelProps,
  type CapabilityRunPhase,
} from "./BmadCapabilityPanel";

const manifestHash = `sha256:${"a".repeat(64)}`;

const review = {
  capabilityId: "bmm:bmad-product-brief",
  runId: "caprun_01ARZ3NDEKTSV4RRFFQ69G5FAV",
  manifestHash,
  expiresAt: 1_725_000_600_000,
};

const approved = {
  capabilityId: "bmm:bmad-product-brief",
  manifestHash,
  decisionId: "decision_01ARZ3NDEKTSV4RRFFQ69G5FAV",
  expiresAt: 1_725_000_300_000,
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
      },
      resultJson: null,
    });
    const { container } = render(<BmadCapabilityPanel {...props} />);
    expect(
      screen.getByRole("heading", { name: /inactive draft produced/i }),
    ).toBeTruthy();
    expect(document.body.textContent).not.toMatch(
      /\\b(?:Install|Activate|Register|Execute|Apply)\\b/u,
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

  it("treats malformed stored results as non-displayable data", () => {
    const props = createProps({
      kind: "completed",
      completed: {
        capabilityId: "bmm:bmad-product-brief",
        runId: review.runId,
        resultKind: "document_artifact",
      },
      resultJson: '{"resultKind":"document_artifact","documentArtifact":{"title":7}}',
    });
    render(<BmadCapabilityPanel {...props} />);
    expect(screen.getByText(/stored locally under run/u)).toBeTruthy();
  });
});
