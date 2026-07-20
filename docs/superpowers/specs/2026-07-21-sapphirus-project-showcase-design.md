# Sapphirus Project Showcase Design

Date: 2026-07-21
Status: Approved narrative; implementation pending

## Purpose

Create one self-contained HTML page that explains Sapphirus to people who do not work in software development. The page must work equally well as a guided live walkthrough on a large screen and as a document that stakeholders can explore afterward.

The showcase is an explanation, not a slide deck, technical dashboard, sales claim, or readiness report. It should make the product understandable without requiring knowledge of Rust, React, Tauri, IPC, schemas, CI, BMAD internals, or Azure architecture.

## Audience and success criteria

The primary audience is non-technical organizational stakeholders. After reading or seeing the page, a person should be able to answer:

1. What problem does Sapphirus solve?
2. What is Sapphirus in everyday language?
3. What happens from a user's request to an approved result?
4. What can the product do today?
5. What safety and control mechanisms distinguish it from an unrestricted AI chat tool?
6. Which capabilities are still being completed?

Success means the page is readable from a presentation screen, understandable without a presenter, honest about current limitations, responsive on a phone, and usable without a build step or network connection.

## Deliverable

- Path: `docs/showcase/sapphirus-project-showcase.html`
- One HTML file with embedded CSS and JavaScript.
- No package installation, build command, analytics, remote fonts, CDN scripts, or network-loaded assets.
- It must open directly from disk and also work through a simple local web server.
- The page may use inline SVG for meaningful icons and diagrams.
- All visible text remains searchable, selectable, and accessible HTML rather than being baked into images.

## Narrative structure

### 1. Opening: controlled AI work, not uncontrolled automation

The first viewport introduces Sapphirus as a Windows workspace for AI-assisted project work where people remain in control. It should establish one central idea: assistance can be useful without silently giving an AI permission to alter files or organizational systems.

The opening contains a concise headline, two short supporting sentences, a primary link to the workflow explanation, a secondary link to current capabilities, and one visual product signal. It must not contain readiness percentages, technical architecture labels, decorative metrics, or inflated claims.

### 2. The problem

Explain the organizational tension in plain language:

- AI can accelerate understanding, planning, and drafting.
- Ordinary chat tools often sit outside the actual project context.
- Unrestricted agents can make changes that are difficult to inspect, authorize, or reverse.
- Organizations need assistance, accountability, and recovery together.

This section uses a short contrast between "fast but opaque" and "assisted and governed" without presenting competitors or fear-based security language.

### 3. What Sapphirus is

Describe Sapphirus as an internal Windows desktop workspace that brings structured AI assistance close to a real project while keeping project authority on the employee's computer.

Explain BMAD as a set of structured professional roles and workflows that help users move from questions to plans and reviewed outcomes. Avoid expanding the acronym unless a verified product-facing expansion is available. Describe personas in familiar terms such as analyst, architect, product manager, developer, designer, and technical writer.

Make clear that Sapphirus is more than a chat window: it understands an approved workspace, presents bounded context, records activity, and separates a suggestion from permission to act.

### 4. How it works

Use one prominent six-step horizontal workflow on wide screens and a vertical sequence on narrow screens:

1. **Choose a workspace** — the user selects the project Sapphirus may inspect.
2. **Ask for help** — the user chooses a structured role or describes the outcome they need.
3. **Prepare a proposal** — Sapphirus organizes context and produces a bounded recommendation or proposed change.
4. **Review the exact result** — the person sees what is proposed before any project change.
5. **Approve and apply** — only an explicit, current approval allows the desktop host to perform a governed change.
6. **Keep evidence and recover** — activity, checkpoints, undo, and interrupted-change recovery remain available.

The explanation should visually reinforce that steps 3 and 4 do not themselves change files.

### 5. What it is capable of

Present capabilities as a varied editorial composition rather than a repetitive card grid. Group them into five understandable themes:

- **Understand the project:** choose a local workspace, browse its structure, inspect bounded file content, search, and assemble reviewed context.
- **Provide structured guidance:** deterministic local Help and visible BMAD personas provide repeatable guidance even without a live model connection.
- **Prepare governed changes:** proposals can be reviewed against the exact intended result before approval.
- **Apply, undo, and recover:** approved UTF-8 file changes are checkpointed, journaled, verified, undoable through a new review, and recoverable after interruption.
- **Create organizational evidence:** activity history, explicit consent, bounded permissions, validation, and release evidence support accountability.

Use concrete examples that non-developers understand, such as explaining a project, preparing a product brief, reviewing a proposed document change, or restoring a previous version. Examples must not imply that every original BMAD workflow is already available.

### 6. Why the control model matters

Explain the safety model with four plain-language commitments:

- **You choose the project.** Access is limited to the workspace the user selected.
- **A proposal is not permission.** Suggestions cannot silently become file changes.
- **Approval is specific and temporary.** Permission is tied to the exact reviewed action and cannot be reused indefinitely.
- **The system fails safely.** If identity, consent, verification, or recovery evidence is missing, the action stops instead of guessing.

Support these commitments with a simple authority diagram: the visual interface requests; the trusted Windows host decides and records; workspace effects occur only after validation. Do not show code-level component names in the primary diagram. An optional plain-language detail may mention that the visible interface does not directly receive broad filesystem, token, database, or process control.

### 7. Available now and what comes next

Use two clearly differentiated columns or bands.

**Available and demonstrated now:**

