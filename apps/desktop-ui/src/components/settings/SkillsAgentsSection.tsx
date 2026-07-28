import { Button } from "@sapphirus/ui";
import { ChevronRight, Sparkles } from "lucide-react";
import { SettingsRow } from "./SettingsRow";

/**
 * Whether the skill and agent catalog is reachable, and the way in to it.
 *
 * `builderPackageCount` is a number rather than the About projection this row
 * used to read out of. The count is the only field of that projection this pane
 * has any business in, and handing the whole thing over would mean a change to
 * About's shape could break Skills — the coupling the split exists to remove.
 * `null` means the host has not answered yet, which is why the row is absent
 * rather than showing zero: "no Builder packages installed" and "we do not know
 * yet" are different claims.
 */
export interface SkillsAgentsSectionProps {
  readonly builderPackageCount: number | null;
  readonly onOpenSkillsAndAgents: () => void;
  readonly skillsAgentsAvailable: boolean;
  readonly skillsAgentsStatusLabel: string;
}

export function SkillsAgentsSection({
  builderPackageCount,
  onOpenSkillsAndAgents,
  skillsAgentsAvailable,
  skillsAgentsStatusLabel,
}: SkillsAgentsSectionProps) {
  return (
    <section aria-labelledby="settings-skills-title">
      <header className="settings-pane__header">
        <h3 id="settings-skills-title">Skills &amp; agents</h3>
        <p>
          BMAD provides the skill and agent foundation; Forge governs
          access and execution.
        </p>
      </header>
      <div className="settings-list">
        <SettingsRow
          detail="Access to installed skills and supported agent actions."
          label="Host catalog capability"
          value={skillsAgentsAvailable ? "Supported" : "Unavailable"}
        />
        <SettingsRow
          detail="Validated skill and agent information for this renderer session."
          label="Catalog projection"
          value={skillsAgentsStatusLabel}
        />
        {builderPackageCount === null ? null : (
          <SettingsRow
            detail="Installed Builder packages create inert drafts through reviewed runs."
            label="Builder packages"
            value={`${builderPackageCount} installed`}
          />
        )}
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
  );
}
