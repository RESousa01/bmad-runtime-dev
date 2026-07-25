import { Button } from "@sapphirus/ui";
import {
  Check,
  ChevronRight,
  Download,
  FolderKanban,
  Info,
  KeyRound,
  LockKeyhole,
  Monitor,
  Moon,
  Palette,
  PlugZap,
  SlidersHorizontal,
  Sparkles,
  Sun,
  Trash2,
  Unplug,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type {
  AboutProjection,
  DensityPreference,
  ModelAuthStatusProjection,
  ModelProviderConnection,
  ModelProviderId,
  ModelSelection,
  RetentionManifestProjection,
  ThemePreference,
} from "../../lib/hostClient";
import { ProviderIcon } from "./ProviderIcon";

export type SettingsSection =
  | "general"
  | "appearance"
  | "agent-model"
  | "workspaces"
  | "skills-agents"
  | "updates"
  | "offboarding"
  | "about";

export type AboutStatus = "loading" | "unavailable" | "ready";

export type OffboardingStatus =
  | "loading"
  | "unavailable"
  | "ready"
  | "erasing"
  | "erased";

// Must match the host-validated ADR-0004 phrase byte for byte.
export const OFFBOARDING_ERASE_CONFIRMATION = "erase-local-authority-data";

const retentionCategoryLabels: Record<string, string> = {
  workspace_and_authority_records: "Workspace and authority records",
  evidence_events: "Evidence events",
  stored_payloads: "Stored payloads",
  approval_consumptions: "Approval consumptions",
  method_sessions: "Method sessions",
  method_checkpoints: "Method checkpoints",
  execution_journals: "Execution journals",
  help_runs: "Help runs",
};

function retentionCategoryLabel(category: string): string {
  return (
    retentionCategoryLabels[category]
    ?? category.replaceAll("_", " ").replace(/^./u, (c) => c.toUpperCase())
  );
}

function formatRetainedBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export interface SettingsDialogProps {
  about: AboutProjection | null;
  aboutStatus: AboutStatus;
  agentStatusLabel: string;
  density: DensityPreference;
  initialSection?: SettingsSection;
  modelAccessDetail: string;
  modelAccessLabel: string;
  modelAuthStatus: ModelAuthStatusProjection | null;
  modelSelection: ModelSelection;
  modelSelectionBusy: boolean;
  modelSelectionNotice: string | null;
  onClose: () => void;
  onDensityChange: (density: DensityPreference) => void;
  offboardingManifest: RetentionManifestProjection | null;
  offboardingNotice: string | null;
  offboardingStatus: OffboardingStatus;
  onEraseOffboarding: (confirm: string) => void;
  onManageWorkspaces: () => void;
  onModelConnect: (connection: ModelProviderConnection) => Promise<void>;
  onModelDisconnect: () => Promise<void>;
  onModelSelectionChange: (selection: ModelSelection) => Promise<void>;
  onOpenSkillsAndAgents: () => void;
  onThemeChange: (theme: ThemePreference) => void;
  preferencesNotice: string | null;
  runtimeLabel: string;
  skillsAgentsAvailable: boolean;
  skillsAgentsStatusLabel: string;
  theme: ThemePreference;
  updateStatusLabel: string;
  workspaceDetail: string;
  workspaceLabel: string;
}

const sections: Array<{
  icon: typeof SlidersHorizontal;
  id: SettingsSection;
  label: string;
}> = [
  { id: "general", label: "General", icon: SlidersHorizontal },
  { id: "appearance", label: "Appearance", icon: Palette },
  { id: "agent-model", label: "Providers", icon: PlugZap },
  { id: "workspaces", label: "Workspaces", icon: FolderKanban },
  { id: "skills-agents", label: "Skills & agents", icon: Sparkles },
  { id: "updates", label: "Updates", icon: Download },
  { id: "offboarding", label: "Local data", icon: Trash2 },
  { id: "about", label: "About", icon: Info },
];

const themes: Array<{ icon: typeof Sun; id: ThemePreference; label: string }> = [
  { id: "light", label: "Light", icon: Sun },
  { id: "dark", label: "Dark", icon: Moon },
  { id: "system", label: "System", icon: Monitor },
];

function Row({
  detail,
  label,
  value,
}: {
  detail: string;
  label: string;
  value: string;
}) {
  return (
    <div className="settings-row">
      <span>
        <strong>{label}</strong>
        <small>{detail}</small>
      </span>
      <em>{value}</em>
    </div>
  );
}

export function SettingsDialog({
  about,
  aboutStatus,
  offboardingManifest,
  offboardingNotice,
  offboardingStatus,
  onEraseOffboarding,
  agentStatusLabel,
  density,
  initialSection = "general",
  modelAccessDetail,
  modelAccessLabel,
  modelAuthStatus,
  modelSelection,
  modelSelectionBusy,
  modelSelectionNotice,
  onClose,
  onDensityChange,
  onManageWorkspaces,
  onModelConnect,
  onModelDisconnect,
  onModelSelectionChange,
  onOpenSkillsAndAgents,
  onThemeChange,
  preferencesNotice,
  runtimeLabel,
  skillsAgentsAvailable,
  skillsAgentsStatusLabel,
  theme,
  updateStatusLabel,
  workspaceDetail,
  workspaceLabel,
}: SettingsDialogProps) {
  const panelRef = useRef<HTMLElement>(null);
  const [section, setSection] = useState<SettingsSection>(initialSection);
  const [eraseConfirmation, setEraseConfirmation] = useState("");
  const [selectedProvider, setSelectedProvider] = useState<ModelProviderId>(
    modelSelection.providerId,
  );
  const [modelDraft, setModelDraft] = useState(modelSelection.modelId);
  const [apiKeyDraft, setApiKeyDraft] = useState("");
  const [providerFormNotice, setProviderFormNotice] = useState<string | null>(null);
  const [azureEndpointDraft, setAzureEndpointDraft] = useState("");
  const [azureRegionDraft, setAzureRegionDraft] = useState("");
  const eraseArmed = eraseConfirmation === OFFBOARDING_ERASE_CONFIRMATION;
  const modelIsValid = /^[A-Za-z0-9][A-Za-z0-9._:/-]{0,127}$/u.test(modelDraft);
  const azureEndpointIsValid = selectedProvider !== "azure"
    || /^https:\/\/[A-Za-z0-9.-]+(?:\/openai\/v1\/?)?$/u.test(azureEndpointDraft);
  const selectedProviderName = selectedProvider === "azure" ? "Azure OpenAI" : "OpenAI";
  const selectedProviderConnected = modelAuthStatus?.status === "development_ready"
    && modelAuthStatus.mode === "direct_provider_development"
    && modelAuthStatus.providerId === selectedProvider;
  const canConnect = modelAuthStatus?.signInAvailable === true;

  useEffect(() => {
    panelRef.current
      ?.querySelector<HTMLElement>("button:not([disabled])")
      ?.focus();
  }, []);

  useEffect(() => {
    setSelectedProvider(modelSelection.providerId);
    setModelDraft(modelSelection.modelId);
  }, [modelSelection.modelId, modelSelection.providerId]);

  return (
    <section
      aria-label="Settings"
      aria-modal="true"
      className="settings-dialog"
      ref={panelRef}
      role="dialog"
    >
      <header className="settings-dialog__header">
        <h2>Settings</h2>
        <Button aria-label="Close settings" onPress={onClose} size="icon" variant="quiet">
          <X aria-hidden="true" size={17} />
        </Button>
      </header>
      <div className="settings-dialog__body">
        <nav aria-label="Settings sections" className="settings-dialog__nav">
          {sections.map(({ icon: Icon, id, label }) => (
            <Button
              {...(section === id ? { "aria-current": "page" as const } : {})}
              key={id}
              onPress={() => setSection(id)}
              variant="quiet"
            >
              <Icon aria-hidden="true" size={16} /> {label}
            </Button>
          ))}
        </nav>

        <div className="settings-dialog__pane">
          {section === "general" ? (
            <section aria-labelledby="settings-general-title">
              <header className="settings-pane__header">
                <h3 id="settings-general-title">General</h3>
                <p>Desktop runtime and local data behavior.</p>
              </header>
              <div className="settings-list">
                <Row
                  detail="The signed host owns workspace authority."
                  label="Desktop runtime"
                  value={runtimeLabel}
                />
                <div className="settings-row">
                  <span>
                    <strong>Prototype mode</strong>
                    <small>No account or organization sign-in is required.</small>
                  </span>
                  <em>Local only</em>
                </div>
                <Row
                  detail="Only explicitly added files are sent."
                  label="Attached files"
                  value="Explicit only"
                />
                <div className="settings-row settings-row--stacked">
                  <span>
                    <strong>Local data</strong>
                    <small>
                      Files stay on this device unless you explicitly add them
                      to a message. Governed changes and rollback availability
                      are verified by the desktop host.
                    </small>
                  </span>
                  <LockKeyhole aria-hidden="true" size={18} />
                </div>
              </div>
            </section>
          ) : section === "appearance" ? (
            <section aria-labelledby="settings-appearance-title">
              <header className="settings-pane__header">
                <h3 id="settings-appearance-title">Appearance</h3>
                <p>
                  {preferencesNotice
                    ?? "Preferences are saved on this device and restored on launch."}
                </p>
              </header>
              <fieldset aria-label="Theme" className="settings-choice-group">
                <legend>Theme</legend>
                <div className="preference-options">
                  {themes.map(({ icon: Icon, id, label }) => (
                    <Button
                      aria-label={label}
                      aria-pressed={theme === id}
                      className="preference-option"
                      key={id}
                      onPress={() => onThemeChange(id)}
                      variant="secondary"
                    >
                      <Icon aria-hidden="true" size={16} />
                      <span>
                        <strong>{label}</strong>
                        <small>
                          {id === "system" ? "Follow Windows" : `${label} interface`}
                        </small>
                      </span>
                      {theme === id ? (
                        <Check aria-hidden="true" className="preference-check" size={15} />
                      ) : null}
                    </Button>
                  ))}
                </div>
              </fieldset>
              <fieldset aria-label="Interface density" className="settings-choice-group">
                <legend>Interface density</legend>
                <div className="density-options">
                  <Button
                    aria-pressed={density === "comfortable"}
                    onPress={() => onDensityChange("comfortable")}
                    variant="secondary"
                  >
                    Comfortable
                  </Button>
                  <Button
                    aria-pressed={density === "compact"}
                    onPress={() => onDensityChange("compact")}
                    variant="secondary"
                  >
                    Compact
                  </Button>
                </div>
              </fieldset>
            </section>
          ) : section === "agent-model" ? (
            <section aria-labelledby="settings-agent-title">
              <header className="settings-pane__header">
                <h3 id="settings-agent-title">Providers</h3>
                <p>Model connections owned by the Forge desktop host.</p>
              </header>
              <div className="provider-summary">
                <span>
                  <small>Current model</small>
                  <strong>{modelAccessLabel}</strong>
                  <em>{modelAccessDetail}</em>
                </span>
              </div>
              <div aria-label="Model providers" className="provider-list">
                {([
                  {
                    id: "openai" as const,
                    name: "OpenAI",
                    setup: "Connect an API key for this Forge session.",
                  },
                  {
                    id: "azure" as const,
                    name: "Azure OpenAI",
                    setup: "Connect an Azure endpoint and API key for this session.",
                  },
                ]).map((provider) => {
                  const connected = modelAuthStatus?.status === "development_ready"
                    && modelAuthStatus.mode === "direct_provider_development"
                    && modelAuthStatus.providerId === provider.id;
                  return (
                    <label
                      className="provider-row provider-row--selectable"
                      data-selected={selectedProvider === provider.id || undefined}
                      key={provider.id}
                    >
                      <input
                        aria-label={provider.name}
                        checked={selectedProvider === provider.id}
                        name="model-provider"
                        onChange={() => setSelectedProvider(provider.id)}
                        type="radio"
                        value={provider.id}
                      />
                      <ProviderIcon className="provider-row__icon" provider={provider.id} />
                      <span className="provider-row__copy">
                        <strong>{provider.name}</strong>
                        <small>{connected ? "Connected for this app session" : provider.setup}</small>
                      </span>
                      <span className="provider-row__status">
                        <em data-connected={connected || undefined}>
                          {connected ? "Connected" : "Needs key"}
                        </em>
                        {connected && modelAuthStatus.modelId ? (
                          <code>{modelAuthStatus.modelId}</code>
                        ) : null}
                      </span>
                    </label>
                  );
                })}
              </div>
              <div className="provider-model-field">
                <label htmlFor="forge-model-id">Model or deployment</label>
                <div>
                  <input
                    aria-label="Model or deployment"
                    autoComplete="off"
                    id="forge-model-id"
                    onChange={(event) => setModelDraft(event.currentTarget.value)}
                    placeholder={selectedProvider === "azure" ? "Azure deployment name" : "OpenAI model ID"}
                    spellCheck={false}
                    value={modelDraft}
                  />
                  <Button
                    isDisabled={!modelIsValid || modelSelectionBusy}
                    onPress={() => void onModelSelectionChange({
                      providerId: selectedProvider,
                      modelId: modelDraft,
                    })}
                    size="small"
                    variant="primary"
                  >
                    {modelSelectionBusy ? "Applying…" : "Use model"}
                  </Button>
                </div>
                <small>
                  {modelSelectionNotice
                    ?? "Provider and model are selected here. API credentials are not persisted in frontend settings."}
                </small>
              </div>
              <div className="provider-connection">
                {selectedProviderConnected ? (
                  <div className="provider-connection__connected">
                    <span>
                      <Check aria-hidden="true" size={16} />
                      <strong>{selectedProviderName} connected</strong>
                      <small>The API key is held only in native memory until Forge exits.</small>
                    </span>
                    <Button
                      aria-label={`Disconnect ${selectedProviderName}`}
                      isDisabled={modelSelectionBusy || !modelAuthStatus?.signOutAvailable}
                      onPress={() => void onModelDisconnect()}
                      size="small"
                      variant="secondary"
                    >
                      <Unplug aria-hidden="true" size={14} /> Disconnect
                    </Button>
                  </div>
                ) : (
                  <form
                    aria-label={`Connect ${selectedProviderName}`}
                    onSubmit={(event) => {
                      event.preventDefault();
                      const form = event.currentTarget;
                      const credential = String(new FormData(form).get("apiKey") ?? "");
                      const keyIsValid = credential.length >= 8
                        && credential.length <= 4_096
                        && !/[\s\u0000-\u001f\u007f]/u.test(credential);
                      if (!keyIsValid) {
                        setProviderFormNotice("Enter a valid API key.");
                        return;
                      }
                      if (!canConnect || !modelIsValid || !azureEndpointIsValid) {
                        return;
                      }
                      setProviderFormNotice(null);
                      const connection: ModelProviderConnection = {
                        providerId: selectedProvider,
                        modelId: modelDraft,
                        apiKey: credential,
                        endpoint: selectedProvider === "azure" ? azureEndpointDraft : null,
                        region: selectedProvider === "azure"
                          ? azureRegionDraft.trim() || null
                          : null,
                      };
                      void onModelConnect(connection).finally(() => {
                        setApiKeyDraft("");
                        form.reset();
                      });
                    }}
                  >
                    <label htmlFor="forge-provider-api-key">
                      {selectedProviderName} API key
                    </label>
                    <div className="provider-connection__secret">
                      <KeyRound aria-hidden="true" size={15} />
                      <input
                        aria-label={`${selectedProviderName} API key`}
                        autoComplete="new-password"
                        id="forge-provider-api-key"
                        name="apiKey"
                        onChange={(event) => {
                          setApiKeyDraft(event.currentTarget.value);
                          setProviderFormNotice(null);
                        }}
                        placeholder="Paste API key"
                        spellCheck={false}
                        type="password"
                        value={apiKeyDraft}
                      />
                    </div>
                    {selectedProvider === "azure" ? (
                      <div className="provider-connection__azure">
                        <label htmlFor="forge-azure-endpoint">Azure endpoint</label>
                        <input
                          aria-label="Azure OpenAI endpoint"
                          id="forge-azure-endpoint"
                          onChange={(event) => setAzureEndpointDraft(event.currentTarget.value)}
                          placeholder="https://resource.openai.azure.com"
                          spellCheck={false}
                          value={azureEndpointDraft}
                        />
                        <label htmlFor="forge-azure-region">Region (optional)</label>
                        <input
                          aria-label="Azure OpenAI region"
                          id="forge-azure-region"
                          onChange={(event) => setAzureRegionDraft(event.currentTarget.value)}
                          placeholder="westeurope"
                          spellCheck={false}
                          value={azureRegionDraft}
                        />
                      </div>
                    ) : null}
                    <Button
                      aria-label={`Connect ${selectedProviderName}`}
                      isDisabled={
                        !canConnect
                        || !modelIsValid
                        || !azureEndpointIsValid
                        || modelSelectionBusy
                      }
                      size="small"
                      type="submit"
                      variant="primary"
                    >
                      <PlugZap aria-hidden="true" size={14} /> Connect
                    </Button>
                    <small>
                      {providerFormNotice
                        ?? (canConnect
                        ? "Sent once to the Rust host, kept in memory, never saved to Forge settings."
                        : "Provider connection requires the native Forge desktop host.")}
                    </small>
                  </form>
                )}
              </div>
              <p className="provider-footnote">
                Provider credentials stay in native memory for this app session and are cleared
                when Forge exits. Agent capability: {agentStatusLabel}.
              </p>
            </section>
          ) : section === "workspaces" ? (
            <section aria-labelledby="settings-workspace-title">
              <header className="settings-pane__header">
                <h3 id="settings-workspace-title">Workspaces</h3>
                <p>Manage the local folder grants used by this desktop.</p>
              </header>
              <div className="workspace-setting-summary">
                <span>
                  <small>Current workspace</small>
                  <strong>{workspaceLabel}</strong>
                  <em>{workspaceDetail}</em>
                </span>
                <LockKeyhole aria-hidden="true" size={18} />
              </div>
              <p className="settings-pane__copy">
                Files stay local unless you add them to a message and confirm
                sending. Changes use the Review tab; rollback availability is
                verified by the desktop host.
              </p>
              <Button
                className="settings-pane__action"
                onPress={onManageWorkspaces}
                variant="secondary"
              >
                Manage workspaces <ChevronRight aria-hidden="true" size={15} />
              </Button>
            </section>
          ) : section === "skills-agents" ? (
            <section aria-labelledby="settings-skills-title">
              <header className="settings-pane__header">
                <h3 id="settings-skills-title">Skills &amp; agents</h3>
                <p>
                  BMAD provides the skill and agent foundation; Forge governs
                  access and execution.
                </p>
              </header>
              <div className="settings-list">
                <Row
                  detail="Access to installed skills and supported agent actions."
                  label="Host catalog capability"
                  value={skillsAgentsAvailable ? "Supported" : "Unavailable"}
                />
                <Row
                  detail="Validated skill and agent information for this renderer session."
                  label="Catalog projection"
                  value={skillsAgentsStatusLabel}
                />
                {about ? (
                  <Row
                    detail="Installed Builder packages create inert drafts through reviewed runs."
                    label="Builder packages"
                    value={`${about.inactiveBuilderPackageCount} installed`}
                  />
                ) : null}
              </div>
              <Button
                className="settings-pane__action"
                isDisabled={!skillsAgentsAvailable}
                onPress={onOpenSkillsAndAgents}
                variant="secondary"
              >
                <Sparkles aria-hidden="true" size={16} /> Open Skills and agents
                <ChevronRight aria-hidden="true" size={15} />
              </Button>
            </section>
          ) : section === "updates" ? (
            <section aria-labelledby="settings-updates-title">
              <header className="settings-pane__header">
                <h3 id="settings-updates-title">Updates</h3>
                <p>Signed release updates are separate from local prototype testing.</p>
              </header>
              <div className="settings-list">
                <Row
                  detail="Current update posture reported by the desktop host."
                  label="Status"
                  value={updateStatusLabel}
                />
                <Row
                  detail="A signed release channel must be configured at build time."
                  label="Release channel"
                  value={
                    about === null
                      ? "Unknown"
                      : about.updateConfigured
                        ? "Configured"
                        : "Not configured"
                  }
                />
              </div>
            </section>
          ) : section === "offboarding" ? (
            <section aria-labelledby="settings-offboarding-title">
              <header className="settings-pane__header">
                <h3 id="settings-offboarding-title">Local data</h3>
                <p>
                  What this app keeps on this device, and how to erase it.
                  Uninstalling never deletes this data; your workspace files
                  are yours and are never touched.
                </p>
              </header>
              {offboardingStatus === "erased" ? (
                <p className="settings-pane__copy" role="status">
                  Local data was erased. The session is now read-only; restart
                  the app to start with a fresh identity.
                </p>
              ) : offboardingStatus === "unavailable" ? (
                <p className="settings-pane__copy" role="status">
                  Local data details are unavailable in this mode.
                </p>
              ) : offboardingManifest === null ? (
                <p className="settings-pane__copy" role="status">
                  Loading local data details…
                </p>
              ) : (
                <>
                  <div className="settings-list">
                    {offboardingManifest.categories.map((entry) => (
                      <Row
                        detail="Kept under the app's local authority store."
                        key={entry.category}
                        label={retentionCategoryLabel(entry.category)}
                        value={`${entry.count}`}
                      />
                    ))}
                    <Row
                      detail="Encrypted local store, database, and key material."
                      label="Total retained"
                      value={formatRetainedBytes(offboardingManifest.retainedBytes)}
                    />
                  </div>
                  <fieldset
                    aria-label="Erase local data"
                    className="settings-choice-group settings-choice-group--danger"
                  >
                    <legend>Erase local data</legend>
                    <p className="settings-pane__copy">
                      Erasing signs you out, revokes every workspace grant, and
                      destroys the local store key so remaining bytes are
                      undecryptable. This cannot be undone. Type{" "}
                      <code>{OFFBOARDING_ERASE_CONFIRMATION}</code> to confirm.
                    </p>
                    <input
                      aria-label="Erase confirmation phrase"
                      autoComplete="off"
                      className="settings-erase-confirmation"
                      onChange={(event) => setEraseConfirmation(event.target.value)}
                      placeholder={OFFBOARDING_ERASE_CONFIRMATION}
                      spellCheck={false}
                      type="text"
                      value={eraseConfirmation}
                    />
                    <Button
                      className="settings-pane__action"
                      isDisabled={!eraseArmed || offboardingStatus === "erasing"}
                      onPress={() => onEraseOffboarding(eraseConfirmation)}
                      variant="secondary"
                    >
                      <Trash2 aria-hidden="true" size={15} />{" "}
                      {offboardingStatus === "erasing"
                        ? "Erasing local data…"
                        : "Erase all local data"}
                    </Button>
                  </fieldset>
                </>
              )}
              {offboardingNotice ? (
                <p className="settings-pane__copy" role="status">
                  {offboardingNotice}
                </p>
              ) : null}
            </section>
          ) : (
            <section aria-labelledby="settings-about-title">
              <header className="settings-pane__header">
                <h3 id="settings-about-title">About</h3>
                <p>Version and installation identity for support requests.</p>
              </header>
              {aboutStatus === "ready" && about !== null ? (
                <div className="settings-list">
                  <Row detail="Forge desktop host." label="Version" value={about.appVersion} />
                  <div className="settings-row settings-row--stacked">
                    <span>
                      <strong>Installation id</strong>
                      <small>Identifies this local installation for support; shares no file content.</small>
                    </span>
                    <code className="settings-install-id">{about.installationId}</code>
                  </div>
                  <Row
                    detail="Bundled BMAD foundation package."
                    label="Foundation"
                    value={`${about.foundationPackageName} ${about.foundationPackageVersion}`}
                  />
                  <Row
                    detail="Host boot mode for this session."
                    label="Boot mode"
                    value={about.bootMode === "ready" ? "Ready" : "Read-only recovery"}
                  />
                </div>
              ) : (
                <p className="settings-pane__copy" role="status">
                  {aboutStatus === "loading"
                    ? "Loading version information…"
                    : "Version information is unavailable in this mode."}
                </p>
              )}
            </section>
          )}
        </div>
      </div>
    </section>
  );
}
