// @vitest-environment jsdom
import "../test/setup";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { UtilityPanel } from "./UtilityPanel";

function renderSettings(initialSettingsPage?: "general" | "appearance" | "skills-agents" | "workspace") {
  const onManageWorkspaces = vi.fn();
  const onOpenSkillsAndAgents = vi.fn();
  render(
    <UtilityPanel
      density="comfortable"
      {...(initialSettingsPage ? { initialSettingsPage } : {})}
      agentStatusLabel="Available"
      skillsAgentsAvailable
      mode="settings"
      modelAccessDetail="Deterministic development · local review required"
      modelAccessLabel="Development model"
      onClose={vi.fn()}
      onDensityChange={vi.fn()}
      onManageWorkspaces={onManageWorkspaces}
      onOpenSkillsAndAgents={onOpenSkillsAndAgents}
      onThemeChange={vi.fn()}
      runtimeLabel="Local host ready"
      theme="dark"
      skillsAgentsStatusLabel="Loaded"
      workspaceDetail="Governed edits"
      workspaceLabel="bmad-runtime-dev"
    />,
  );
  return { onManageWorkspaces, onOpenSkillsAndAgents };
}

describe("UtilityPanel settings", () => {
  it("uses a Codex-style settings navigation with one focused detail pane", async () => {
    const user = userEvent.setup();
    renderSettings();

    expect(screen.getByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Settings sections" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "General" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getByRole("heading", { name: "General" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Appearance" }));
    expect(screen.getByRole("heading", { name: "Appearance" })).toBeTruthy();
    expect(screen.getByRole("group", { name: "Theme" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Skills and agents" }));
    expect(screen.getByRole("heading", { name: "Skills and agents" })).toBeTruthy();
    expect(screen.getByText("BMAD Help")).toBeTruthy();
    expect(screen.getByText("Skill-guided request flow")).toBeTruthy();
    expect(screen.getByText("Development model")).toBeTruthy();
    expect(screen.getByText("Review before send")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Workspace" }));
    expect(screen.getByRole("heading", { name: "Workspace" })).toBeTruthy();
    expect(screen.getByText("bmad-runtime-dev")).toBeTruthy();
  });

  it("routes real settings actions through callbacks", async () => {
    const user = userEvent.setup();
    const { onManageWorkspaces, onOpenSkillsAndAgents } = renderSettings();

    await user.click(screen.getByRole("button", { name: "Skills and agents" }));
    await user.click(screen.getByRole("button", { name: "Open Skills and agents" }));
    await user.click(screen.getByRole("button", { name: "Workspace" }));
    await user.click(screen.getByRole("button", { name: "Manage workspaces" }));

    expect(onOpenSkillsAndAgents).toHaveBeenCalledOnce();
    expect(onManageWorkspaces).toHaveBeenCalledOnce();
  });

  it("can deep-link directly to Skills and agents from the agent control", () => {
    renderSettings("skills-agents");

    expect(screen.getByRole("button", { name: "Skills and agents" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getByRole("heading", { name: "Skills and agents" })).toBeTruthy();
    expect(screen.getByText("Host catalog capability")).toBeTruthy();
  });
});
