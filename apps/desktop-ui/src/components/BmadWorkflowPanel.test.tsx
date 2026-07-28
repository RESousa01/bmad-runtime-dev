// @vitest-environment jsdom
import "../test/setup";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { describe, expect, it, vi } from "vitest";

import { BmadWorkflowPanel } from "./BmadWorkflowPanel";
import type { BmadWorkflowStateProjection } from "../lib/hostClient";

function state(
  overrides: Partial<BmadWorkflowStateProjection> = {},
): BmadWorkflowStateProjection {
  return {
    schemaVersion: "sapphirus.bmad-workflow-state.v1",
    methodInstalled: true,
    phaseCode: "3-solutioning",
    phaseTitle: "Solutioning",
    steps: [
      {
        key: "bmad-prd",
        displayName: "Create Edit and Review PRD",
        menuCode: "PRD",
        phaseCode: "2-planning",
        required: true,
        completed: true,
        available: false,
        evidencePath: "docs/plan/prd-checkout.md",
      },
      {
        key: "bmad-architecture",
        displayName: "Architecture",
        menuCode: "CA",
        phaseCode: "3-solutioning",
        required: true,
        completed: false,
        available: true,
        evidencePath: null,
      },
    ],
    nextRequired: "bmad-architecture",
    unresolvedLocations: [],
    ...overrides,
  };
}

async function expectAccessible(container: HTMLElement) {
  const results = await axe.run(container, {
    rules: { "color-contrast": { enabled: false } },
  });
  expect(results.violations).toEqual([]);
}

