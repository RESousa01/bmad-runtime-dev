// @vitest-environment jsdom
import "../../test/setup";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type {
  AboutProjection,
  RetentionManifestProjection,
} from "../../lib/hostClient";
import {
  OFFBOARDING_ERASE_CONFIRMATION,
  SettingsDialog,
  type SettingsDialogProps,
} from "./SettingsDialog";

const about: AboutProjection = {
  appVersion: "0.1.0",
  installationId: "install_01ARZ3NDEKTSV4RRFFQ69G5FAV",
  bootMode: "ready",
  foundationPackageName: "bmad-method",
  foundationPackageVersion: "6.10.0",
  inactiveBuilderPackageCount: 2,
  updateConfigured: false,
  updateInstallAvailable: false,
};

const retentionManifest: RetentionManifestProjection = {
  schemaVersion: "sapphirus.retention-manifest.v1",
  categories: [
    { category: "workspace_and_authority_records", count: 4 },
    { category: "evidence_events", count: 128 },
    { category: "stored_payloads", count: 9 },
  ],
  retainedBytes: 262_144,
};

function createProps(
  overrides: Partial<SettingsDialogProps> = {},
): SettingsDialogProps {
  return {
    about,
    aboutStatus: "ready",
    agentStatusLabel: "Available",
    density: "comfortable",
    modelAccessDetail: "Deterministic development · review required",
    modelAccessLabel: "Development model",
    offboardingManifest: retentionManifest,
    offboardingNotice: null,
    offboardingStatus: "ready",
    onClose: vi.fn(),
    onDensityChange: vi.fn(),
    onEraseOffboarding: vi.fn(),
    onManageWorkspaces: vi.fn(),
    onOpenSkillsAndAgents: vi.fn(),
    onThemeChange: vi.fn(),
    preferencesNotice: null,
    runtimeLabel: "Local host ready",
    skillsAgentsAvailable: true,
    skillsAgentsStatusLabel: "Loaded",
    theme: "dark",
    updateStatusLabel: "Managed by your organization",
    workspaceDetail: "Read only",
    workspaceLabel: "bmad-runtime-dev",
    ...overrides,
  };
}

describe("SettingsDialog", () => {
  it("navigates all eight sections", async () => {
    const user = userEvent.setup();
    render(<SettingsDialog {...createProps()} />);

    expect(screen.getByRole("heading", { name: "General" })).toBeTruthy();
    for (const [button, heading] of [
      ["Appearance", "Appearance"],
      ["Agent & model", "Agent & model"],
      ["Workspaces", "Workspaces"],
      ["Skills & agents", "Skills & agents"],
      ["Updates", "Updates"],
      ["Local data", "Local data"],
      ["About", "About"],
    ] as const) {
      await user.click(screen.getByRole("button", { name: button }));
      expect(screen.getByRole("heading", { name: heading })).toBeTruthy();
    }
  });

  it("persists appearance changes through the provided callbacks", async () => {
    const props = createProps({ initialSection: "appearance" });
    const user = userEvent.setup();
    render(<SettingsDialog {...props} />);

    await user.click(screen.getByRole("button", { name: "Light" }));
    expect(props.onThemeChange).toHaveBeenCalledWith("light");
    await user.click(screen.getByRole("button", { name: "Compact" }));
    expect(props.onDensityChange).toHaveBeenCalledWith("compact");
  });

  it("shows a preferences notice when persistence is unavailable", () => {
    render(
      <SettingsDialog
        {...createProps({
          initialSection: "appearance",
          preferencesNotice: "Preferences are not saved in the browser demo.",
        })}
      />,
    );
    expect(
      screen.getByText("Preferences are not saved in the browser demo."),
    ).toBeTruthy();
  });

  it("keeps updates status-only with no install action", () => {
    render(<SettingsDialog {...createProps({ initialSection: "updates" })} />);
    expect(screen.getByText("Managed by your organization")).toBeTruthy();
    expect(screen.getByText("Not configured")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /install|check for update/i })).toBeNull();
  });

  it("shows the retention manifest and arms erase only on the exact phrase", async () => {
    const props = createProps({ initialSection: "offboarding" });
    const user = userEvent.setup();
    render(<SettingsDialog {...props} />);

    expect(screen.getByText("Workspace and authority records")).toBeTruthy();
    expect(screen.getByText("128")).toBeTruthy();
    expect(screen.getByText("256.0 KB")).toBeTruthy();

    const eraseButton = screen.getByRole("button", {
      name: /erase all local data/i,
    });
    expect(eraseButton.hasAttribute("disabled")).toBe(true);

    const confirmation = screen.getByRole("textbox", {
      name: "Erase confirmation phrase",
    });
    await user.type(confirmation, "erase");
    expect(eraseButton.hasAttribute("disabled")).toBe(true);
    expect(props.onEraseOffboarding).not.toHaveBeenCalled();

    await user.clear(confirmation);
    await user.type(confirmation, OFFBOARDING_ERASE_CONFIRMATION);
    expect(eraseButton.hasAttribute("disabled")).toBe(false);
    await user.click(eraseButton);
    expect(props.onEraseOffboarding).toHaveBeenCalledWith(
      OFFBOARDING_ERASE_CONFIRMATION,
    );
  });

  it("reports the erased terminal state", () => {
    render(
      <SettingsDialog
        {...createProps({
          initialSection: "offboarding",
          offboardingManifest: null,
          offboardingStatus: "erased",
        })}
      />,
    );
    expect(screen.getByText(/Local data was erased/u)).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: /erase all local data/i }),
    ).toBeNull();
  });

  it("surfaces version and installation identity in About", () => {
    render(<SettingsDialog {...createProps({ initialSection: "about" })} />);
    expect(screen.getByText("0.1.0")).toBeTruthy();
    expect(screen.getByText("install_01ARZ3NDEKTSV4RRFFQ69G5FAV")).toBeTruthy();
    expect(screen.getByText("bmad-method 6.10.0")).toBeTruthy();
  });

  it("reports About as unavailable without host data", () => {
    render(
      <SettingsDialog
        {...createProps({ about: null, aboutStatus: "unavailable", initialSection: "about" })}
      />,
    );
    expect(
      screen.getByText("Version information is unavailable in this mode."),
    ).toBeTruthy();
  });
});
