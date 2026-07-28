import type { ReactNode } from "react";

export type InlineMessageTone = "info" | "success" | "warning" | "error";

export interface InlineMessageProps {
  tone: InlineMessageTone;
  /** The one-line statement. Say what happened, not that something happened. */
  title: string;
  children?: ReactNode;
  className?: string;
}

const ROLE: Record<InlineMessageTone, "status" | "alert"> = {
  info: "status",
  success: "status",
  warning: "status",
  error: "alert",
};

/**
 * A message attached to the surface it is about.
 *
 * Only `error` is an alert. An alert interrupts whatever a screen reader is
 * saying, which is right when the user's next action depends on it and hostile
 * when it does not -- a success that interrupts is how a screen reader becomes
 * unusable. Everything else is a polite status.
 *
 * Design specification 3.5: tone is never carried by colour alone, so the
 * tone word is rendered as text beside the message.
 */
export function InlineMessage({ tone, title, children, className = "" }: InlineMessageProps) {
  return (
    <div
      className={["sapphirus-inline-message", `sapphirus-inline-message--${tone}`, className]
        .filter(Boolean)
        .join(" ")}
      role={ROLE[tone]}
    >
      <span className="sapphirus-inline-message__tone">{tone}</span>
      <div className="sapphirus-inline-message__body">
        <strong className="sapphirus-inline-message__title">{title}</strong>
        {children === undefined ? null : (
          <div className="sapphirus-inline-message__detail">{children}</div>
        )}
      </div>
    </div>
  );
}
