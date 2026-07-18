import { Button } from "@sapphirus/ui";
import {
  Bot,
  Check,
  ChevronRight,
  Download,
  FolderKanban,
  Info,
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
import type {
  AboutProjection,
  DensityPreference,
  ThemePreference,
} from "../../lib/hostClient";

export type SettingsSection =
  | "general"
  | "appearance"
  | "agent-model"
  | "workspaces"
  | "skills-agents"
  | "updates"
  | "about";

export type AboutStatus = "loading" | "unavailable" | "ready";

export interface SettingsDialogProps {
  about: AboutProjection | null;
  aboutStatus: AboutStatus;
  agentStatusLabel: string;
  density: DensityPreference;
  initialSection?: SettingsSection;
  modelAccessDetail: string;
  modelAccessLabel: string;
  onClose: () => void;
  onDensityChange: (density: DensityPreference) => void;
  onManageWorkspaces: () => void;
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
  { id: "agent-model", label: "Agent & model", icon: Bot },
  { id: "workspaces", label: "Workspaces", icon: FolderKanban },
  { id: "skills-agents", label: "Skills & agents", icon: Sparkles },
  { id: "updates", label: "Updates", icon: Download },
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
  agentStatusLabel,
  density,
  initialSection = "general",
  modelAccessDetail,
  modelAccessLabel,
  onClose,
  onDensityChange,
  onManageWorkspaces,
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

  useEffect(() => {
    panelRef.current
      ?.querySelector<HTMLElement>("button:not([disabled])")
      ?.focus();
  }, []);

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
                <p>Desktop runtime, account, and local data behavior.</p>
              </header>
              <div className="settings-list">
                <Row
                  detail="The signed host owns workspace authority."
                  label="Desktop runtime"
                  value={runtimeLabel}
                />
                <div className="settings-row">
                  <span>
                    <strong>Account</strong>
                    <small>Organization sign-in is not configured for this build.</small>
                  </span>
                  <em>Local device</em>
                </div>
                <Row
                  detail="Exact context must be reviewed and approved first."
                  label="Request policy"
                  value="Review before send"
                />
                <div className="settings-row settings-row--stacked">
                  <span>
                    <strong>Local data</strong>
                    <small>
                      Files stay on this device unless you explicitly send approved
                      context. Governed changes and rollback availability are
                      verified by the desktop host.
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
                <h3 id="settings-agent-title">Agent &amp; model</h3>
                <p>Model access status and the request safety policy.</p>
              </header>
              <div className="settings-list">
                <Row detail={modelAccessDetail} label="Model access" value={modelAccessLabel} />
                <Row
                  detail="Skill-guided request flow"
                  label="Agent capability"
                  value={agentStatusLabel}
                />
                <Row
                  detail="Approval is explicit and single-use. Nothing is sent without it."
                  label="Request policy"
                  value="Review before send"
                />
                <Row
                  detail="Interactive sign-in becomes available with organization identity."
                  label="Sign-in"
                  value="Not configured"
                />
              </div>
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
                Files stay local unless you review, approve, and explicitly send
                exact context. Changes require governed review; rollback
                availability is verified by the desktop host.
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
                  BMAD provides the skill and agent foundation; Sapphirus governs
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
                    detail="Installed Builder packages awaiting a local activation decision."
                    label="Builder packages"
                    value={`${about.inactiveBuilderPackageCount} inactive`}
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
                <p>Updates are distributed and verified by your organization.</p>
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
                <Row
                  detail="In-app installation stays off until organization signing exists."
                  label="In-app install"
                  value="Unavailable"
                />
              </div>
            </section>
          ) : (
            <section aria-labelledby="settings-about-title">
              <header className="settings-pane__header">
                <h3 id="settings-about-title">About</h3>
                <p>Version and installation identity for support requests.</p>
              </header>
              {aboutStatus === "ready" && about !== null ? (
                <div className="settings-list">
                  <Row detail="Sapphirus desktop host." label="Version" value={about.appVersion} />
                  <div className="settings-row settings-row--stacked">
                    <span>
                      <strong>Installation id</strong>
                      <small>Identifies this install to your organization; shares no file content.</small>
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
