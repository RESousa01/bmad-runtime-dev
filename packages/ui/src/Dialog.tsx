import type { ReactNode } from "react";
import {
  Dialog as AriaDialog,
  DialogTrigger as AriaDialogTrigger,
  Heading,
  Modal,
  ModalOverlay,
  type DialogTriggerProps,
} from "react-aria-components";

export interface DialogProps {
  /**
   * The dialog's name. Required, and rendered as its heading.
   *
   * FE-05's third acceptance criterion is that a screen reader receives a name
   * for *every* dialog. An optional prop would be omitted eventually, and the
   * omission is invisible to everyone who is not using a screen reader, so the
   * type is what enforces it.
   */
  title: string;
  /**
   * A sentence of context, announced with the name. Optional because not every
   * dialog has more to say than its title, but wired to aria-describedby when
   * present rather than left as decorative text.
   */
  description?: string;
  children: ReactNode;
  className?: string;
  /** Dismissable by Escape and by clicking outside. Defaults to true. */
  isDismissable?: boolean;
}

export function Dialog({
  title,
  description,
  children,
  className = "",
  isDismissable = true,
}: DialogProps) {
  const describedBy = description === undefined ? undefined : "sapphirus-dialog-description";

  return (
    <ModalOverlay className="sapphirus-modal-overlay" isDismissable={isDismissable}>
      <Modal className="sapphirus-modal">
        <AriaDialog
          // Spread rather than pass undefined: exactOptionalPropertyTypes
          // distinguishes "absent" from "present and undefined", and an
          // aria-describedby of undefined is not the same as no description.
          {...(describedBy === undefined ? {} : { "aria-describedby": describedBy })}
          className={["sapphirus-dialog", className].filter(Boolean).join(" ")}
        >
          <Heading className="sapphirus-dialog__title" slot="title">
            {title}
          </Heading>
          {description === undefined ? null : (
            <p className="sapphirus-dialog__description" id={describedBy}>
              {description}
            </p>
          )}
          {children}
        </AriaDialog>
      </Modal>
    </ModalOverlay>
  );
}

/**
 * Pairs a trigger with a dialog and owns the open state.
 *
 * Re-exported rather than re-implemented: RAC already gives deterministic
 * nesting -- Escape closes the innermost overlay and returns focus to whatever
 * opened it -- and reproducing that by hand is how focus ends up stranded on a
 * removed node.
 */
export function DialogTrigger({ children, ...props }: DialogTriggerProps) {
  return <AriaDialogTrigger {...props}>{children}</AriaDialogTrigger>;
}
