// @vitest-environment jsdom
import "../../test/setup";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type {
  AboutProjection,
  ModelAuthStatusProjection,
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

const connectedModel: ModelAuthStatusProjection = {
  status: "development_ready",
  mode: "direct_provider_development",
  authEpoch: 3,
  developmentOnly: true,
  destinationLabel: "OpenAI · gpt-5-mini",
  providerId: "openai",
  modelId: "gpt-5-mini",
  signInAvailable: false,
  signOutAvailable: true,
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
    modelAuthStatus: connectedModel,
    modelSelection: { providerId: "openai", modelId: "gpt-5-mini" },
    modelSelectionBusy: false,
    modelSelectionNotice: null,
    offboardingManifest: retentionManifest,
    offboardingNotice: null,
    offboardingStatus: "ready",
    onClose: vi.fn(),
    onDensityChange: vi.fn(),
    onEraseOffboarding: vi.fn(),
    onManageWorkspaces: vi.fn(),
    onModelConnect: vi.fn(async () => undefined),
    onModelDisconnect: vi.fn(async () => undefined),
    onModelSelectionChange: vi.fn(async () => undefined),
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
  it("describes attached files without a second review step", () => {
    render(<SettingsDialog {...createProps()} />);

    expect(screen.getByText("Only explicitly added files are sent.")).toBeTruthy();
    expect(screen.queryByText("Confirm once")).toBeNull();
    expect(screen.queryByText(/need one confirmation/i)).toBeNull();
  });

  it("navigates all eight sections", async () => {
    const user = userEvent.setup();
    render(<SettingsDialog {...createProps()} />);

    expect(screen.getByRole("heading", { name: "General" })).toBeTruthy();
    for (const [button, heading] of [
      ["Appearance", "Appearance"],
      ["Providers", "Providers"],
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

  it("selects the provider and model in the frontend without exposing credentials", async () => {
    const props = createProps({ initialSection: "agent-model" });
    const user = userEvent.setup();
    render(<SettingsDialog {...props} />);

    expect(screen.getByText("OpenAI")).toBeTruthy();
    expect(screen.getByText("Azure OpenAI")).toBeTruthy();
    expect(screen.getByText("gpt-5-mini")).toBeTruthy();
    await user.click(screen.getByRole("radio", { name: "Azure OpenAI" }));
    const model = screen.getByRole("textbox", { name: "Model or deployment" });
    await user.clear(model);
    await user.type(model, "forge-gpt-5");
    await user.click(screen.getByRole("button", { name: "Use model" }));
    expect(props.onModelSelectionChange).toHaveBeenCalledWith({
      providerId: "azure",
      modelId: "forge-gpt-5",
    });
    expect(document.body.textContent).not.toContain("sk-");
  });

  it("connects OpenAI from the provider screen and clears the secret field", async () => {
    const props = createProps({
      initialSection: "agent-model",
      modelAuthStatus: {
        ...connectedModel,
        status: "unavailable",
        mode: "offline",
        developmentOnly: false,
        destinationLabel: "OpenAI · gpt-5-mini",
        signInAvailable: true,
        signOutAvailable: false,
      },
    });
    const user = userEvent.setup();
    const runtimeCredential = ["sk", "test", "runtime", "only"].join("-");
    render(<SettingsDialog {...props} />);

    const keyInput = screen.getByLabelText<HTMLInputElement>("OpenAI API key");
    expect(keyInput.type).toBe("password");
    await user.type(keyInput, runtimeCredential);
    await user.click(screen.getByRole("button", { name: "Connect OpenAI" }));

    expect(props.onModelConnect).toHaveBeenCalledWith({
      providerId: "openai",
      modelId: "gpt-5-mini",
      apiKey: runtimeCredential,
      endpoint: null,
      region: null,
    });
    expect(keyInput.value).toBe("");
    expect(document.body.textContent).not.toContain(runtimeCredential);
  });

  it("accepts a WebView-autofilled API key even when no React input event fires", async () => {
    const props = createProps({
      initialSection: "agent-model",
      modelAuthStatus: {
        ...connectedModel,
        status: "unavailable",
        mode: "offline",
        developmentOnly: false,
        destinationLabel: "OpenAI · gpt-5-mini",
        signInAvailable: true,
        signOutAvailable: false,
      },
    });
    const user = userEvent.setup();
    const runtimeCredential = ["sk", "autofilled", "runtime", "only"].join("-");
    render(<SettingsDialog {...props} />);

    const keyInput = screen.getByLabelText<HTMLInputElement>("OpenAI API key");
    const setNativeValue = Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value",
    )?.set;
    expect(setNativeValue).toBeTypeOf("function");
    setNativeValue?.call(keyInput, runtimeCredential);

    const connect = screen.getByRole("button", { name: "Connect OpenAI" });
    expect(connect).toHaveProperty("disabled", false);
    await user.click(connect);

    expect(props.onModelConnect).toHaveBeenCalledWith({
      providerId: "openai",
      modelId: "gpt-5-mini",
      apiKey: runtimeCredential,
      endpoint: null,
      region: null,
    });
    expect(keyInput.value).toBe("");
  });

  it("disconnects the active provider without asking for its key again", async () => {
    const props = createProps({ initialSection: "agent-model" });
    const user = userEvent.setup();
    render(<SettingsDialog {...props} />);

    await user.click(screen.getByRole("button", { name: "Disconnect OpenAI" }));
    expect(props.onModelDisconnect).toHaveBeenCalledTimes(1);
    expect(screen.queryByLabelText("OpenAI API key")).toBeNull();
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
    expect(screen.queryByText("Unavailable")).toBeNull();
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
