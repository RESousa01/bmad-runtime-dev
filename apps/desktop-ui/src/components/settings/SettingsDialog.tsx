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
  RetentionCategory,
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

/// Every category the host reports, named for the person reading the panel
/// rather than for the table it came from.
///
/// This map had eight entries while `RETENTION_CATEGORIES`
/// (`crates/desktop-store/src/lib.rs`) had twenty-three, and nothing anywhere
/// noticed: the fallback below quietly humanised the other fifteen, so the panel
/// showed `Memlog entries` and `Method decision consumptions` in a section whose
/// stated job is explaining what the app keeps. The host list carries a comment
/// recording that *it* had already fallen behind two migrations — the same drift,
/// one layer down.
///
/// `check-boundaries.mjs` now pins this map against the host list, so the next
/// category added on either side fails the gate instead of shipping.
const retentionCategoryLabels: Record<string, string> = {
  workspace_and_authority_records: "Workspace and authority records",
  evidence_events: "Evidence events",
  stored_payloads: "Stored payloads",
  approval_consumptions: "Approval consumptions",
  chat_conversations: "Conversations",
  chat_messages: "Messages",
  method_sessions: "Method sessions",
  method_checkpoints: "Method checkpoints",
  method_artifacts: "Method artifacts",
  method_decision_consumptions: "Method approvals you granted",
  capability_runs: "Capability runs",
  capability_results: "Capability results",
  memlog_entries: "Working-memory entries",
  architecture_sessions: "Architecture sessions",
  architecture_session_steps: "Architecture steps",
  builder_drafts: "Builder drafts",
  builder_draft_revisions: "Builder draft revisions",
  builder_analyses: "Builder analyses",
  builder_analysis_decisions: "Builder approvals you granted",
  execution_journals: "Execution journals",
  execution_checkpoints: "Change checkpoints",
  execution_results: "Change results",
  help_runs: "Help runs",
};

function retentionCategoryLabel(category: string): string {
  return (
    retentionCategoryLabels[category]
    ?? category.replaceAll("_", " ").replace(/^./u, (c) => c.toUpperCase())
  );
}

/// The same categories, arranged so the panel answers "what is this" instead
/// of only "how many rows".
///
/// Every one of the twenty-three rows used to carry the identical sentence
/// "Kept under the app's local authority store", which is a fact about the
/// list, not about any row in it. It is stated once above the list now, and
/// the explanation that varies — what kind of record this is — is stated once
/// per family.
///
/// This is presentation only. `retentionCategoryLabels` above stays the
/// pinned one-to-one map of every category the host reports; a category that
/// appears in no family below is still shown, under "Other records", so a new
/// host category can never go missing from this panel.
const retentionGroups: ReadonlyArray<{
  readonly categories: readonly string[];
  readonly detail: string;
  readonly id: string;
  readonly title: string;
}> = [
  {
    id: "conversations",
    title: "Conversations",
    detail:
      "The tasks you have opened and every message in them, including what the agent replied.",
    categories: ["chat_conversations", "chat_messages"],
  },
  {
    id: "authority",
    title: "Workspace grants and evidence",
    detail:
      "Which folders you granted access to, the approvals you spent, and the record of what was done under them.",
    categories: [
      "workspace_and_authority_records",
      "approval_consumptions",
      "evidence_events",
      "stored_payloads",
    ],
  },
  {
    id: "changes",
    title: "Governed changes",
    detail:
      "Edits proposed against your workspace, the checkpoint that makes applying one reversible, and the outcome.",
    categories: ["execution_journals", "execution_checkpoints", "execution_results"],
  },
  {
    id: "method",
    title: "Skills, agents, and runs",
    detail:
      "Work driven by the bundled skills and agents: the runs themselves, what they produced, and the approvals you granted them.",
    categories: [
      "method_sessions",
      "method_checkpoints",
      "method_artifacts",
      "method_decision_consumptions",
      "capability_runs",
      "capability_results",
      "help_runs",
      "memlog_entries",
      "architecture_sessions",
      "architecture_session_steps",
    ],
  },
  {
    id: "builder",
    title: "Builder authoring",
    detail:
      "Skill and agent drafts you authored here, their revision history, and the analyses run over them.",
    categories: [
      "builder_drafts",
      "builder_draft_revisions",
      "builder_analyses",
      "builder_analysis_decisions",
    ],
  },
];