describe("BmadWorkflowPanel", () => {
  it("states the phase, the next gate, and the evidence behind each completion", async () => {
    const { container } = render(
      <BmadWorkflowPanel error={null} loading={false} onRefresh={vi.fn()} state={state()} />,
    );

    expect(screen.getByRole("heading", { name: "Current phase: Solutioning" })).toBeTruthy();
    expect(screen.getByText(/Next required:/u).textContent).toContain("Architecture");
    // A completion is shown with the artifact that evidenced it, never bare.
    expect(screen.getByText("docs/plan/prd-checkout.md")).toBeTruthy();
    await expectAccessible(container);
  });

  it("offers to start only an available, incomplete step", async () => {
    const onStartStep = vi.fn();
    const user = userEvent.setup();
    render(
      <BmadWorkflowPanel
        error={null}
        loading={false}
        onRefresh={vi.fn()}
        onStartStep={onStartStep}
        state={state()}
      />,
    );

    const buttons = screen.getAllByRole("button", { name: "Start" });
    expect(buttons).toHaveLength(1);
    await user.click(buttons[0]!);
    expect(onStartStep).toHaveBeenCalledWith(
      expect.objectContaining({ key: "bmad-architecture" }),
    );
  });

  it("says plainly when there is no BMAD install rather than inventing a phase", async () => {
    const { container } = render(
      <BmadWorkflowPanel
        error={null}
        loading={false}
        onRefresh={vi.fn()}
        state={state({ methodInstalled: false })}
      />,
    );

    expect(screen.getByRole("heading", { name: /No BMAD install found/u })).toBeTruthy();
    expect(screen.queryByText(/Next required:/u)).toBeNull();
    await expectAccessible(container);
  });

  it("explains why a step could not be checked instead of guessing it", () => {
    render(
      <BmadWorkflowPanel
        error={null}
        loading={false}
        onRefresh={vi.fn()}
        state={state({ unresolvedLocations: ["implementation_artifacts"] })}
      />,
    );

    expect(screen.getByRole("note").textContent).toContain("implementation_artifacts");
    expect(screen.getByRole("note").textContent).toContain("rather than guessed");
  });

  it("reports that every gate is met when nothing is outstanding", () => {
    render(
      <BmadWorkflowPanel
        error={null}
        loading={false}
        onRefresh={vi.fn()}
        state={state({ nextRequired: null })}
      />,
    );

    expect(screen.getByRole("status").textContent).toContain("Every required gate is complete.");
  });

  it("groups steps under their phase", () => {
    render(<BmadWorkflowPanel error={null} loading={false} onRefresh={vi.fn()} state={state()} />);

    const planning = screen.getByRole("region", { name: "Planning" });
    expect(within(planning).getByText("Create Edit and Review PRD")).toBeTruthy();
    const solutioning = screen.getByRole("region", { name: "Solutioning" });
    expect(within(solutioning).getByText("Architecture")).toBeTruthy();
  });

  it("surfaces a host error with a retry rather than a blank panel", async () => {
    const onRefresh = vi.fn();
    const user = userEvent.setup();
    render(
      <BmadWorkflowPanel
        error="The workspace grant expired."
        loading={false}
        onRefresh={onRefresh}
        state={null}
      />,
    );

    expect(screen.getByRole("alert").textContent).toContain("grant expired");
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(onRefresh).toHaveBeenCalledOnce();
  });

  /// Regression guard: the unloaded branch rendered only a status line, and
  /// `onRefresh` is the sole caller of the host's workflow-state command — so
  /// method progress could never be read at all.
  it("offers a load control before any state has been read", async () => {
    const onRefresh = vi.fn();
    const user = userEvent.setup();
    render(
      <BmadWorkflowPanel error={null} loading={false} onRefresh={onRefresh} state={null} />,
    );

    await user.click(screen.getByRole("button", { name: "Read method progress" }));
    expect(onRefresh).toHaveBeenCalledOnce();
  });

  it("disables the load control while a read is in flight", () => {
    render(
      <BmadWorkflowPanel error={null} loading onRefresh={vi.fn()} state={null} />,
    );

    expect(screen.getByRole("button", { name: "Reading…" })).toHaveProperty("disabled", true);
  });

  /// Regression guard: the error branch returned early and discarded `state`,
  /// so a refresh that failed wiped every phase, gate and evidence path already
  /// on screen — indistinguishable from a workspace with no method progress.
  it("keeps the loaded progress on screen when a refresh fails", async () => {
    const onRefresh = vi.fn();
    const user = userEvent.setup();
    const { container } = render(
      <BmadWorkflowPanel
        error="The workspace grant expired."
        loading={false}
        onRefresh={onRefresh}
        state={state()}
      />,
    );

    expect(screen.getByRole("alert").textContent).toContain("grant expired");
    // The progress the host already reported is still there and still readable.
    expect(screen.getByRole("heading", { name: "Current phase: Solutioning" })).toBeTruthy();
    expect(screen.getByText("docs/plan/prd-checkout.md")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(onRefresh).toHaveBeenCalledOnce();
    await expectAccessible(container);
  });

  it("does not accept another refresh while one is in flight", () => {
    render(
      <BmadWorkflowPanel error={null} loading onRefresh={vi.fn()} state={state()} />,
    );

    expect(screen.getByRole("button", { name: "Refreshing…" })).toHaveProperty("disabled", true);
  });

  /// Regression guard: the no-install branch had no control at all, so
  /// installing BMAD afterwards left the panel permanently asserting there was
  /// no install with nothing on screen able to ask the host again.
  it("can re-check for an install it did not find", async () => {
    const onRefresh = vi.fn();
    const user = userEvent.setup();
    render(
      <BmadWorkflowPanel
        error={null}
        loading={false}
        onRefresh={onRefresh}
        state={state({ methodInstalled: false })}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Check again" }));
    expect(onRefresh).toHaveBeenCalledOnce();
  });

  /// FE-25. Seven of the 48 upstream skills are present in the pinned upstream
  /// and absent from this build, and several of them are steps of the method
  /// graph. They rendered as ordinary rows — with a start handler wired, the
  /// panel offered a Start for a step nothing here could have run.
  it("names a step this build cannot start, with the reason, instead of offering it", async () => {
    const onStartStep = vi.fn();
    const { container } = render(
      <BmadWorkflowPanel
        error={null}
        loading={false}
        onRefresh={vi.fn()}
        onStartStep={onStartStep}
        state={state({
          steps: [
            {
              key: "bmad-sprint-status",
              displayName: "Sprint status",
              menuCode: "SS",
              phaseCode: "4-implementation",
              required: false,
              // The ordering is satisfied: only the coverage matrix stands
              // between this row and a Start it could not have honoured.
              completed: false,
              available: true,
              evidencePath: null,
            },
          ],
          nextRequired: null,
        })}
      />,
    );

    const row = screen.getByRole("listitem");
    expect(within(row).getByText("Sprint status")).toBeTruthy();
    expect(within(row).getByText("Cannot run here")).toBeTruthy();
    expect(
      within(row).getByText("Present in the pinned upstream and absent from this build."),
    ).toBeTruthy();
    expect(within(row).queryByRole("button")).toBeNull();
    expect(onStartStep).not.toHaveBeenCalled();
    await expectAccessible(container);
  });

  /// `unknown` is not a softer `unavailable`. A step nothing on hand records is
  /// not a step this build has established it cannot run, and saying the
  /// stronger of the two would be the invention this surface exists to stop.
  it("says a step nothing records is not known, rather than that it cannot run", () => {
    render(
      <BmadWorkflowPanel
        error={null}
        loading={false}
        onRefresh={vi.fn()}
        onStartStep={vi.fn()}
        state={state({
          steps: [
            {
              key: "bmad-invented-by-nobody",
              displayName: "Invented step",
              menuCode: "IS",
              phaseCode: "4-implementation",
              required: false,
              completed: false,
              available: true,
              evidencePath: null,
            },
          ],
          nextRequired: null,
        })}
      />,
    );

    const row = screen.getByRole("listitem");
    expect(within(row).getByText("Not known")).toBeTruthy();
    expect(within(row).getByText(/bmad-invented-by-nobody/u)).toBeTruthy();
    expect(within(row).getByText(/has no row for it/u)).toBeTruthy();
    expect(within(row).queryByText("Cannot run here")).toBeNull();
    expect(within(row).queryByRole("button")).toBeNull();
  });

  /// Upstream keys a multi-action skill as `skill:action` and the coverage
  /// matrix records one row per upstream directory, so the action has to come
  /// off before the matrix is asked. Otherwise every multi-action step of the
  /// graph reads as a skill nobody recorded.
  it("asks the coverage matrix about the skill behind a multi-action step key", async () => {
    const onStartStep = vi.fn();
    const user = userEvent.setup();
    render(
      <BmadWorkflowPanel
        error={null}
        loading={false}
        onRefresh={vi.fn()}
        onStartStep={onStartStep}
        state={state({
          steps: [
            {
              key: "bmad-create-story:validate",
              displayName: "Validate story",
              menuCode: "VS",
              phaseCode: "4-implementation",
              required: false,
              completed: false,
              available: true,
              evidencePath: null,
            },
          ],
          nextRequired: null,
        })}
      />,
    );

    const row = screen.getByRole("listitem");
    expect(within(row).queryByText("Not known")).toBeNull();
    await user.click(within(row).getByRole("button", { name: "Start" }));
    expect(onStartStep).toHaveBeenCalledWith(
      expect.objectContaining({ key: "bmad-create-story:validate" }),
    );
  });

  /// Regression guard: an incomplete step that is not yet available rendered
  /// its name and nothing else — no control, no reason, no difference from a
  /// row whose button had failed to render.
  it("says why an incomplete step offers no start control", () => {
    render(
      <BmadWorkflowPanel
        error={null}
        loading={false}
        onRefresh={vi.fn()}
        onStartStep={vi.fn()}
        state={state({
          steps: [
            {
              key: "bmad-ux",
              displayName: "UX specification",
              menuCode: "UX",
              phaseCode: "3-solutioning",
              required: false,
              completed: false,
              available: false,
              evidencePath: null,
            },
          ],
          nextRequired: null,
        })}
      />,
    );

    const row = screen.getByRole("listitem");
    expect(within(row).getByText("waiting on an earlier step")).toBeTruthy();
    expect(within(row).queryByRole("button")).toBeNull();
  });
});
