import { describe, expect, it } from "vitest";

import { parseWorkflowState } from "./workflowProtocol";

const step = (overrides: Record<string, unknown> = {}) => ({
  key: "bmad-architecture",
  displayName: "Architecture",
  menuCode: "CA",
  phaseCode: "3-solutioning",
  required: true,
  completed: false,
  available: true,
  evidencePath: null,
  ...overrides,
});

const state = (overrides: Record<string, unknown> = {}) => ({
  schemaVersion: "sapphirus.bmad-workflow-state.v1",
  methodInstalled: true,
  phaseCode: "3-solutioning",
  phaseTitle: "Solutioning",
  steps: [step()],
  nextRequired: "bmad-architecture",
  unresolvedLocations: [],
  ...overrides,
});

describe("workflow state protocol", () => {
  it("accepts a computed state", () => {
    const parsed = parseWorkflowState(state());
    expect(parsed.phaseCode).toBe("3-solutioning");
    expect(parsed.steps[0]?.key).toBe("bmad-architecture");
  });

  it("requires evidence to back a completion, and none without one", () => {
    // A step claiming completion with no artifact behind it is a guess.
    expect(() =>
      parseWorkflowState(state({ steps: [step({ completed: true, evidencePath: null })] })),
    ).toThrow();
    // Evidence on an incomplete step is equally inconsistent.
    expect(() =>
      parseWorkflowState(
        state({ steps: [step({ completed: false, evidencePath: "docs/x.md" })] }),
      ),
    ).toThrow();
    // The consistent pair is accepted.
    expect(
      parseWorkflowState(
        state({
          steps: [step({ completed: true, evidencePath: "docs/x.md" })],
          nextRequired: null,
        }),
      ).steps[0]?.evidencePath,
    ).toBe("docs/x.md");
  });

  it("rejects a next-required gate that is unknown or already met", () => {
    expect(() => parseWorkflowState(state({ nextRequired: "not-a-step" }))).toThrow();
    expect(() =>
      parseWorkflowState(
        state({
          steps: [step({ completed: true, evidencePath: "docs/x.md" })],
          nextRequired: "bmad-architecture",
        }),
      ),
    ).toThrow();
  });

  it("rejects an unknown phase and a foreign schema", () => {
    expect(() => parseWorkflowState(state({ phaseCode: "9-unknown" }))).toThrow();
    expect(() => parseWorkflowState(state({ steps: [step({ phaseCode: "9-x" })] }))).toThrow();
    expect(() => parseWorkflowState(state({ schemaVersion: "other.v1" }))).toThrow();
  });

  it("rejects unknown keys smuggled into the state or a step", () => {
    expect(() => parseWorkflowState({ ...state(), apply: true })).toThrow();
    expect(() =>
      parseWorkflowState(state({ steps: [{ ...step(), command: "pwsh" }] })),
    ).toThrow();
  });

  it("rejects an evidence path that escapes the workspace", () => {
    for (const escape of ["../secrets.md", "/etc/passwd", "C:/Windows/system.ini"]) {
      expect(() =>
        parseWorkflowState(
          state({
            steps: [step({ completed: true, evidencePath: escape })],
            nextRequired: null,
          }),
        ),
      ).toThrow();
    }
  });
});
