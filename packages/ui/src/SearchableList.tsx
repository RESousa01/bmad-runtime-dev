import type { Key } from "react";
import { useId, useMemo, useState } from "react";
import { ActionList, type ActionListItem } from "./ActionList";

export interface SearchableListProps {
  items: readonly ActionListItem[];
  /** Names both the field and the list. */
  label: string;
  placeholder?: string;
  className?: string;
  onAction?: (id: string) => void;
}

/**
 * A filter field over an ActionList.
 *
 * The live region is the point of this component, not the filtering. Typing in
 * a box that silently reorders a list below it tells a sighted user everything
 * and a screen-reader user nothing: they hear their own keystrokes and no
 * result. FE-06's second acceptance criterion is that the filtered count is
 * announced, so the count is rendered into a polite live region and phrased as
 * a sentence rather than a bare number.
 */
export function SearchableList({
  items,
  label,
  placeholder,
  className = "",
  onAction,
}: SearchableListProps) {
  const [query, setQuery] = useState("");
  const fieldId = useId();

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (needle === "") return items;
    return items.filter((item) =>
      item.label.toLowerCase().includes(needle)
      || (item.detail ?? "").toLowerCase().includes(needle));
  }, [items, query]);

  const announcement = query.trim() === ""
    ? `${items.length} ${items.length === 1 ? "item" : "items"}`
    : `${filtered.length} of ${items.length} ${
      filtered.length === 1 ? "item matches" : "items match"
    } "${query.trim()}"`;

  return (
    <div className={["sapphirus-searchable-list", className].filter(Boolean).join(" ")}>
      <label className="sapphirus-searchable-list__label" htmlFor={fieldId}>
        {label}
      </label>
      <input
        autoComplete="off"
        className="sapphirus-searchable-list__field"
        id={fieldId}
        onChange={(event) => setQuery(event.target.value)}
        placeholder={placeholder}
        spellCheck={false}
        type="search"
        value={query}
      />
      {/*
        Polite, not assertive: the count updates on every keystroke, and an
        assertive region would interrupt the user's own typing.
      */}
      <p aria-live="polite" className="sapphirus-searchable-list__count" role="status">
        {announcement}
      </p>
      <ActionList
        items={filtered}
        label={`${label} results`}
        {...(onAction === undefined
          ? {}
          : { onAction: (key: Key) => onAction(String(key)) })}
      />
    </div>
  );
}
