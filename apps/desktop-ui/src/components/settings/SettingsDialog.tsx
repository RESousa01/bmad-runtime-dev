import { Button } from "@sapphirus/ui";
import { X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type {
  AboutProjection,
  DensityPreference,
  ModelAuthStatusProjection,
  ModelProviderConnection,
  ModelSelection,
  RetentionManifestProjection,
  ThemePreference,
} from "../../lib/hostClient";
import { AboutSection, type AboutStatus } from "./AboutSection";
import { AppearanceSection } from "./AppearanceSection";
import { GeneralSection } from "./GeneralSection";
import {
  LocalDataSection,
  OFFBOARDING_ERASE_CONFIRMATION,
  type OffboardingStatus,
} from "./LocalDataSection";
import { ProviderSection } from "./ProviderSection";
import { SkillsAgentsSection } from "./SkillsAgentsSection";
import { UpdatesSection } from "./UpdatesSection";
import { WorkspacesSection } from "./WorkspacesSection";

export type SettingsSection =
  | "general"
  | "appearance"
  | "agent-model"
  | "workspaces"
  | "skills-agents"
  | "updates"
  | "offboarding"
  | "about";

/*
 * Re-exported, not redefined. Both belong to the section that gives them
 * meaning — `AboutStatus` describes what About can say, the erase phrase is the
 * arming mechanism of the Local data pane — but `App.tsx` already imports
 * `OffboardingStatus` from this module, and this file stays the address the
 * rest of the app knows Settings by.
 */
export type { AboutStatus, OffboardingStatus };
export { OFFBOARDING_ERASE_CONFIRMATION };

/// Every category the host reports, named for the person reading the panel
/// rather than for the table it came from.
///
/// This map had eight entries while `RETENTION_CATEGORIES`
/// (`crates/desktop-store/src/lib.rs`) had twenty-three, and nothing anywhere
/// noticed: the fallback in `LocalDataSection` quietly humanised the other
/// fifteen, so the panel showed `Memlog entries` and `Method decision
/// consumptions` in a section whose stated job is explaining what the app
/// keeps. The host list carries a comment recording that *it* had already
/// fallen behind two migrations — the same drift, one layer down.
///
/// `check-boundaries.mjs` now pins this map against the host list, so the next
/// category added on either side fails the gate instead of shipping. It pins it
/// by reading *this file* at a literal path, which is why the map did not move
/// into `LocalDataSection` with the rest of the retention rendering: moving it
/// would not fail that gate, it would leave the gate matching nothing and
/// silently checking nothing. The section takes it as a prop instead.
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
  model_selections: "Model choices",
  bmad_skill_sessions: "BMAD skill sessions",
  bmad_skill_session_events: "BMAD skill session events",
  bmad_artifacts: "BMAD artifacts",
  bmad_artifact_events: "BMAD artifact events",
  memory_entries: "Long-term memory entries",
  builder_lifecycles: "Builder lifecycles",
  builder_lifecycle_events: "Builder lifecycle events",
  bmad_installations: "BMAD installations",
  bmad_installation_events: "BMAD installation events",
};

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

/*
 * No icons. 3a is text-led, and eight glyphs down a nav of eight words added a
 * second thing to read without distinguishing anything the label did not
 * already say. The theme choices in `AppearanceSection` keep theirs, because
 * sun/moon/monitor carry the meaning there rather than repeating it.
 */
const sections: Array<{
  id: SettingsSection;
  label: string;
}> = [
  { id: "general", label: "General" },
  { id: "appearance", label: "Appearance" },
  { id: "agent-model", label: "Providers" },
  { id: "workspaces", label: "Workspaces" },
  { id: "skills-agents", label: "Skills & agents" },
  { id: "updates", label: "Updates" },
  { id: "offboarding", label: "Local data" },
  { id: "about", label: "About" },
];

