import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  ChangesExecutionProjection,
  ChangesReviewEnvelopeProjection,
} from "../lib/hostClient";
import {
  GovernedChangesPanel,
  type GovernedChangesPanelProps,
} from "./GovernedChangesPanel";

const sha = (seed: string) => `sha256:${seed.repeat(64).slice(0, 64)}`;

const reviewEnvelope: ChangesReviewEnvelopeProjection = {
  approvalId: "approval_01J00000000000000000000000",
  displayedDiffHash: sha("a"),
  review: {
    schemaVersion: "sapphirus.changes-review.v1",
    proposalId: "proposal_01J00000000000000000000000",
    candidateId: "candidate_01J00000000000000000000000",
    candidateHash: sha("b"),
    workspaceId: "workspace_01J00000000000000000000000",
    workspaceGrantEpoch: 2,
    proposalKind: "edit",
    sourceExecutionId: null,
    files: [{
      relativePath: "src/main.rs",
      operation: "modify",
      beforeContent: "fn main() {}\n",
      afterContent: "fn main() { updated(); }\n",
      beforeHash: sha("c"),
      afterHash: sha("d"),
      beforeBytes: 13,
      afterBytes: 26,
    }],
    totalChangedBytes: 26,
    createdAt: 1_000,
    expiresAt: 601_000,
  },
};

const appliedExecution: ChangesExecutionProjection = {
  executionId: "execution_01J00000000000000000000000",
  checkpointId: "checkpoint_01J00000000000000000000000",
  completedAt: 5_000,
  undoable: true,
  files: [{
    relativePath: "src/main.rs",
    operation: "modified",
    exists: true,
    contentHash: sha("d"),
  }],
};

function createProps(
  overrides: Partial<GovernedChangesPanelProps> = {},
): GovernedChangesPanelProps {
  return {
    canEnableEdits: false,
    enableEditsBusy: false,
    errorMessage: null,
    onDecide: vi.fn(),
    onEnableEdits: vi.fn(),
    onPropose: vi.fn(),
    onStartNewProposal: vi.fn(),
    onUndo: vi.fn(),
    state: { kind: "idle" },
    ...overrides,
  };
}

afterEach(cleanup);

describe("GovernedChangesPanel", () => {
  it("offers governed-edits enablement only when the host allows it", () => {
    const props = createProps({
      canEnableEdits: true,
      state: { kind: "unavailable", reason: "This workspace is read only." },
    });
    render(<GovernedChangesPanel {...props} />);
    expect(screen.getByText("This workspace is read only.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Allow governed edits" }));
    expect(props.onEnableEdits).toHaveBeenCalledTimes(1);

    cleanup();
    render(<GovernedChangesPanel {...createProps({
      state: { kind: "unavailable", reason: "Host unavailable." },
    })} />);
    expect(screen.queryByRole("button", { name: "Allow governed edits" })).toBeNull();
  });

  it("submits a trimmed proposal from the composer", () => {
    const props = createProps({ state: { kind: "idle" } });
    render(<GovernedChangesPanel {...props} />);
    fireEvent.change(screen.getByLabelText("Relative path"), {
      target: { value: "  src/new.rs " },
    });
    fireEvent.change(screen.getByLabelText("Proposed content"), {
      target: { value: "pub fn created() {}\n" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Review changes" }));
    expect(props.onPropose).toHaveBeenCalledWith("src/new.rs", "pub fn created() {}\n");
  });

  it("renders the exact reviewed content and binds each decision", () => {
    const props = createProps({
      state: { kind: "review", busy: false, review: reviewEnvelope },
    });
    render(<GovernedChangesPanel {...props} />);
    expect(screen.getByRole("heading", { name: "Review changes" })).toBeTruthy();
    expect(
      screen.getByLabelText("Current content of src/main.rs").textContent,
    ).toContain("fn main() {}");
    expect(
      screen.getByLabelText("Proposed content of src/main.rs").textContent,
    ).toContain("fn main() { updated(); }");

    fireEvent.click(screen.getByRole("button", { name: "Apply changes" }));
    fireEvent.click(screen.getByRole("button", { name: "Revise" }));
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    expect(props.onDecide).toHaveBeenNthCalledWith(1, "apply");
    expect(props.onDecide).toHaveBeenNthCalledWith(2, "revise");
    expect(props.onDecide).toHaveBeenNthCalledWith(3, "discard");
  });

  it("disables every decision while a decision is in flight", () => {
    render(<GovernedChangesPanel {...createProps({
      state: { kind: "review", busy: true, review: reviewEnvelope },
    })} />);
    for (const name of ["Apply changes", "Revise", "Discard"]) {
      expect((screen.getByRole("button", { name }) as HTMLButtonElement).disabled).toBe(true);
    }
  });

  it("offers Undo changes after an applied execution", () => {
    const props = createProps({
      state: { kind: "applied", busy: false, execution: appliedExecution },
    });
    render(<GovernedChangesPanel {...props} />);
    expect(screen.getByRole("heading", { name: "Changes applied" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Undo changes" }));
    expect(props.onUndo).toHaveBeenCalledWith(appliedExecution.executionId);
  });

  it("explains undo conflicts without offering an effect", () => {
    const props = createProps({
      state: {
        kind: "undo_unavailable",
        value: {
          executionId: appliedExecution.executionId,
          reason: "The workspace changed after this change was applied.",
          conflicts: [{
            relativePath: "src/main.rs",
            expectedExists: true,
            currentExists: true,
          }],
        },
      },
    });
    render(<GovernedChangesPanel {...props} />);
    expect(screen.getByRole("heading", { name: "Undo changes is unavailable" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Undo changes" })).toBeNull();
    expect(screen.getByRole("list", { name: "Undo conflicts" })).toBeTruthy();
  });

  it("surfaces safe host errors as an alert", () => {
    render(<GovernedChangesPanel {...createProps({
      errorMessage: "The workspace changed after review.",
      state: { kind: "review", busy: false, review: reviewEnvelope },
    })} />);
    expect(screen.getByRole("alert").textContent).toContain(
      "The workspace changed after review.",
    );
  });
});
