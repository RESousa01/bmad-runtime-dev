import { Button } from "@sapphirus/ui";
import { Check, Monitor, Moon, Sun } from "lucide-react";
import type { DensityPreference, ThemePreference } from "../../lib/hostClient";

/*
 * The nav above has no icons — 3a is text-led, and eight glyphs down a nav of
 * eight words added a second thing to read without distinguishing anything the
 * label did not already say. These three keep theirs, because sun/moon/monitor
 * carry the meaning here rather than repeating it.
 */
const themes: Array<{ icon: typeof Sun; id: ThemePreference; label: string }> = [
  { id: "light", label: "Light", icon: Sun },
  { id: "dark", label: "Dark", icon: Moon },
  { id: "system", label: "System", icon: Monitor },
];

/**
 * Theme and density.
 *
 * The seam is "settings that change how everything else looks". Both are
 * device-local preferences applied globally and neither has any relationship to
 * a workspace, a provider or the local store, so this pane can be re-themed
 * without reading a line of the rest of Settings.
 *
 * `preferencesNotice` arrives as text rather than as a "can persist" flag: what
 * the pane needs to say when persistence is unavailable is the caller's to
 * word, and a boolean would push that sentence in here where the reason for it
 * is not known.
 */
export interface AppearanceSectionProps {
  readonly density: DensityPreference;
  readonly onDensityChange: (density: DensityPreference) => void;
  readonly onThemeChange: (theme: ThemePreference) => void;
  readonly preferencesNotice: string | null;
  readonly theme: ThemePreference;
}

export function AppearanceSection({
  density,
  onDensityChange,
  onThemeChange,
  preferencesNotice,
  theme,
}: AppearanceSectionProps) {
  return (
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
  );
}
