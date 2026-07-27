import { useEffect, useRef, useState, type RefObject } from "react";

import { focusableElements } from "../lib/panelFocus";

export interface PaletteAction {
  readonly id: string;
  readonly label: string;
  readonly hint?: string;
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

  const normalized = query.trim().toLocaleLowerCase("en-US");
  const visible = actions.filter((action) =>
    action.label.toLocaleLowerCase("en-US").includes(normalized),
  );
  const clamped = Math.min(selected, Math.max(0, visible.length - 1));

  const runAction = (action: PaletteAction | undefined) => {
    if (action !== undefined) {
      onClose();
      action.run();
    }
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
                <button
                  aria-label={action.label}
                  className={`command-palette__action${index === clamped ? " command-palette__action--selected" : ""}`}
                  onClick={() => {
                    runAction(action);
                  }}
                  onMouseEnter={() => {
                    setSelected(index);
                  }}
                  type="button"
                >
                  <span>{action.label}</span>
                  {action.hint === undefined ? null : <kbd>{action.hint}</kbd>}
                </button>
              </li>
            ))
          )}
        </ul>
      </section>
    </div>
  );
}
