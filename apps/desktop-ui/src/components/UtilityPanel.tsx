import { Button } from "@sapphirus/ui";
import {
  Check,
  ChevronRight,
  Download,
  FolderKanban,
  LockKeyhole,
  Monitor,
  Moon,
  Palette,
  SlidersHorizontal,
  Sparkles,
  Sun,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { DensityPreference, ThemePreference } from "../data/demo";

export type SettingsPage = "general" | "appearance" | "skills-agents" | "workspace";

export interface UtilityPanelProps {
  agentStatusLabel: string;
  density: DensityPreference;
  initialSettingsPage?: SettingsPage;
  mode: "account" | "settings";
  modelAccessDetail: string;
  modelAccessLabel: string;
  onClose: () => void;
  onDensityChange: (density: DensityPreference) => void;
  onInstallAppUpdate: () => void;
  onManageWorkspaces: () => void;
  onOpenSkillsAndAgents: () => void;
  onThemeChange: (theme: ThemePreference) => void;
  runtimeLabel: string;
  skillsAgentsAvailable: boolean;
  skillsAgentsStatusLabel: string;
  theme: ThemePreference;
  updateBusy: boolean;
  updateStatusLabel: string;
  workspaceDetail: string;
  workspaceLabel: string;
}

const themes: Array<{ icon: typeof Sun; id: ThemePreference; label: string }> = [
  { id: "light", label: "Light", icon: Sun },
  { id: "dark", label: "Dark", icon: Moon },
  { id: "system", label: "System", icon: Monitor },
];

export function UtilityPanel({
  agentStatusLabel,
  density,
  initialSettingsPage = "general",
  mode,
  modelAccessDetail,
  modelAccessLabel,
  onClose,
  onDensityChange,
  onInstallAppUpdate,
  onManageWorkspaces,
  onOpenSkillsAndAgents,
  onThemeChange,
  runtimeLabel,
  skillsAgentsAvailable,
  skillsAgentsStatusLabel,
  theme,
  updateBusy,
  updateStatusLabel,
  workspaceDetail,
  workspaceLabel,
}: UtilityPanelProps) {
  const panelRef = useRef<HTMLElement>(null);
  const [settingsPage, setSettingsPage] = useState<SettingsPage>(initialSettingsPage);

  useEffect(() => {
    panelRef.current?.querySelector<HTMLElement>("button:not([disabled])")?.focus();
  }, []);

  return (
    <section
      aria-label={mode === "settings" ? "Settings" : "Account"}
      aria-modal="true"
      className={`utility-panel utility-panel--${mode}`}
      ref={panelRef}
      role="dialog"
    >
      <header className="utility-panel__header">
        <div>
          <h2>{mode === "settings" ? "Settings" : "Account"}</h2>
        </div>
        <Button aria-label={`Close ${mode}`} onPress={onClose} size="icon" variant="quiet">
          <X aria-hidden="true" size={17} />
        </Button>
      </header>
      {mode === "settings" ? (
        <div className="settings-layout">
          <nav aria-label="Settings sections" className="settings-nav">
            <Button
              {...(settingsPage === "general" ? { "aria-current": "page" as const } : {})}
              onPress={() => setSettingsPage("general")}
              variant="quiet"
            >
              <SlidersHorizontal aria-hidden="true" size={16} /> General
            </Button>
            <Button
              {...(settingsPage === "appearance" ? { "aria-current": "page" as const } : {})}
              onPress={() => setSettingsPage("appearance")}
              variant="quiet"
            >
              <Palette aria-hidden="true" size={16} /> Appearance
            </Button>
            <Button
              {...(settingsPage === "skills-agents" ? { "aria-current": "page" as const } : {})}
              onPress={() => setSettingsPage("skills-agents")}
              variant="quiet"
            >
              <Sparkles aria-hidden="true" size={16} /> Skills and agents
            </Button>
            <Button
              {...(settingsPage === "workspace" ? { "aria-current": "page" as const } : {})}
              onPress={() => setSettingsPage("workspace")}
              variant="quiet"
            >
              <FolderKanban aria-hidden="true" size={16} /> Workspace
            </Button>
          </nav>

          <div className="settings-pane">
            {settingsPage === "general" ? (
              <section aria-labelledby="settings-general-title">
                <header className="settings-pane__header">
                  <h3 id="settings-general-title">General</h3>
                  <p>Desktop runtime, request safety, and local data behavior.</p>
                </header>
                <div className="settings-list">
                  <div className="settings-row">
                    <span><strong>Desktop runtime</strong><small>The signed host owns workspace authority.</small></span>
                    <em>{runtimeLabel}</em>
                  </div>
                  <div className="settings-row">
                    <span><strong>App updates</strong><small>Checks the signed release channel and installs an eligible update.</small></span>
                    <em>{updateStatusLabel}</em>
                  </div>
                  <div className="settings-row">
                    <span><strong>Request policy</strong><small>Exact context must be reviewed and approved first.</small></span>
                    <em>Review before send</em>
                  </div>
                  <div className="settings-row settings-row--stacked">
                    <span><strong>Local data</strong><small>Files stay on this device unless you explicitly send approved context. Governed changes and rollback availability are verified by the desktop host.</small></span>
                    <LockKeyhole aria-hidden="true" size={18} />
                  </div>
                </div>
                <Button className="settings-pane__action" isDisabled={updateBusy} onPress={onInstallAppUpdate} variant="secondary">
                  <Download aria-hidden="true" size={16} /> {updateBusy ? "Checking for updates" : "Check for updates"}
                </Button>
              </section>
            ) : settingsPage === "appearance" ? (
              <section aria-labelledby="settings-appearance-title">
                <header className="settings-pane__header">
                  <h3 id="settings-appearance-title">Appearance</h3>
                  <p>These preferences apply to the current desktop session.</p>
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
                        <span><strong>{label}</strong><small>{id === "system" ? "Follow Windows" : `${label} interface`}</small></span>
                        {theme === id ? <Check aria-hidden="true" className="preference-check" size={15} /> : null}
                      </Button>
                    ))}
                  </div>
                </fieldset>
                <fieldset aria-label="Interface density" className="settings-choice-group">
                  <legend>Interface density</legend>
                  <div className="density-options">
                    <Button aria-pressed={density === "comfortable"} onPress={() => onDensityChange("comfortable")} variant="secondary">Comfortable</Button>
                    <Button aria-pressed={density === "compact"} onPress={() => onDensityChange("compact")} variant="secondary">Compact</Button>
                  </div>
                </fieldset>
              </section>
            ) : settingsPage === "skills-agents" ? (
              <section aria-labelledby="settings-skills-title">
                <header className="settings-pane__header">
                  <h3 id="settings-skills-title">Skills and agents</h3>
                  <p>BMAD provides the skill and agent foundation; Sapphirus governs access and execution.</p>
                </header>
                <div className="settings-list">
                  <div className="settings-row">
                    <span><strong>Host catalog capability</strong><small>Access to installed skills and supported agent actions.</small></span>
                    <em>{skillsAgentsAvailable ? "Supported" : "Unavailable"}</em>
                  </div>
                  <div className="settings-row">
                    <span><strong>Catalog projection</strong><small>Validated skill and agent information for this renderer session.</small></span>
                    <em>{skillsAgentsStatusLabel}</em>
                  </div>
                  <div className="settings-row">
                    <span><strong>Agent capability</strong><small><span>BMAD Help</span><span>Skill-guided request flow</span></small></span>
                    <em>{agentStatusLabel}</em>
                  </div>
                  <div className="settings-row">
                    <span><strong>Model access</strong><small>{modelAccessDetail}</small></span>
                    <em>{modelAccessLabel}</em>
                  </div>
                  <div className="settings-row">
                    <span><strong>Request policy</strong><small>Approval is explicit and single-use.</small></span>
                    <em>Review before send</em>
                  </div>
                </div>
                <Button className="settings-pane__action" isDisabled={!skillsAgentsAvailable} onPress={onOpenSkillsAndAgents} variant="secondary">
                  <Sparkles aria-hidden="true" size={16} /> Open Skills and agents <ChevronRight aria-hidden="true" size={15} />
                </Button>
              </section>
            ) : (
              <section aria-labelledby="settings-workspace-title">
                <header className="settings-pane__header">
                  <h3 id="settings-workspace-title">Workspace</h3>
                  <p>Manage the local folder grant used by this desktop.</p>
                </header>
                <div className="workspace-setting-summary">
                  <span><small>Current workspace</small><strong>{workspaceLabel}</strong><em>{workspaceDetail}</em></span>
                  <LockKeyhole aria-hidden="true" size={18} />
                </div>
                <p className="settings-pane__copy">Files stay local unless you review, approve, and explicitly send exact context. Changes require governed review; rollback availability is verified by the desktop host.</p>
                <Button className="settings-pane__action" onPress={onManageWorkspaces} variant="secondary">
                  Manage workspaces <ChevronRight aria-hidden="true" size={15} />
                </Button>
              </section>
            )}
          </div>
        </div>
      ) : (
        <div className="account-summary">
          <div aria-hidden="true" className="account-avatar"><Monitor size={18} /></div>
          <div><strong>Desktop account</strong><span>Sign-in is not configured</span></div>
          <span className="account-status"><span className="status-dot" /> {runtimeLabel}</span>
        </div>
      )}
    </section>
  );
}
