import { SettingsRow } from "./SettingsRow";

/**
 * Update posture, reported and nothing more.
 *
 * The seam is that this pane has no action. There is no signed release channel
 * in this build, so an "Install" or "Check now" control here would be a button
 * that cannot work; keeping the pane free of callbacks makes adding one a
 * visible change to this interface rather than a quiet addition inside a
 * 900-line file.
 *
 * `updateConfigured` is a tri-state on purpose. `null` is "the host has not
 * answered", which reads as "Unknown" — distinct from a host that answered
 * "not configured".
 */
export interface UpdatesSectionProps {
  readonly updateConfigured: boolean | null;
  readonly updateStatusLabel: string;
}

export function UpdatesSection({
  updateConfigured,
  updateStatusLabel,
}: UpdatesSectionProps) {
  return (
    <section aria-labelledby="settings-updates-title">
      <header className="settings-pane__header">
        <h3 id="settings-updates-title">Updates</h3>
        <p>Signed release updates are separate from local prototype testing.</p>
      </header>
      <div className="settings-list">
        <SettingsRow
          detail="Current update posture reported by the desktop host."
          label="Status"
          value={updateStatusLabel}
        />
        <SettingsRow
          detail="A signed release channel must be configured at build time."
          label="Release channel"
          value={
            updateConfigured === null
              ? "Unknown"
              : updateConfigured
                ? "Configured"
                : "Not configured"
          }
        />
      </div>
    </section>
  );
}
