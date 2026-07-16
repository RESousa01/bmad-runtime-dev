# Sapphirus Desktop Shell Visual Refactor Design

**Date:** 2026-07-15  
**Status:** Approved for implementation planning  
**Scope:** `apps/desktop-ui` and presentation-only shared UI adjustments  
**Reference:** `Prototype design improvement.zip`, especially the supplied full desktop workbench view

## Decision

Refine the complete Sapphirus desktop shell through a component-aware visual refactor. The implementation will preserve the current product architecture, behavior, capability boundaries, accessibility semantics, and Sapphirus color palette while adopting the reference prototype's stronger hierarchy, compact spacing, panel treatment, navigation density, and interaction polish.

This is not a disconnected prototype and not a full shell rewrite. Existing React state, typed host communication, BMAD projections, modal behavior, and honest unavailable/preview states remain authoritative.

## Context

The current desktop renderer already has the correct high-level workbench structure:

- native title bar;
- global product navigation;
- session navigation;
- primary task workspace;
- inspector with Context, Changes, Logs, Evidence, and Method surfaces;
- responsive overlay behavior for supporting panels.

The supplied reference improves the visual execution of that same information architecture. Its useful qualities are compact density, clearer grouping, stable rails, quieter secondary surfaces, stronger focus on the task workspace, and more disciplined technical content. Its purple Nocturne palette is not part of the target.

The existing Sapphirus color tokens in `packages/ui/src/tokens.css` remain the sole color authority. In particular, the established canvas, chrome, surface, raised, hover, selected, border, text, accent, semantic-status, and diff colors must not be replaced or reinterpreted.

## Goals

1. Make the entire desktop shell feel coherent, professional, compact, and purpose-built for sustained technical work.
2. Improve visual hierarchy without removing information or hiding capability limitations.
3. Make global navigation, sessions, task content, the composer, and the inspector read as one coordinated system.
4. Apply the same component language across Agent, Workspaces, Explorer, Changes, Activity, Settings, Account, BMAD Method Library, dialogs, and responsive overlays.
5. Preserve functional behavior and accessibility while improving all visible states.
6. Produce a maintainable refactor that reduces style drift instead of layering a temporary theme over existing inconsistencies.

## Non-goals

- No new model, agent, editing, apply, checkpoint, undo, or workspace capability.
- No new route, IPC command, host-client behavior, persistence behavior, or backend dependency.
- No fabricated success, runnable state, model response, evidence, or workspace mutation.
- No replacement color palette, purple prototype palette, decorative gradient system, glass treatment, or glow-heavy styling.
- No redesign of BMAD semantics or authority boundaries.
- No dependency churn unless an existing dependency is demonstrably insufficient.
- No unrelated changes to the dirty backend, D2, D3, CI, installer, or infrastructure worktree.

## Hard visual constraints

### Color

- Preserve all existing Sapphirus light and dark theme color values.
- New styles must consume existing semantic custom properties rather than introduce direct palette literals.
- Accent color is reserved for selected state, focus, concise status emphasis, and decisive actions.
- Success, warning, danger, and diff colors keep their current meanings.
- Depth comes from surface selection, borders, spacing, and restrained shadows, not accent flooding.

### Typography

- Continue using Inter Variable for interface text.
- Continue using JetBrains Mono for source, paths, hashes, logs, and code.
- Use medium weight as the default hierarchy tool.
- Reserve stronger weights for page titles, decisive states, and primary action labels.
- Avoid oversized headings in the dense workbench.

### Shape and spacing

- Use a compact 4/8/12/16/24px spacing rhythm.
- Use restrained 6-8px radii for controls, rows, cards, and panels.
- Keep control hit areas accessible even when their visible treatment is compact.
- Prefer borders and whitespace to heavy shadows.

### Icons

- Continue using the existing Lucide icon set.
- Normalize icon size, stroke weight, baseline, and label spacing per component role.
- Do not introduce text glyphs, emoji, CSS drawings, handcrafted SVGs, or decorative placeholder art.

## Shell architecture

### Title bar

The title bar remains a compact native drag surface. Brand mark, brand name, drag region, and native window controls align to one baseline. The title bar is visually subordinate to the workbench. Window-control hit areas remain generous, with the existing distinct danger treatment for Close.

### Desktop grid

The wide layout remains a four-part workbench:

1. global navigation rail;
2. session rail;
3. flexible task workspace;
4. inspector.

The task workspace is the primary visual field. Rails use tighter density, softer separation, and quieter typography so they support rather than compete. Existing layout ranges remain the starting point; changes to widths must improve reference fidelity without starving the task workspace or regressing current breakpoints.

### Scroll ownership

