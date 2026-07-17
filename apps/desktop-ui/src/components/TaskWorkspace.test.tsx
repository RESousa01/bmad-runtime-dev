// @vitest-environment jsdom
import "../test/setup";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { initialBmadRequestState } from "../lib/bmadModelProjection";
import type { ContextPreviewProjection } from "../lib/hostClient";
import { TaskWorkspace, type TaskWorkspaceProps } from "./TaskWorkspace";

function createContextPreview(
  relativePaths: readonly string[],
): ContextPreviewProjection {
  return {
    workspaceId: "workspace-1",
    manifestHash: "manifest-hash",
    items: relativePaths.map((relativePath, index) => ({
      relativePath,
      startLine: 1,
      endLine: 3,
      reason: "Selected for this task",
      contentHash: `hash-${index}`,
      classification: "source",
      redactions: [],
      byteCount: 24,
      estimatedTokens: 8,
      content: "authenticated source content",
    })),
    totalBytes: relativePaths.length * 24,
    estimatedTokens: relativePaths.length * 8,
    modelTarget: null,
  };
}

function createProps(
  overrides: Partial<TaskWorkspaceProps> = {},
): TaskWorkspaceProps {
  return {
    canAttachFiles: true,
    contextPreview: null,
    hostStatusLabel: "Desktop host ready",
    interactionDisabled: false,
    isBrowserDemo: false,
    isNewSession: true,
    isReadOnlyRecovery: false,
    methodGuidanceAvailable: true,
    methodGuidanceState: initialBmadRequestState,
    methodLibraryAvailable: true,
    modelAccessDetail: "Development connection · governed by BMAD review",
    modelAccessLabel: "Development model",
    onAttachFiles: vi.fn(),
    onOpenAgentSettings: vi.fn(),
    onOpenChanges: vi.fn(),
    onOpenMethodLibrary: vi.fn(),
    onOpenRunDetails: vi.fn(),
    onOpenSidebar: vi.fn(),
    onReviewRequest: vi.fn(async () => undefined),
    sessionTitle: "Method guidance",
    workspaceName: "sapphirus",
    ...overrides,
  };
}

