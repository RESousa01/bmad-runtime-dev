export type StatusTone = "active" | "pending" | "inactive" | "failed";

export interface StatusTextProps {
  tone: StatusTone;
  /** The state, in the user's words. "Running", not "RUNNING_STATE". */
  children: string;
  className?: string;
}

/**
 * A run or availability state.
 *
 * The word is the status; the dot is decoration. Design specification 3.5
 * requires text or a symbol alongside colour, and this is deliberately not a
 * component that can render a bare coloured dot -- `children` is a required
 * string, so there is no way to use it without saying what the colour means.
 */
export function StatusText({ tone, children, className = "" }: StatusTextProps) {
  return (
    <span
      className={["sapphirus-status", `sapphirus-status--${tone}`, className]
        .filter(Boolean)
        .join(" ")}
    >
      <span aria-hidden="true" className="sapphirus-status__dot" />
      {children}
    </span>
  );
}