- Title bar remains outside workbench scrolling.
- Global navigation and session rail remain stable within the viewport.
- Task header and composer remain stable while task content scrolls.
- Inspector tab strip and relevant footer actions remain stable while panel content scrolls.
- Overlays own their scrolling and preserve body/workbench stability.

## Surface design

### Global navigation

- Give every item consistent dimensions, icon placement, label alignment, and hit area.
- Express selected state with the existing accent, a restrained selected surface, and stronger text contrast.
- Keep hover, pressed, keyboard-focus, and disabled states distinct.
- Separate product navigation from Settings and Account with space and a subtle divider.
- Preserve tooltips and accessible labels when labels become constrained.

### Session rail

- Keep New session as the primary rail action.
- Align filter and panel controls to the same control grid.
- Improve session scanning with clearer title/meta separation, quieter timestamps, and consistent unread markers.
- Define polished default, hover, selected, focus, unread, empty, and disabled states.
- Keep the pinned workspace anchored to the footer and visually quieter than the active session list.

### Task header and content

- Tighten workspace identity, host state, and session-action grouping.
- Make the task title the clear start of the primary content hierarchy.
- Preserve preview, Method guidance, local-only, unbound, and recovery labels.
- Present user and assistant messages as a calm editorial stream, using avatars and metadata for authorship without excessive card chrome.
- Make stage progress, review summaries, impact metrics, notices, and follow-up messages feel related through consistent spacing and typography.
- Design new-session, submitting, retained-session, proposal-ready, discarded, and read-only-recovery states intentionally rather than allowing incidental layout changes.

### Composer

- Treat the composer as the stable command surface at the bottom of the task workspace.
- Give the text area clear input affordance and enough space for multiline intent.
- Keep attachments, mode, context review, host state, and send action aligned in a quieter secondary toolbar.
- Preserve all current availability explanations and submission guards.
- Cover empty, populated, focus, disabled, submitting, local-only, validation, and recovery states.
- Maintain keyboard and narrow-layout usability.

### Inspector

- Apply one tab treatment across Context, Changes, Logs, Evidence, and Method.
- Keep tabs stable while content scrolls.
- Improve the legibility of file accordions, metadata blocks, hashes, code previews, logs, evidence, and diffs.
- Use deliberate wrapping, truncation, or horizontal scrolling based on content type.
- Keep Discard, Revise, and Apply visually distinct even when unavailable.
- Preserve the explanatory footnote for disabled Apply behavior.
- Treat the BMAD Method Library and retained Help recommendation as first-class inspector surfaces, with the same spacing and hierarchy discipline as other panels.

### Other primary views

Workspaces, Explorer, Changes, and Activity must receive the same heading, toolbar, row, empty-state, notice, and content-width treatment. Their behavior remains unchanged. The refactor must remove visible shell drift between those views and Agent without forcing them into inappropriate task-message layouts.

### Settings, Account, dialogs, and overlays

- Standardize panel/dialog headers, titles, close controls, body spacing, and action placement.
- Preserve focus containment, Escape handling, inert background behavior, and accessible naming.
- Use one backdrop treatment derived from existing tokens.
- Avoid stacked overlays and ambiguous dismissal targets.

## Component language

### Buttons

- Filled primary: decisive actions such as New session and Review changes.
- Bordered secondary: supporting actions such as Review context, filters, and revisions.
- Quiet/icon: navigation utilities and non-primary controls.
- Destructive: reserved for actual destructive intent; disabled destructive actions remain legible.
- All variants cover hover, focus, pressed, disabled, and busy states.

### Tabs and selectable rows

- Use the existing accent and selected surface without large filled color blocks.
- Keep selected state visible independently of hover.
- Preserve keyboard focus and tab semantics.
- Align row metadata consistently and avoid accidental layout shift between states.

### Notices and badges

Use a consistent family for:

- preview/demo;
- local-only/unbound;
- read-only recovery;
- unavailable/disabled;
- success;
- warning;
- destructive/error.

The text remains the primary truth. Color and icons reinforce meaning but never carry it alone.

### Technical content

- Paths and short identifiers may truncate with accessible full-value exposure where appropriate.
- Hashes and code may wrap or scroll horizontally according to their existing interaction.
- Diffs preserve semantic added/removed colors and monospace alignment.
- Logs use consistent timestamp and event columns.
- Metadata lists use aligned label/value structure rather than ad hoc text blocks.

## Responsive behavior

### Wide desktop

Show the full four-panel workbench with independent scroll ownership and no overlapping controls.

### Medium desktop

Collapse the inspector to an accessible right-side overlay while keeping an explicit task-header control to open it.

### Compact desktop and tablet

Collapse the session rail to an accessible left-side overlay. The task workspace remains primary, and both supporting panels remain explicitly reachable.

