import { ProgressBar as AriaProgressBar } from "react-aria-components";

export interface ProgressProps {
  /** Names what is progressing. Required. */
  label: string;
  /** 0-100. Omit for work whose end is not known. */
  value?: number;
  className?: string;
}

/**
 * Determinate or indeterminate progress.
 *
 * Omitting `value` is not a shortcut for zero -- it means the end is genuinely
 * unknown, and it renders as indeterminate rather than as a bar that never
 * moves. A stalled-looking 0% is read as broken.
 */
export function Progress({ label, value, className = "" }: ProgressProps) {
  return (
    <AriaProgressBar
      aria-label={label}
      className={["sapphirus-progress", className].filter(Boolean).join(" ")}
      {...(value === undefined ? { isIndeterminate: true } : { value })}
    >
      {({ percentage, valueText }) => (
        <>
          <span className="sapphirus-progress__text">{valueText ?? "Working"}</span>
          <div className="sapphirus-progress__track">
            <div
              className="sapphirus-progress__fill"
              style={{ width: percentage === undefined ? "100%" : `${percentage}%` }}
            />
          </div>
        </>
      )}
    </AriaProgressBar>
  );
}
