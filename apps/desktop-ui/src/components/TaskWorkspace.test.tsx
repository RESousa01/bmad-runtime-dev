// @vitest-environment jsdom
import "../test/setup";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
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
    methodGuidanceBusy: false,
    methodLibraryAvailable: true,
    onOpenInspector: vi.fn(),
    onOpenMethodLibrary: vi.fn(),
    onOpenSessions: vi.fn(),
    onReviewChanges: vi.fn(),
    onReviewContext: vi.fn(),
    onTaskSubmitted: vi.fn(async () => undefined),
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
      screen.getByRole("button", { name: "Request Method guidance" }),
    ).toHaveProperty("disabled", true);
    expect(screen.getAllByText("Method guidance").length).toBeGreaterThan(0);
    expect(
      screen.getAllByText(/local, unbound Method session/i).length,
    ).toBeGreaterThan(0);
  });

  it("submits the trimmed intent and shows only the truthful unbound result", async () => {
    const user = userEvent.setup();
    const onTaskSubmitted = vi.fn(async (_intent: string) => undefined);
    render(<TaskWorkspace {...createProps({ onTaskSubmitted })} />);

    await user.type(
      screen.getByLabelText("Describe what you want Method guidance for"),
      "  Help me choose the next architecture step.  ",
    );
    await user.click(screen.getByRole("button", { name: "Request Method guidance" }));

    expect(onTaskSubmitted).toHaveBeenCalledOnce();
    expect(onTaskSubmitted).toHaveBeenCalledWith("Help me choose the next architecture step.");
    expect(await screen.findByText("Created · Unbound")).toBeTruthy();
    expect(screen.getByText(/no model request was made/i)).toBeTruthy();
    expect(screen.getByText(/no workspace change was proposed/i)).toBeTruthy();
    expect(
      screen.getByText(
        /Review the source-grounded recommendation in the Method inspector/i,
      ),
    ).toBeTruthy();

    expect(screen.queryByText("Demo response")).toBeNull();
    expect(screen.queryByText("Ready for review")).toBeNull();
    expect(screen.queryByText("2 files")).toBeNull();
    expect(screen.queryByRole("button", { name: "Review changes" })).toBeNull();
  });

  it("labels the initial retained-run lookup without claiming creation", () => {
    render(<TaskWorkspace {...createProps({ methodGuidanceBusy: true })} />);

    expect(
      screen.getByLabelText("Describe what you want Method guidance for"),
    ).toHaveProperty("disabled", true);
    expect(
      screen.getByRole("button", { name: "Request Method guidance" }),
    ).toHaveProperty("disabled", true);
    expect(screen.getAllByText(/Checking for a retained local Method session/i).length)
      .toBeGreaterThan(0);
    expect(screen.getByText("Checking · Local only")).toBeTruthy();
    expect(screen.queryByText("Creating · Local only")).toBeNull();
  });

  it("claims creation only after the user submits an intent", async () => {
    const user = userEvent.setup();
    const onTaskSubmitted = vi.fn(() => new Promise<void>(() => undefined));
    render(<TaskWorkspace {...createProps({ onTaskSubmitted })} />);

    await user.type(
      screen.getByLabelText("Describe what you want Method guidance for"),
      "Choose the next safe Method step",
    );
    await user.click(screen.getByRole("button", { name: "Request Method guidance" }));

    expect(await screen.findByText("Creating · Local only")).toBeTruthy();
    expect(screen.getAllByText(/Creating the local Method session/i).length)
      .toBeGreaterThan(0);
  });

  it(
    "never turns a real guidance submission into demo content if availability changes",
    async () => {
      const user = userEvent.setup();
      const onTaskSubmitted = vi.fn(async (_intent: string) => undefined);
      const { rerender } = render(
        <TaskWorkspace {...createProps({ onTaskSubmitted })} />,
      );

      await user.type(
        screen.getByLabelText("Describe what you want Method guidance for"),
        "Recommend a safe next Method step",
      );
      await user.click(
        screen.getByRole("button", { name: "Request Method guidance" }),
      );
      expect(await screen.findByText("Created · Unbound")).toBeTruthy();

      rerender(
        <TaskWorkspace
          {...createProps({
            methodGuidanceAvailable: false,
            onTaskSubmitted,
          })}
        />,
      );

      expect(screen.getByText("Created · Unbound")).toBeTruthy();
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
          methodGuidanceBusy: false,
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
