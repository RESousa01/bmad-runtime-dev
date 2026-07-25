# Forge v2 frontend (renderer) deep-dive audit — 2026-07-24

**Scope:** adversarial review of `apps/desktop-ui/src` — security/XSS, type safety, React
lifecycle correctness, state/async logic, accessibility, and list reconciliation. Focus on
concrete defects, not style.

**Verdict:** the renderer has an unusually disciplined security and type posture. One concrete
correctness bug was found and fixed (F1). The remaining items are one minor accessibility gap and
the already-tracked W2 boundary.

## Strong posture (verified, not assumed)

- **No XSS surface.** Zero `dangerouslySetInnerHTML` / `innerHTML` / `insertAdjacentHTML` /
  `document.write` / `eval` in the renderer. All host-supplied strings render as React text and are
  auto-escaped. The "HTML-like projected strings render as inert text" test confirms this at the
  behavioral level.
- **No type-safety escapes.** Zero `any` / `as any` / `@ts-ignore` / `@ts-expect-error` /
  `eslint-disable` in non-test source. `tsc --noEmit` is the lint gate and passes.
- **No renderer-side ambient authority.** No `fetch` / `XMLHttpRequest` / `WebSocket` / `Worker` /
  `localStorage` / `sessionStorage` — consistent with the Rust-host trust boundary; the renderer
  reaches the host only through the typed client.
- **Lifecycle hygiene.** The projection poll (`setTimeout` loop), the `prefers-color-scheme`
  listener, and the preferences fetch all have correct effect cleanup (`clearTimeout`,
  `removeEventListener`, `cancelled` guards). No leaked timers/listeners were found.
- **StrictMode-safe.** The app mounts in `<StrictMode>`. The per-task store hook
  (`useCapabilitySessions`) mirrors state into a ref with an idempotent `storeRef.current = store`
  during render and mutates only in event handlers, so React 18's double-render does not corrupt it.
- **Error handling.** `getSafeHostMessage` surfaces a message only for typed host errors
  (`HostCommandError` / `HostCapabilityError`) and falls back to a generic string otherwise — no
  raw error internals leak to the UI.
- **Non-null assertions** are guarded (e.g. `events.at(-1)!` is under `events.length > 0`).
- **List keys.** Index-based keys appear only in static, non-reordered display lists (diff
  magnitude blocks, JSON lines, tokenised message text) — acceptable; no stateful reorderable list
  uses an index key.

## F1 — context auto-suggest can strand its in-flight flag (found and fixed)

**Where:** `apps/desktop-ui/src/App.tsx` — `suggestContext` / `browseAndAttach`.

**Defect:** `suggestContext` sets `contextSuggestionInFlightRef = true` and guards against a second
concurrent suggestion. Its `finally` reset was gated on
`generation === contextSuggestionGenerationRef.current`. But `browseAndAttach` (manual file attach)
**bumps the shared `contextSuggestionGenerationRef` without touching the in-flight ref**. So if a
user attaches files while an auto-suggest search is in flight, the suggestion's `finally` sees a
changed generation and skips the reset — stranding `contextSuggestionInFlightRef = true` and the
"auto-selecting…" spinner. Auto-select is then silently disabled until a `clearContext` / new task.

**Fix:** the in-flight ref and spinner are reset **unconditionally** in the `finally`. No other
`suggestContext` can be running when this one settles (the in-flight guard guarantees it), so the
generation gate was wrong for cleanup; the result-application guards in the `try` block are
unchanged. Pre-existing bug (independent of the task/session work). Renderer suite stays green
(411).

## I4 — fidelity badge note is not keyboard/SR-accessible (minor)

`BmadLibraryPanel`'s fidelity badge is `<span title={note}>{label}</span>`. The visible label
("Adapted") *is* announced, so the honesty signal reaches screen-reader users; only the detailed
`title` note is mouse-hover-only. If the accessibility gate is tightened, promote the note to an
accessible affordance (e.g. an `aria-describedby` on an accessible tooltip). Not an axe violation
today, and the badge is not in the axe-covered default states.

## W2 — result state for remaining lanes still global (tracked)

The capability **and changes** lanes are now per-task; `contextPreview` and `bmadHelpState` remain
app-global, so a slow completion in those two lanes can still surface under the wrong tab
(`sessionContext` derives one global bmadHelp `sessionId`). bmadHelp is the largest remaining lane
(non-nullable, projection-polling) and stays its own milestone — see
[`forge-v2-remediation-audit-2026-07-24.md`](forge-v2-remediation-audit-2026-07-24.md).

## Verification

| Check | Result |
|---|---|
| Renderer `typecheck` (lint) | Pass |
| Renderer `test` | Pass — 411 / 39 files |
| XSS / type-escape / ambient-authority sweeps | Clean |
