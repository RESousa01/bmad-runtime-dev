---
title: "First Vertical Slice User Stories"
aliases:
  - "61 - First Vertical Slice User Stories"
tags:
  - bmad-runtime
  - vault/delivery-plan
section: "Delivery Plan"
order: 61
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: user-story-pack
status: implementation-guide
---



# First Vertical Slice User Stories

## V6.17 story split

Web stories cover browser sign-in, cloud project import/upload/clone, cloud context, exact approval, remote isolated apply/test, SQL/Blob evidence, cloud checkpoint, and rollback. Desktop stories cover signed install, sign-in/entitlement, data-boundary explanation, native folder selection/revocation, local context preview/egress, exact local diff/command approval, checkpoint, journaled apply/local test, evidence, crash recovery, and rollback.

No story may accept “works in the sealed fake” as proof for the desktop or remote executor. The remote-job story is separate: preview exact upload → create `web_managed` record → receive non-applicable result → import local proposal → fresh local approval/checkpoint/apply.

## Epic VS-0: UX foundation and visual approval

### Story VS-0.1 — Approve the canonical workbench direction

As a user, I can recognize the product as a calm governed workbench and understand its navigation before backend integration begins.

Acceptance:

- The implementation follows the approved UX blueprint in file 43 and token/component baseline in files 26 and 66.
- Light and dark workbench concepts cover the `1280×720` and `1440×900` first-slice states before route implementation.
- The shell visibly distinguishes `Cloud workspace` from `Local folder` using text and icon, not color alone.
- The global rail, context rail, conversation, inspector, and composer have measured desktop, narrow, and mobile behavior.
- Storybook includes AppShell, navigation, RunCapsule, approval review, execution, partial failure, and evidence stories before API wiring.
- Visual review blocks generic dashboard cards, clipped actions, nested-card clutter, low-contrast metadata, all-monospace chrome, and decorative motion.

### Story VS-0.2 — Use one progressive RunCapsule

As a user, I can follow a run from understanding through evidence without decoding a card for every server event.

Acceptance:

- One `RunCapsule` groups `Understand → Plan → Review → Execute → Evidence`.
- The active stage has one primary action and completed stages collapse to inspectable summaries.
- Context, changes, command, logs, policy, and evidence open in the stable inspector.
- Unknown events remain available under Technical details and cannot crash or fragment the run.
- Completed runs collapse without hiding validation, evidence, or rollback availability.
- Reconnect/replay restores capsule stage, inspector selection, and scroll anchor without replaying entrance animation.

### Story VS-0.3 — Review a governed action clearly

As a reviewer, I can understand and decide a proposed side effect without reading raw hashes first.

Acceptance:

- `Review changes` opens a dedicated approval review with Outcome, Workspace impact, Execution, External access, Safety, Decision, and Technical details in that order.
- The sticky footer offers Reject, Request changes, and a boundary-specific action such as `Approve & run in cloud` or `Approve & run locally`.
- Candidate and policy hashes remain inspectable but do not replace the human impact summary.
- Stale, expired, disconnected, or denied state disables the decision and gives a visible reason.
- Keyboard-only users can inspect every review tab, decide, and return focus to the exact trigger.
- High-risk confirmation is explicit; routine approvals do not require retyping arbitrary phrases.

### Story VS-0.4 — Meet responsive, accessibility, and motion gates

As a user with different viewport, input, contrast, or motion needs, I can complete the same safe decision path.

Acceptance:

- The first slice works at `1280×720`, `1440×900`, narrow tablet, and mobile review widths without clipped decision controls or composer overlap.
- `200%` zoom, keyboard-only navigation, forced colors, light/dark themes, and reduced motion are release-gated.
- Panel resize handles are keyboard operable, visibly focused, and announce size.
- Streaming tokens/log lines are not individually announced; meaningful stage changes use a controlled polite live region.
- Reduced motion disables transform/layout animation and preserves immediate state feedback.
- Automated axe checks are combined with manual keyboard, screen-reader smoke, focus-order, and zoom review; automated checks alone do not imply compliance.

## Epic VS-1: Project and thread shell

### Story VS-1.1 — Create authenticated project shell

As a user, I can open the app with my workforce identity and see only projects I am allowed to access.

Acceptance:

- Entra/app auth or local dev auth shim maps user to project membership.
- Unauthorized project IDs return access denied without leaking object details.
- Audit event records auth mapping.

### Story VS-1.2 — Create chat thread

As a user, I can start a project thread and send a message.

Acceptance:

- Thread/message records persist.
- Run can be created from a message.
- UI stream starts with ordered run events.

## Epic VS-2: Workspace snapshot and context

### Story VS-2.1 — Upload sample repository

As a user, I can upload a sample zip and get an immutable workspace snapshot.

Acceptance:

- Snapshot archive stored through the Blob port (in-memory/fake implementation is valid for the local slice).
- File manifest includes hashes, sizes, ignored paths, and secret-scan status.
- File tree renders read-only.

### Story VS-2.2 — Build context pack

As the runtime, I can select relevant files/snippets for a run.

Acceptance:

- Context pack stores file hashes and selection reasons.
- Secrets and binaries excluded.
- Context invalidates after checkpoint changes affected files.

## Epic VS-3: Plan/proposal/policy

### Story VS-3.1 — Fake model creates typed plan

As a developer, I can use fake model output to produce a deterministic plan.

Acceptance:

- Output validates against schema.
- Orchestrator creates Plan record and event.
- UI renders plan card.

### Story VS-3.2 — Proposal and Airlock approval

As a user, I can inspect and approve a proposed patch.

Acceptance:

- Proposal has stable hash.
- Airlock evaluates path/preimage/schema policy.
- Approval card shows the full `ExecutionSpecCandidate`, policy result, and exact candidate hash.
- Expired approval cannot execute.

## Epic VS-4: Execute/validate/evidence

### Story VS-4.1 — Simulate approved patch through the trusted fake

As the runtime, I can apply one predefined approved patch to a sealed temporary fixture through a deterministic fake, without claiming process/container isolation.

Acceptance:

- Fake validates the exact candidate/spec hash and single-use audience.
- Fake has no process, shell, network, dependency restore, package import, or arbitrary command surface.
- Fake writes a simulated `WebWorkerResultManifest` and bounded logs through in-memory/fake ports.
- Fake has no lifecycle-store mutation interface and the UI labels the result as simulated/non-isolating.

### Story VS-4.2 — Import manifest and create evidence

As a reviewer, I can see what changed and how it was validated.

Acceptance:

- Runtime imports manifest idempotently.
- Checkpoint recorded.
- Evidence bundle links proposal, candidate, policy, approval, spec, simulated attempt/result, logs, changed files, checkpoint, and rollback point.
- First real isolated execution is a later fixed-template ACA Job gate and is not implied by this story pack.
