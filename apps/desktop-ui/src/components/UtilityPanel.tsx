import { Button } from "@sapphirus/ui";
import { Check, Monitor, Moon, Sun, X } from "lucide-react";
import { useEffect, useRef } from "react";
import type { DensityPreference, ThemePreference } from "../data/demo";

export interface UtilityPanelProps {
  density: DensityPreference;
  mode: "account" | "settings";
  onClose: () => void;
  onDensityChange: (density: DensityPreference) => void;
  onThemeChange: (theme: ThemePreference) => void;
  theme: ThemePreference;
}

const themes: Array<{ icon: typeof Sun; id: ThemePreference; label: string }> = [
  { id: "light", label: "Light", icon: Sun },
  { id: "dark", label: "Dark", icon: Moon },
  { id: "system", label: "System", icon: Monitor },
];

export function UtilityPanel({
  density,
  mode,
  onClose,
  onDensityChange,
  onThemeChange,
  theme,
}: UtilityPanelProps) {
  const panelRef = useRef<HTMLElement>(null);

  useEffect(() => {
    panelRef.current?.querySelector<HTMLElement>("button:not([disabled])")?.focus();
  }, []);

  return (
    <section
      aria-label={mode === "settings" ? "Settings" : "Account"}
      className="utility-panel"
      ref={panelRef}
      role="dialog"
    >
      <header>
        <h2>{mode === "settings" ? "Settings" : "Account"}</h2>
        <Button aria-label={`Close ${mode}`} onPress={onClose} size="icon" variant="quiet">
          <X aria-hidden="true" size={17} />
        </Button>
      </header>
      {mode === "settings" ? (
        <>
          <fieldset>
            <legend>Appearance</legend>
            <div className="preference-options">
              {themes.map(({ icon: Icon, id, label }) => (
                <Button
                  aria-pressed={theme === id}
                  className="preference-option"
                  key={id}
                  onPress={() => onThemeChange(id)}
                  variant="secondary"
                >
                  <Icon aria-hidden="true" size={16} />
                  {label}
                  {theme === id ? <Check aria-hidden="true" className="preference-check" size={15} /> : null}
                </Button>
              ))}
            </div>
          </fieldset>
          <fieldset>
            <legend>Density</legend>
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
          <p>Appearance preferences stay on this device.</p>
        </>
      ) : (
        <div className="account-summary">
          <div aria-hidden="true" className="account-avatar">RS</div>
          <div><strong>Desktop preview</strong><span>Sign-in is not configured</span></div>
          <span className="account-status"><span className="status-dot status-dot--preview" /> Internal build</span>
        </div>
      )}
    </section>
  );
}