### Narrow viewport

- Prioritize task content and the composer.
- Maintain usable panel controls and overlay dismissal.
- Keep primary actions visible without horizontal overflow.
- Allow tabs and technical content to scroll deliberately where wrapping would damage comprehension.
- Preserve comfortable pointer targets and keyboard access.

## Motion and accessibility

- Use brief, subtle transitions for hover, selected state, tab changes, and panel movement.
- Fully disable or simplify non-essential movement under reduced-motion preferences.
- Preserve visible focus rings and forced-colors behavior.
- Maintain semantic landmarks, headings, labels, roles, live regions, and modal focus containment.
- Do not reduce contrast or text legibility to achieve visual quietness.
- Verify keyboard-only navigation across the title controls, global rail, sessions, task actions, composer, inspector tabs, dialogs, and overlays.

## Engineering approach

1. Capture the current rendered shell and reference state before edits.
2. Establish presentation-only shell sizing and spacing variables in the desktop stylesheet. Do not add new color values.
3. Refine shared UI primitives only when the improvement benefits multiple shell surfaces and preserves their public API.
4. Adjust component markup where required for correct alignment, grouping, stable scroll ownership, or responsive behavior.
5. Keep application state and event handlers in their current owners. Presentation extractions must accept data and callbacks rather than duplicate business logic.
6. Refactor styles by shell layer and component responsibility, removing superseded declarations rather than accumulating overrides.
7. Implement wide layout first, then medium and narrow layouts, then state/accessibility polish.
8. Avoid changes outside `apps/desktop-ui` and clearly justified presentation primitives in `packages/ui`.

## Verification strategy

### Automated gates

- desktop UI unit/component tests;
- TypeScript typecheck;
- lint;
- production Vite build;
- existing architecture-boundary validation when included in the renderer gate.

### Interaction checks

- navigate every primary view;
- select and create sessions where currently enabled;
- open and close responsive session/inspector overlays;
- switch every inspector tab;
- exercise dialogs, settings, and account surfaces;
- exercise composer empty, populated, disabled, submitting, and retained-session states;
- review Method Library, Context, Changes, Logs, and Evidence states;
- verify focus restoration and Escape behavior.

### Visual QA

- Compare the reference and implementation at the same desktop viewport and interaction state.
- Evaluate composition, panel widths, alignment, spacing, typography, radii, borders, control sizing, and overflow while retaining the Sapphirus palette.
- Validate medium, compact, and narrow layouts separately.
- Fix all material hierarchy, layout, interaction, and accessibility mismatches before handoff.
- Record the final comparison and any non-blocking polish items in `design-qa.md`.

## Acceptance criteria

The refactor is accepted when:

1. The entire desktop shell follows one coherent compact visual system.
2. Existing Sapphirus color token values are unchanged.
3. All current product behavior and capability limitations remain accurate.
4. Global navigation, sessions, task workspace, composer, inspector, primary views, dialogs, and overlays share consistent component treatment.
5. Wide, medium, compact, and narrow layouts have no accidental overlap, clipping, or horizontal page overflow.
6. Keyboard, focus, reduced-motion, forced-colors, and modal accessibility behavior remains functional.
7. Automated renderer gates pass.
8. Visual QA passes with no unresolved material issues.
9. Unrelated repository work remains untouched.

## Risks and mitigations

### Large stylesheet regression surface

**Risk:** The existing renderer stylesheet is extensive, and additive overrides could create breakpoint conflicts.  
**Mitigation:** Refactor by component responsibility, remove superseded rules, verify each breakpoint, and prefer a small set of non-color layout variables over repeated values.

### Behavior regression from markup changes

**Risk:** Re-grouping controls can disturb accessible semantics, focus handling, or tests.  
**Mitigation:** Preserve semantic elements, roles, labels, event ownership, and focus utilities; keep markup changes presentation-driven and run focused tests after each shell slice.

### Prototype imitation over product truth

**Risk:** Literal copying could introduce colors, controls, or states that Sapphirus does not support.  
**Mitigation:** Treat the prototype as a hierarchy and composition reference only. Existing tokens and product capability projections remain authoritative.

### Dirty worktree interference

**Risk:** Existing backend and infrastructure changes could be accidentally mixed into the visual refactor.  
**Mitigation:** Limit edits and validation to the renderer and justified shared presentation files, inspect diffs by path, and do not modify unrelated work.

## Approved direction summary

- Entire desktop shell, not only Agent.
- Component-aware visual refactor, not CSS overlay or full rewrite.
- Existing colors retained exactly.
- Prototype hierarchy and polish adapted throughout.
- Product behavior, honesty, accessibility, and architecture preserved.
- Thorough responsive and visual QA required before completion.
