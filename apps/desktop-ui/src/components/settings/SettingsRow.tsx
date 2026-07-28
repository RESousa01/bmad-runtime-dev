/**
 * The label / explanation / value line five of the settings sections are built
 * from.
 *
 * It stays one component rather than markup each section copies because its
 * shape is load-bearing outside itself: the About test reads a value with
 * `getByText("UI build").closest(".settings-row")`, which only resolves because
 * the label and the value are siblings inside a single `.settings-row`. Five
 * hand-rolled copies would each pass their own section's tests while drifting
 * out of that relationship one at a time.
 */
export interface SettingsRowProps {
  /** The sentence under the label. Explains what the value means, not what it is. */
  readonly detail: string;
  readonly label: string;
  readonly value: string;
}

export function SettingsRow({ detail, label, value }: SettingsRowProps) {
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
