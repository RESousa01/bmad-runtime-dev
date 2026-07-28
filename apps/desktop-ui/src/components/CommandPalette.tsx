import { useEffect, useId, useRef, useState, type RefObject } from "react";

import { matchesTerms } from "../app/commands/commands";
import { focusableElements } from "../lib/panelFocus";

export interface PaletteAction {
  readonly id: string;
  readonly label: string;
  /**
   * Which group the command belongs to. Rendered as plain text, not as `kbd`:
   * this used to be a `hint` holding words like "drawer" inside a `<kbd>`,
   * which announces them as keyboard input.
   */
  readonly category?: string;
  /** Already formatted for this platform -- "Ctrl+K" or "⌘K". */
  readonly keybinding?: string;
  /** Searched alongside the label, so a command is findable by the word the user reaches for. */
  readonly keywords?: readonly string[];
  /**
   * Why the command cannot run, or undefined when it can.
   *
   * Present rather than absent when unavailable, because the palette shows the
   * command either way. Omitting an impossible command is indistinguishable
   * from the app not having the feature, and leaves the user with nothing to
   * act on; this way they read "Open a task first" and know what to do.
   */
  readonly unavailableReason?: string;
  readonly run: () => void;
}

export interface CommandPaletteProps {
  readonly actions: readonly PaletteAction[];
  readonly onClose: () => void;
  readonly open: boolean;
}

/**
 * Ctrl+K command palette in the OpenCode style: one overlay, fuzzy-free
 * substring filtering, keyboard-first. Actions are supplied by the shell so
 * the palette itself holds no authority.
 *
 * The outer component owns only the open/closed transition and where focus
 * returns to. Everything with per-opening state lives in the dialog below,
 * which is mounted only while the palette is open — so each opening starts from
 * a fresh query and selection because the component is new, not because an
 * effect wrote the state back to its initial value after the first paint.
 */
export function CommandPalette({ actions, onClose, open }: CommandPaletteProps) {
  const returnFocusRef = useRef<HTMLElement | null>(null);
  // Both the capture and the initial focus stay in this effect, and the dialog
  // below deliberately does not focus itself on mount. A child's mount effect
  // runs before its parent's, so a self-focusing dialog would move focus into
  // the search field first and this effect would then "remember" the search
  // field as the place to return to — leaving focus on a detached input, i.e.
  // on `<body>`, when the palette closed.
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (open) {
      // Remember where focus came from. Without this the palette dropped focus
      // on <body> when it closed, leaving the next keystroke with nowhere
      // sensible to land.
      const active = document.activeElement;
      returnFocusRef.current =
        active instanceof HTMLElement && active !== document.body ? active : null;
      inputRef.current?.focus();
      return;
    }
    const target = returnFocusRef.current;
    returnFocusRef.current = null;
    if (target?.isConnected && !target.closest("[inert]")) {
      target.focus();
    }
  }, [open]);

  if (!open) {
    return null;
  }

  return <CommandPaletteDialog actions={actions} inputRef={inputRef} onClose={onClose} />;
}

