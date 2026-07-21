import { Check, ChevronDown, FolderOpen, Search } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { WorkspaceProjection } from "../lib/hostClient";

export interface WorkspaceSwitcherProps {
  readonly activeWorkspaceId: string | null;
  readonly canOpenFolder: boolean;
  readonly onActivate: (workspaceId: string) => void;
  readonly onOpenFolder: () => void;
  readonly onOpenManager: () => void;
  readonly statusLabel: string;
  readonly workspaces: readonly WorkspaceProjection[];
}

/**
 * Compact anchored dropdown for switching between granted workspaces:
 * search, checkmarked active row, and an "Open folder…" row that starts the
 * native governed folder grant. Labels are workspace display names only —
 * absolute paths never reach the renderer.
 */
export function WorkspaceSwitcher({
  activeWorkspaceId,
  canOpenFolder,
  onActivate,
  onOpenFolder,
  onOpenManager,
  statusLabel,
  workspaces,
}: WorkspaceSwitcherProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const rootRef = useRef<HTMLDivElement | null>(null);
  const searchRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (!open) {
      return undefined;
    }
    setQuery("");
    searchRef.current?.focus();
    function closeOnOutsidePointer(event: PointerEvent) {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("pointerdown", closeOnOutsidePointer);
    return () => document.removeEventListener("pointerdown", closeOnOutsidePointer);
  }, [open]);

  const active = workspaces.find(
    (workspace) => workspace.workspaceId === activeWorkspaceId,
  ) ?? workspaces[0] ?? null;
  const normalized = query.trim().toLocaleLowerCase("en-US");
  const visible = workspaces.filter((workspace) =>
    workspace.displayName.toLocaleLowerCase("en-US").includes(normalized),
  );

  return (
    <div
      className="workspace-switcher"
      onKeyDown={(event) => {
        if (event.key === "Escape" && open) {
          event.preventDefault();
          event.stopPropagation();
          setOpen(false);
        }
      }}
      ref={rootRef}
    >
      <button
        aria-expanded={open}
        aria-label={`Manage workspace ${active?.displayName ?? "none"}`}
        className="workspace-crumb"
        onClick={() => setOpen((current) => !current)}
        type="button"
      >
        <span className="workspace-crumb__name">
          {active?.displayName ?? "No workspace"}
        </span>
        <ChevronDown aria-hidden="true" size={13} strokeWidth={1.8} />
        <span aria-hidden="true" className="workspace-crumb__divider">/</span>
        <span>{statusLabel}</span>
      </button>
      {open ? (
        <section aria-label="Switch workspace" className="workspace-switcher__popover">
          <div className="workspace-switcher__search">
            <Search aria-hidden="true" size={13} />
            <input
              aria-label="Search workspaces"
              autoComplete="off"
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search workspaces"
              ref={searchRef}
              spellCheck={false}
              type="text"
              value={query}
            />
          </div>
          <ul aria-label="Workspaces" className="workspace-switcher__list">
            {visible.length === 0 ? (
              <li className="workspace-switcher__empty">No matching workspaces.</li>
            ) : (
              visible.map((workspace) => (
                <li key={workspace.workspaceId}>
                  <button
                    className="workspace-switcher__row"
                    onClick={() => {
                      setOpen(false);
                      if (workspace.workspaceId !== activeWorkspaceId) {
                        onActivate(workspace.workspaceId);
                      }
                    }}
                    type="button"
                  >
                    <span className="workspace-switcher__name">{workspace.displayName}</span>
                    {workspace.workspaceId === active?.workspaceId ? (
                      <Check aria-hidden="true" size={14} />
                    ) : null}
                  </button>
                </li>
              ))
            )}
          </ul>
          <button
            className="workspace-switcher__row workspace-switcher__manage"
            onClick={() => {
              setOpen(false);
              onOpenManager();
            }}
            type="button"
          >
            Manage workspaces…
          </button>
          {canOpenFolder ? (
            <button
              className="workspace-switcher__row workspace-switcher__open"
              onClick={() => {
                setOpen(false);
                onOpenFolder();
              }}
              type="button"
            >
              <FolderOpen aria-hidden="true" size={14} />
              Open folder…
            </button>
          ) : null}
        </section>
      ) : null}
    </div>
  );
}