function groupRetentionCategories(
  categories: readonly RetentionCategory[],
): ReadonlyArray<{
  readonly detail: string;
  readonly entries: readonly RetentionCategory[];
  readonly id: string;
  readonly title: string;
}> {
  const placed = new Set(retentionGroups.flatMap((group) => group.categories));
  const ungrouped = categories.filter((entry) => !placed.has(entry.category));
  return [
    ...retentionGroups
      .map((group) => ({
        detail: group.detail,
        entries: categories.filter((entry) => group.categories.includes(entry.category)),
        id: group.id,
        title: group.title,
      }))
      .filter((group) => group.entries.length > 0),
    ...(ungrouped.length > 0
      ? [
          {
            detail: "Kept in the same local store as the categories above.",
            entries: ungrouped,
            id: "other",
            title: "Other records",
          },
        ]
      : []),
  ];
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

  // Prop-to-state sync, and the rule is right that it is one. It is kept
  // because the host's answer is the authority here: after a connect or a
  // disconnect the dialog must show the provider and model the host actually
  // holds, not the draft the user typed on the way there.
  //
  // The residual defect is narrow and known: if the host pushes a new model
  // auth status while the user is mid-edit — a projection event rather than
  // their own submit — the draft is discarded under them. Neither documented
  // alternative is better here. Remounting on a `key` would also reset the
  // form notices and pull focus back to the first control; adjusting state
  // during render trades this rule for `react-hooks/set-state-in-render`.
  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    setSelectedProvider(modelSelection.providerId);
    setModelDraft(modelSelection.modelId);
  }, [modelSelection.modelId, modelSelection.providerId]);
  /* eslint-enable react-hooks/set-state-in-effect */

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
                  // Said "The signed host owns workspace authority." Nothing in
                  // this product verifies a code signature — there is no
                  // `WinVerifyTrust` call anywhere — and the binary is not
                  // signed at all: `tauri.conf.json` carries no
                  // `certificateThumbprint` and no `signCommand`, and the host
                  // reports `update_install_available: false` for exactly that
                  // reason. Asserting it in Settings, which is where a user goes
                  // to check provenance, is the worst possible place to be
                  // wrong about it.
                  detail="The desktop host process owns workspace authority. Forge does not verify this build's code signature."
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
                        // Same denylist as `hostClient/validation.ts`: the control characters are
                        // what the guard exists to reject, so a literal control class here is
                        // deliberate.
                        // eslint-disable-next-line no-control-regex
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
                  Uninstalling keeps this data unless you explicitly ask the
                  uninstaller to erase it; your workspace files are yours and
                  are never touched.
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
                  <p className="retention-lede">
                    Each number below counts records, not files, held in the
                    app&rsquo;s local authority store on this device.
                  </p>
                  <div className="retention-groups">
                    {groupRetentionCategories(offboardingManifest.categories).map(
                      (group) => (
                        <div className="retention-group" key={group.id}>
                          <h4>{group.title}</h4>
                          <p>{group.detail}</p>
                          <dl className="retention-group__counts">
                            {group.entries.map((entry) => (
                              <div key={entry.category}>
                                <dt>{retentionCategoryLabel(entry.category)}</dt>
                                <dd>{entry.count}</dd>
                              </div>
                            ))}
                          </dl>
                        </div>
                      ),
                    )}
                  </div>
                  <div className="settings-list">
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
                  {/*
                    `appVersion` is the crate version, so every build this
                    repository has ever produced answers "0.1.0" — a tester
                    reporting a fault on "0.1.0" identifies nothing. The detail
                    now says so rather than implying the value is a build
                    identity, and the row below carries one for the half of the
                    app that can supply it. A commit-pinned *host* identity needs
                    a field on the About projection, which `assertExactKeys`
                    requires to land on both sides at once.
                  */}
                  <Row
                    detail="Product version. It does not identify which build this is."
                    label="Version"
                    value={about.appVersion}
                  />
                  <Row
                    detail="Identifies this renderer bundle for support."
                    label="UI build"
                    value={__FORGE_UI_BUILD__}
                  />
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