function CommandPaletteDialog({
  actions,
  inputRef,
  onClose,
}: {
  readonly actions: readonly PaletteAction[];
  readonly inputRef: RefObject<HTMLInputElement | null>;
  readonly onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const dialogRef = useRef<HTMLElement | null>(null);

  // `aria-modal="true"` is a claim about the rest of the page, not about the
  // dialog: it promises nothing behind the palette can be reached. Tab
  // containment is half of keeping that promise (the shell marks itself inert
  // while `paletteOpen` for the other half). Without it, Tab walked straight
  // out of the "modal" dialog and into the title bar behind it.
  //
  // The listener is on the document rather than on the dialog element so that
  // Tab is contained even when focus is somewhere a dialog-scoped handler would
  // never see it — on `document.body`, for instance, after a focused action
  // button is filtered out of the list by the next keystroke.
  useEffect(() => {
    const containTab = (event: globalThis.KeyboardEvent) => {
      const dialog = dialogRef.current;
      if (event.key !== "Tab" || dialog === null) {
        return;
      }
      const focusable = focusableElements(dialog);
      const first = focusable[0];
      const last = focusable.at(-1);
      if (first === undefined || last === undefined) {
        event.preventDefault();
        return;
      }
      const active = document.activeElement;
      if (!(active instanceof HTMLElement) || !focusable.includes(active)) {
        event.preventDefault();
        first.focus();
      } else if (event.shiftKey && active === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && active === last) {
        event.preventDefault();
        first.focus();
      }
    };

    document.addEventListener("keydown", containTab);
    return () => document.removeEventListener("keydown", containTab);
  }, []);

  const visible = actions.filter((action) =>
    matchesTerms([action.label, action.category ?? "", ...(action.keywords ?? [])], query),
  );
  const clamped = Math.min(selected, Math.max(0, visible.length - 1));

  const runAction = (action: PaletteAction | undefined) => {
    if (action === undefined) {
      return;
    }
    // An unavailable command is a no-op that leaves the palette open. Closing
    // on a refusal looks exactly like the command having run, which is worse
    // than nothing happening: the user believes the app did something.
    if (action.unavailableReason !== undefined) {
      return;
    }
    onClose();
    action.run();
  };

  return (
    <div className="command-palette__backdrop">
      {/*
        A button, not a click handler on the backdrop div: the scrim is a real
        control, so dismissing the palette by clicking away is reachable by
        every input method rather than by mouse alone. It is absolutely
        positioned, so it covers the backdrop without taking part in the flex
        layout that centres the dialog.
      */}
      <button
        aria-hidden="true"
        className="command-palette__scrim"
        onClick={onClose}
        tabIndex={-1}
        type="button"
      />
      <section
        aria-label="Command palette"
        aria-modal="true"
        className="command-palette"
        ref={dialogRef}
        role="dialog"
      >
        <input
          aria-label="Search commands"
          autoComplete="off"
          className="command-palette__input"
          onChange={(event) => {
            setQuery(event.target.value);
            setSelected(0);
          }}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              onClose();
            } else if (event.key === "ArrowDown") {
              event.preventDefault();
              setSelected((current) => Math.min(current + 1, visible.length - 1));
            } else if (event.key === "ArrowUp") {
              event.preventDefault();
              setSelected((current) => Math.max(current - 1, 0));
            } else if (event.key === "Enter") {
              event.preventDefault();
              runAction(visible[clamped]);
            }
          }}
          placeholder="Type a command…"
          ref={inputRef}
          spellCheck={false}
          type="text"
          value={query}
        />
        <ul aria-label="Commands" className="command-palette__list">
          {visible.length === 0 ? (
            <li className="command-palette__empty">No matching commands.</li>
          ) : (
            visible.map((action, index) => (
              <li key={action.id}>
                <PaletteRow
                  action={action}
                  onHover={() => setSelected(index)}
                  onRun={() => runAction(action)}
                  selected={index === clamped}
                />
              </li>
            ))
          )}
        </ul>
      </section>
    </div>
  );
}

/**
 * One command in the list, available or not.
 *
 * `aria-disabled` rather than `disabled`, because a disabled button is removed
 * from the tab order and from most screen-reader listings -- which would hide
 * the reason from exactly the users who most need it read aloud. The control
 * stays reachable and announces itself as unavailable; the refusal to run is
 * enforced in `runAction`, not by the DOM.
 */
function PaletteRow({
  action,
  onHover,
  onRun,
  selected,
}: {
  readonly action: PaletteAction;
  readonly onHover: () => void;
  readonly onRun: () => void;
  readonly selected: boolean;
}) {
  const reasonId = useId();
  const unavailable = action.unavailableReason !== undefined;
  const className = [
    "command-palette__action",
    selected ? "command-palette__action--selected" : "",
    unavailable ? "command-palette__action--unavailable" : "",
  ].filter(Boolean).join(" ");

  return (
    <button
      // The label alone is the accessible *name*; the reason goes in the
      // description channel so it is announced after the name rather than
      // becoming part of it. A command named "Close task Open a task first"
      // is unusable in a screen reader's element list.
      aria-describedby={unavailable ? reasonId : undefined}
      aria-disabled={unavailable || undefined}
      aria-label={action.label}
      className={className}
      onClick={onRun}
      onMouseEnter={onHover}
      type="button"
    >
      <span className="command-palette__action-label">{action.label}</span>
      {action.unavailableReason === undefined ? null : (
        <span className="command-palette__action-reason" id={reasonId}>
          {action.unavailableReason}
        </span>
      )}
      {action.category === undefined ? null : (
        <span className="command-palette__action-category">{action.category}</span>
      )}
      {/* A real `kbd` now that it holds a real chord. It used to carry words
          like "drawer", which announces category names as keyboard input. */}
      {action.keybinding === undefined ? null : <kbd>{action.keybinding}</kbd>}
    </button>
  );
}
