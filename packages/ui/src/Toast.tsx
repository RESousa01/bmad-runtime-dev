import {
  UNSTABLE_Toast as AriaToast,
  UNSTABLE_ToastContent as AriaToastContent,
  UNSTABLE_ToastList as AriaToastList,
  UNSTABLE_ToastQueue as AriaToastQueue,
  UNSTABLE_ToastRegion as AriaToastRegion,
  Text,
} from "react-aria-components";
import { Button } from "./Button";

/**
 * Transient status messages.
 *
 * react-aria-components ships this API as `UNSTABLE_*` in 1.19, so every use
 * of it is confined to this file and the rest of the product imports the names
 * below. When the prefix is dropped upstream, one file changes.
 *
 * A toast is for something the user does not have to act on. Anything that
 * needs a decision is a Dialog: a message that disappears on a timer is not a
 * place to put a choice, and an approval that can time out is not an approval.
 */
export interface ForgeToast {
  title: string;
  /** Adds detail. Announced with the title rather than replacing it. */
  description?: string;
  /**
   * `error` is assertive, everything else polite. An error interrupts because
   * the user's next action probably depends on it; a success does not, and
   * interrupting for one is how a screen reader becomes unusable.
   */
  tone?: "info" | "success" | "error";
}

export const toastQueue = new AriaToastQueue<ForgeToast>({
  maxVisibleToasts: 4,
});

export interface ToastRegionProps {
  className?: string;
}

export function ToastRegion({ className = "" }: ToastRegionProps) {
  return (
    <AriaToastRegion
      aria-label="Notifications"
      className={["sapphirus-toast-region", className].filter(Boolean).join(" ")}
      queue={toastQueue}
    >
      <AriaToastList<ForgeToast> className="sapphirus-toast-list">
        {({ toast }) => (
          <AriaToast
            className={`sapphirus-toast sapphirus-toast--${toast.content.tone ?? "info"}`}
            toast={toast}
          >
            <AriaToastContent className="sapphirus-toast__content">
              <Text className="sapphirus-toast__title" slot="title">
                {toast.content.title}
              </Text>
              {toast.content.description === undefined ? null : (
                <Text className="sapphirus-toast__description" slot="description">
                  {toast.content.description}
                </Text>
              )}
            </AriaToastContent>
            {/* Named, because "×" is not a name. */}
            <Button aria-label="Dismiss notification" slot="close" variant="quiet">
              ×
            </Button>
          </AriaToast>
        )}
      </AriaToastList>
    </AriaToastRegion>
  );
}

/**
 * Queues a toast.
 *
 * An error is left without a timeout on purpose: a message that matters enough
 * to interrupt should not vanish before it has been read, and the dismiss
 * control is how it goes away.
 */
export function showToast(toast: ForgeToast, timeoutMs?: number) {
  const timeout = timeoutMs ?? (toast.tone === "error" ? undefined : 6000);
  return toastQueue.add(toast, timeout === undefined ? {} : { timeout });
}