describe("TaskWorkspace Method guidance", () => {
  it("keeps Files availability independent from Method submission", () => {
    render(<TaskWorkspace {...createProps()} />);

    expect(
      screen.getByLabelText("Describe what you want skill guidance for"),
    ).toHaveProperty("disabled", false);
    expect(screen.getByRole("button", { name: "Attach files" })).toHaveProperty(
      "disabled",
      false,
    );
    expect(screen.getByRole("button", { name: "Agent and model settings" })).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Review request" }),
    ).toHaveProperty("disabled", true);
    expect(screen.getAllByText("Local skill guidance").length).toBeGreaterThan(0);
    expect(
      screen.getAllByText(/inert local BMAD Help run/i).length,
    ).toBeGreaterThan(0);
  });

  it("presents the BMAD-guided agent, real model status, and review policy", async () => {
    const user = userEvent.setup();
    const onOpenAgentSettings = vi.fn();
    render(<TaskWorkspace {...createProps({ onOpenAgentSettings })} />);

    await user.click(screen.getByRole("button", { name: "Agent and model settings" }));

    const dialog = screen.getByRole("region", { name: "Agent and model" });
    expect(within(dialog).getByText("BMAD Help")).toBeTruthy();
    expect(within(dialog).getByRole("heading", { name: "Agent configuration" })).toBeTruthy();
    expect(within(dialog).getByText("Development model")).toBeTruthy();
    expect(within(dialog).getByText("Review before send")).toBeTruthy();

    await user.click(within(dialog).getByRole("button", { name: "Open settings" }));
    expect(onOpenAgentSettings).toHaveBeenCalledOnce();
    expect(onOpenAgentSettings).toHaveBeenCalledWith(screen.getByRole("button", { name: "Agent and model settings" }));
    expect(screen.queryByRole("region", { name: "Agent and model" })).toBeNull();
  });

  it("returns focus to the Agent trigger when Escape closes its menu", async () => {
    const user = userEvent.setup();
    render(<TaskWorkspace {...createProps()} />);
    const trigger = screen.getByRole("button", { name: "Agent and model settings" });

    await user.click(trigger);
    const openSettings = screen.getByRole("button", { name: "Open settings" });
    openSettings.focus();
    await user.keyboard("{Escape}");

    expect(screen.queryByRole("region", { name: "Agent and model" })).toBeNull();
    await waitFor(() => expect(document.activeElement).toBe(trigger));
  });

  it("uses canonical task copy with exactly one composer and no premature review action", () => {
    const { container } = render(<TaskWorkspace {...createProps()} />);

    expect(screen.getByText("Task")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "New task" })).toBeTruthy();
    expect(screen.getAllByRole("form", { name: "Task composer" })).toHaveLength(1);
    expect(container.querySelectorAll("textarea")).toHaveLength(1);
    expect(screen.queryByRole("button", { name: /review context/i })).toBeNull();
    expect(screen.queryByRole("button", { name: /session|inspector/i })).toBeNull();
  });

  it("opens Files, Changes, Run details, and Skills and agents through presentation callbacks", async () => {
    const user = userEvent.setup();
    const onAttachFiles = vi.fn();
    const onOpenChanges = vi.fn();
    const onOpenMethodLibrary = vi.fn();
    const onOpenRunDetails = vi.fn();
    render(<TaskWorkspace {...createProps({
      onAttachFiles,
      onOpenChanges,
      onOpenMethodLibrary,
      onOpenRunDetails,
    })} />);

    await user.click(screen.getByRole("button", { name: "Attach files" }));
    await user.click(screen.getByRole("button", { name: "Changes" }));
    await user.click(screen.getByRole("button", { name: "Run details" }));
    await user.click(screen.getByRole("button", { name: "Skills and agents" }));

    expect(onAttachFiles).toHaveBeenCalledOnce();
    expect(onOpenChanges).toHaveBeenCalledOnce();
    expect(onOpenRunDetails).toHaveBeenCalledOnce();
    expect(onOpenMethodLibrary).toHaveBeenCalledOnce();
  });

  it("renders local context preview chips without implying model attachment", () => {
    render(<TaskWorkspace {...createProps({
      contextPreview: createContextPreview([
        "src/main.ts",
        "src/features/task.tsx",
      ]),
    })} />);

    expect(screen.getByText("Local context preview")).toBeTruthy();
    expect(screen.getByText(/not included in a model request unless/i)).toBeTruthy();
    expect(screen.getByText("src/main.ts")).toBeTruthy();
    expect(screen.getByText("src/features/task.tsx")).toBeTruthy();
    expect(screen.queryByText("authenticated source content")).toBeNull();
  });

  it("submits the trimmed intent for review without claiming a send", async () => {
    const user = userEvent.setup();
    const onReviewRequest = vi.fn(async (_intent: string) => undefined);
    render(<TaskWorkspace {...createProps({ onReviewRequest })} />);

    await user.type(
      screen.getByLabelText("Describe what you want skill guidance for"),
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

  it("keeps request submission disabled when the parent interaction gate is closed", () => {
    render(<TaskWorkspace {...createProps({ interactionDisabled: true })} />);

    expect(screen.getByLabelText("Describe what you want skill guidance for")).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "Review request" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "Attach files" })).toHaveProperty("disabled", false);
  });

  it("preserves the draft when request creation rejects", async () => {
    const user = userEvent.setup();
    const onReviewRequest = vi.fn(async () => Promise.reject(new Error("closed")));
    const { container } = render(<TaskWorkspace {...createProps({ onReviewRequest })} />);

    const composer = screen.getByLabelText("Describe what you want skill guidance for");
    await user.type(composer, "Keep this exact intent");
    await user.click(screen.getByRole("button", { name: "Review request" }));

    await waitFor(() => expect(onReviewRequest).toHaveBeenCalledOnce());
    await waitFor(() => expect(composer).toHaveProperty("value", "Keep this exact intent"));
    expect(container.querySelector(".message--user")).toBeNull();
  });

  it("labels the initial retained-run lookup without claiming creation", () => {
    render(<TaskWorkspace {...createProps({
      methodGuidanceState: { kind: "creating", activity: "recovering" },
    })} />);

    expect(
      screen.getByLabelText("Describe what you want skill guidance for"),
    ).toHaveProperty("disabled", true);
    expect(
      screen.getByRole("button", { name: "Review request" }),
    ).toHaveProperty("disabled", true);
    expect(screen.getAllByText(/Checking for retained local skill guidance/i).length)
      .toBeGreaterThan(0);
    expect(screen.getByText("Checking · Local only")).toBeTruthy();
    expect(screen.queryByText("Preparing · Local only")).toBeNull();
  });

  it("claims creation only after the user submits an intent", async () => {
    const user = userEvent.setup();
    const onReviewRequest = vi.fn(() => new Promise<void>(() => undefined));
    const { rerender } = render(<TaskWorkspace {...createProps({ onReviewRequest })} />);

    await user.type(
      screen.getByLabelText("Describe what you want skill guidance for"),
      "Choose the next safe skill-guided step",
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
        screen.getByLabelText("Describe what you want skill guidance for"),
        "Recommend a safe next skill-guided step",
      );
      await user.click(
        screen.getByRole("button", { name: "Review request" }),
      );
      expect(await screen.findByText("Recommend a safe next skill-guided step")).toBeTruthy();

      rerender(
        <TaskWorkspace
          {...createProps({
            methodGuidanceAvailable: false,
            onReviewRequest,
          })}
        />,
      );

      expect(screen.getByText("Recommend a safe next skill-guided step")).toBeTruthy();
      expect(screen.queryByText("Demo response")).toBeNull();
      expect(screen.queryByText("Ready for review")).toBeNull();
    },
  );

  it("shows product onboarding without preview or demonstration content when Method guidance is unavailable", () => {
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
    expect(screen.getByText(/This workspace is open locally/i)).toBeTruthy();
    expect(screen.queryByText(/preview/i)).toBeNull();
    expect(screen.queryByText(/demonstration/i)).toBeNull();
    expect(screen.queryByText("Demo response")).toBeNull();
  });

  it("keeps browser preview status concise without implying local access", () => {
    render(
      <TaskWorkspace
        {...createProps({
          isBrowserDemo: true,
          hostStatusLabel: "Browser preview",
          methodGuidanceAvailable: false,
          methodGuidanceState: initialBmadRequestState,
        })}
      />,
    );

    expect(screen.getAllByText("Browser preview")).toHaveLength(1);
    expect(screen.getByText(/included sample project structure/i)).toBeTruthy();
    expect(screen.getByText(/No access to your device or a model/i)).toBeTruthy();
    expect(screen.queryByText("Demo mode")).toBeNull();
    expect(screen.queryByText("Local workspace")).toBeNull();
  });
});
