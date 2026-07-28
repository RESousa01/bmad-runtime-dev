import { LockKeyhole } from "lucide-react";
import { SettingsRow } from "./SettingsRow";

/**
 * What the runtime is and what it does with local files.
 *
 * The seam is "facts the user cannot change". Everything on this pane is
 * read-only reporting, which is why it takes one string and no callbacks: a
 * control appearing here would belong to whichever section owns the thing it
 * changes.
 */
export interface GeneralSectionProps {
  readonly runtimeLabel: string;
}

export function GeneralSection({ runtimeLabel }: GeneralSectionProps) {
  return (
    <section aria-labelledby="settings-general-title">
      <header className="settings-pane__header">
        <h3 id="settings-general-title">General</h3>
        <p>Desktop runtime and local data behavior.</p>
      </header>
      <div className="settings-list">
        <SettingsRow
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
        <SettingsRow
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
  );
}