- A real Windows desktop application and offline installer prototype.
- Approved local-workspace selection, reading, browsing, and search.
- Deterministic local BMAD Help and visible professional personas.
- Reviewed, governed file changes with history, checkpoints, undo, and interrupted-change recovery.
- Extensive source, cross-language, renderer, native, packaging, and CI verification.

**Still being completed or operationally qualified:**

- The production enterprise model connection; the current production transport intentionally stops as offline when it is not configured.
- Broader user-facing BMAD workflow coverage beyond the currently implemented paths.
- Authenticode signing, timestamping, protected release execution, independent clean-machine qualification, and update/rollback evidence for a release candidate.
- Organization-managed Azure deployment and end-to-end pilot evidence.
- Whole-product accessibility, security, operational, and internal-pilot closure on one frozen release.

Do not display speculative readiness percentages. Use honest status labels and short explanations instead.

### 8. Closing summary and glossary

Close with one memorable statement: Sapphirus is designed to make AI assistance useful inside real organizational work without removing human authority over what happens next.

Include a compact glossary for:

- **Workspace:** the project folder the user has deliberately selected.
- **BMAD:** structured professional roles and workflows used to guide project work.
- **Governed change:** a proposed change that is reviewed, explicitly approved, recorded, and recoverable.
- **Deterministic Help:** local, repeatable guidance that does not contact an AI provider.
- **Model-backed Help:** assistance produced through the planned enterprise AI service after identity, consent, and verification checks.

## Visual direction

Use a refined dark sapphire editorial aesthetic appropriate for an enterprise product explanation:

- Deep navy background with restrained sapphire and cyan accents.
- High-contrast white text and calm blue-gray supporting text.
- One strong focal composition per section rather than nested cards everywhere.
- Large display typography for live readability, with disciplined line lengths for shared reading.
- A subtle recurring path motif connects selection, proposal, review, approval, and recovery.
- Thin borders, soft illumination, and restrained depth; avoid neon cyberpunk styling.
- No decorative badges, fake statistics, dense dashboards, stock photography, or generic AI brain imagery.
- Use the Sapphirus name and a simple geometric inline mark derived from the existing application's visual character, without inventing an external brand claim.

The concept should feel trustworthy, clear, contemporary, and human-controlled rather than futuristic or autonomous.

## Interaction model

The page remains fully understandable without JavaScript. Progressive enhancements may include:

- A compact sticky section navigator on desktop and an accessible compact menu on mobile.
- Smooth anchor navigation with reduced-motion support.
- A subtle reading-progress indicator.
- Gentle reveal transitions that never hide content from keyboard users, print, or reduced-motion users.
- A "Start from the beginning" control near the closing section.

There are no carousels, auto-advancing slides, modal dialogs, simulated AI chat, forms, hidden capability copy, or interactions that are required to understand the page.

## Responsive and presentation behavior

- Primary design viewport: 1440 x 900, suitable for a laptop or presentation display.
- Verify at 1920 x 1080 for large-screen projection.
- Verify at approximately 390 x 844 for mobile sharing.
- Headings, diagrams, and status labels must remain readable at 100% browser zoom.
- Workflow and authority diagrams reflow rather than shrink into unreadable graphics.
- The live walkthrough should take roughly 8–12 minutes when the presenter follows the main narrative, while the full page remains useful for self-paced reading.
- Print styles should remove navigation chrome and preserve the narrative as a readable document.

## Accessibility and content rules

- Semantic landmarks and a logical heading hierarchy.
- Keyboard-accessible navigation with visible focus states.
- WCAG AA contrast as a minimum.
- No information conveyed by color alone.
- SVG diagrams have text alternatives or equivalent adjacent explanations.
- Respect `prefers-reduced-motion`.
- Avoid jargon where a plain-language phrase exists; define necessary project terms on first use.
- Never describe a planned capability as operational.
- Never imply that the browser version has native workspace authority.
- Never imply that the production enterprise model route is currently live.
- Never imply that the current installer is signed or release-qualified.

## Technical structure

The final HTML file will contain:

- A semantic `<header>`, `<main>`, eight narrative `<section>` elements, and `<footer>`.
- A small set of CSS custom properties for color, typography, spacing, radius, borders, and motion.
- Reusable class families for editorial layouts, capability rows, workflow steps, status bands, glossary entries, and accessible controls.
- Inline SVG symbols only where icons materially improve comprehension.
- A small inline script for navigation state, reading progress, and optional reveal enhancement.

If JavaScript fails, every section, link, diagram explanation, and status statement remains available.

## Verification acceptance criteria

Before handoff:

1. Confirm the file opens directly and through a local HTTP server.
2. Inspect the page in a real browser at 1440 x 900, 1920 x 1080, and 390 x 844.
3. Verify all anchor navigation, keyboard focus, reading progress, and reduced-motion behavior.
4. Verify no horizontal overflow, clipped text, accidental tiny copy, or unreadable diagram labels.
5. Compare the browser render against the approved visual concept for copy, hierarchy, typography, palette, spacing, diagram structure, status distinction, and responsive behavior.
6. Confirm all above-the-fold copy matches the approved concept.
7. Search for and remove placeholders, fake metrics, technical leakage, unsupported claims, external requests, and debug output.
8. Confirm the final page distinguishes deterministic local Help from planned model-backed Help, and prototype installer evidence from signed release readiness.

## Explicit non-goals

- Replacing the existing desktop UI.
- Adding a new product runtime or build pipeline.
- Reproducing the readiness scorecard.
- Providing developer installation instructions.
- Marketing unsupported future capabilities as complete.
- Embedding third-party source-vault images or code.
- Creating a slide deck, PDF, video, or hosted public website.
