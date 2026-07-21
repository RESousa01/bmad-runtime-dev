import { useEffect, useRef, useState } from "react";

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
 */
export function CommandPalette({ actions, onClose, open }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setSelected(0);
      inputRef.current?.focus();
    }
  }, [open]);

  if (!open) {
    return null;
  }

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
    <div
      aria-hidden="true"
      className="command-palette__backdrop"
      onClick={onClose}
    >
      <div
        aria-label="Command palette"
        className="command-palette"
        onClick={(event) => {
          event.stopPropagation();
        }}
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
      </div>
    </div>
  );
}
