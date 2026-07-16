// @vitest-environment jsdom
import "../test/setup";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { initialBmadRequestState } from "../lib/bmadModelProjection";
import { TaskWorkspace, type TaskWorkspaceProps } from "./TaskWorkspace";

function createProps(
  overrides: Partial<TaskWorkspaceProps> = {},
): TaskWorkspaceProps {
  return {
    hostStatusLabel: "Desktop host ready",
    interactionDisabled: true,
    isNewSession: true,
    isReadOnlyRecovery: false,
    methodGuidanceAvailable: true,
    methodGuidanceState: initialBmadRequestState,
    methodLibraryAvailable: true,
    onOpenInspector: vi.fn(),
    onOpenMethodLibrary: vi.fn(),
    onOpenSessions: vi.fn(),
    onReviewChanges: vi.fn(),
    onReviewContext: vi.fn(),
    onReviewRequest: vi.fn(async () => undefined),
    proposalState: "ready",
    sessionTitle: "Method guidance",
    workspaceName: "sapphirus",
    ...overrides,
  };
}

describe("TaskWorkspace Method guidance", () => {
  it("enables only the intent composer while attach and mode stay disabled", () => {
    render(<TaskWorkspace {...createProps()} />);

    expect(
      screen.getByLabelText("Describe what you want Method guidance for"),
    ).toHaveProperty("disabled", false);
    expect(screen.getByRole("button", { name: "Attach context" })).toHaveProperty(
      "disabled",
      true,
    );
    expect(screen.getByRole("combobox", { name: "Mode" })).toHaveProperty(
      "disabled",
      true,
    );
    expect(
      screen.getByRole("button", { name: "Review request" }),
    ).toHaveProperty("disabled", true);
    expect(screen.getAllByText("Method guidance").length).toBeGreaterThan(0);
    expect(
      screen.getAllByText(/inert local Method run/i).length,
    ).toBeGreaterThan(0);
  });

  it("submits the trimmed intent for review without claiming a send", async () => {
    const user = userEvent.setup();
    const onReviewRequest = vi.fn(async (_intent: string) => undefined);
    render(<TaskWorkspace {...createProps({ onReviewRequest })} />);

    await user.type(
      screen.getByLabelText("Describe what you want Method guidance for"),
      "  Help me choose the next architecture step.  ",
    );
    await user.click(screen.getByRole("button", { name: "Review request" }));

    expect(onReviewRequest).toHaveBeenCalledOnce();
    expect(onReviewRequest).toHaveBeenCalledWith("Help me choose the next architecture step.");
    expect(screen.getByText("Help me choose the next architecture step.")).toBeTruthy();
    expect(screen.getAllByText(/nothing is sent until you approve context and choose Send request/i).length)
      .toBeGreaterThan(0);

    expect(screen.queryByText("Demo response")).toBeNull();
    expect(screen.queryByText("Ready for review")).toBeNull();
    expect(screen.queryByText("2 files")).toBeNull();
    expect(screen.queryByRole("button", { name: "Review changes" })).toBeNull();
  });

  it("labels the initial retained-run lookup without claiming creation", () => {
    render(<TaskWorkspace {...createProps({
      methodGuidanceState: { kind: "creating", activity: "recovering" },
    })} />);

    expect(
      screen.getByLabelText("Describe what you want Method guidance for"),
    ).toHaveProperty("disabled", true);
    expect(
      screen.getByRole("button", { name: "Review request" }),
    ).toHaveProperty("disabled", true);
    expect(screen.getAllByText(/Checking for a retained local Method result/i).length)
      .toBeGreaterThan(0);
    expect(screen.getByText("Checking · Local only")).toBeTruthy();
    expect(screen.queryByText("Preparing · Local only")).toBeNull();
  });

  it("claims creation only after the user submits an intent", async () => {
    const user = userEvent.setup();
    const onReviewRequest = vi.fn(() => new Promise<void>(() => undefined));
    const { rerender } = render(<TaskWorkspace {...createProps({ onReviewRequest })} />);

    await user.type(
      screen.getByLabelText("Describe what you want Method guidance for"),
      "Choose the next safe Method step",
    );
    await user.click(screen.getByRole("button", { name: "Review request" }));

    rerender(<TaskWorkspace {...createProps({
      methodGuidanceState: { kind: "creating", activity: "creating" },
      onReviewRequest,
    })} />);

    expect(await screen.findByText("Preparing · Local only")).toBeTruthy();
    expect(screen.getAllByText(/Preparing the exact outbound review/i).length)
      .toBeGreaterThan(0);
  });

  it(
    "never turns a real guidance submission into demo content if availability changes",
    async () => {
      const user = userEvent.setup();
      const onReviewRequest = vi.fn(async (_intent: string) => undefined);
      const { rerender } = render(
        <TaskWorkspace {...createProps({ onReviewRequest })} />,
      );

      await user.type(
        screen.getByLabelText("Describe what you want Method guidance for"),
        "Recommend a safe next Method step",
      );
      await user.click(
        screen.getByRole("button", { name: "Review request" }),
      );
      expect(await screen.findByText("Recommend a safe next Method step")).toBeTruthy();

      rerender(
        <TaskWorkspace
          {...createProps({
            methodGuidanceAvailable: false,
            onReviewRequest,
          })}
        />,
      );

      expect(screen.getByText("Recommend a safe next Method step")).toBeTruthy();
      expect(screen.queryByText("Demo response")).toBeNull();
      expect(screen.queryByText("Ready for review")).toBeNull();
    },
  );

  it("preserves the disabled browser preview when Method guidance is unavailable", () => {
    render(
      <TaskWorkspace
        {...createProps({
          interactionDisabled: false,
          methodGuidanceAvailable: false,
          methodGuidanceState: initialBmadRequestState,
        })}
      />,
    );

    expect(screen.getByLabelText("Describe a task")).toHaveProperty(
      "disabled",
      true,
    );
    expect(screen.getByRole("button", { name: "Send task" })).toHaveProperty(
      "disabled",
      true,
    );
    expect(
      screen.getByText(/Agent tasks and local changes are not enabled/i),
    ).toBeTruthy();
  });
});