/**
 * The Settings dialog: a frame, a nav, and exactly one section at a time.
 *
 * What is left here after the split is everything that is true of Settings as a
 * whole rather than of any one pane — the dialog role and label, the header,
 * which section is showing, and where focus goes on open. Each pane owns its
 * own props, its own state and its own heading, so a change to the provider
 * form cannot reach the theme picker and neither can be broken by a change to
 * this file that does not touch its props.
 *
 * Each section renders its own `<section aria-labelledby>` paired with the `h3`
 * inside it. The relationship has to stay inside one component: split across
 * two, the id and the element that carries it drift apart silently, and a
 * screen reader announces a dialog pane with no name.
 *
 * DOM order is the tab order and is deliberate: close button, then nav, then
 * pane. The focus effect below takes the first enabled button in the panel,
 * which is the close button because of that order — `App.test.tsx` asserts it,
 * and it is the right target because closing is the one action always
 * available.
 *
 * Section state is per-section, and dies with the section
 * -------------------------------------------------------
 * A pane's drafts — the model id being typed, the API key, the erase
 * confirmation phrase — live in the pane and are gone when the user navigates
 * to another one. Before the split they lived here and survived that
 * navigation. This is the one behavioural difference the split makes, and it is
 * the intended direction: `AppModals` already keys this dialog on its section
 * so reopening on a different section starts fresh, an unattended API key stops
 * being reachable sooner, and a primed erase confirmation does not wait around
 * while the user reads About.
 */
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
          {sections.map(({ id, label }) => (
            <Button
              {...(section === id ? { "aria-current": "page" as const } : {})}
              key={id}
              onPress={() => setSection(id)}
              variant="quiet"
            >
              {label}
            </Button>
          ))}
        </nav>

        {/*
          Each section is a direct child of the pane, with no wrapper. The
          `.settings-dialog__pane > section` rule that gives every pane its
          column layout and max width is a child combinator, so an element
          between the two silently unstyles all eight.
        */}
        <div className="settings-dialog__pane">
          {section === "general" ? (
            <GeneralSection runtimeLabel={runtimeLabel} />
          ) : section === "appearance" ? (
            <AppearanceSection
              density={density}
              onDensityChange={onDensityChange}
              onThemeChange={onThemeChange}
              preferencesNotice={preferencesNotice}
              theme={theme}
            />
          ) : section === "agent-model" ? (
            <ProviderSection
              agentStatusLabel={agentStatusLabel}
              modelAccessDetail={modelAccessDetail}
              modelAccessLabel={modelAccessLabel}
              modelAuthStatus={modelAuthStatus}
              modelSelection={modelSelection}
              modelSelectionBusy={modelSelectionBusy}
              modelSelectionNotice={modelSelectionNotice}
              onModelConnect={onModelConnect}
              onModelDisconnect={onModelDisconnect}
              onModelSelectionChange={onModelSelectionChange}
            />
          ) : section === "workspaces" ? (
            <WorkspacesSection
              onManageWorkspaces={onManageWorkspaces}
              workspaceDetail={workspaceDetail}
              workspaceLabel={workspaceLabel}
            />
          ) : section === "skills-agents" ? (
            <SkillsAgentsSection
              builderPackageCount={about?.inactiveBuilderPackageCount ?? null}
              onOpenSkillsAndAgents={onOpenSkillsAndAgents}
              skillsAgentsAvailable={skillsAgentsAvailable}
              skillsAgentsStatusLabel={skillsAgentsStatusLabel}
            />
          ) : section === "updates" ? (
            <UpdatesSection
              updateConfigured={about === null ? null : about.updateConfigured}
              updateStatusLabel={updateStatusLabel}
            />
          ) : section === "offboarding" ? (
            <LocalDataSection
              categoryLabels={retentionCategoryLabels}
              manifest={offboardingManifest}
              notice={offboardingNotice}
              onErase={onEraseOffboarding}
              status={offboardingStatus}
            />
          ) : (
            <AboutSection about={about} status={aboutStatus} />
          )}
        </div>
      </div>
    </section>
  );
}
