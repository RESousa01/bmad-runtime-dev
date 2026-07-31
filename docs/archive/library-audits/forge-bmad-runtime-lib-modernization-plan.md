# Forge Team Knowledge System Modernization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use an isolated worktree and execute this plan task-by-task with review checkpoints. Do not run multiple agents against shared documentation or metadata files. Every completed task must include fresh verification output.

**Goal:** Replace the stale, mixed-purpose `bmad-runtime-lib` vault with a maintainable Forge team knowledge system: one current authority under `/docs`, a frozen legacy archive, reproducible source-analysis references, no-Entra current guidance, clear ownership, and automated validation.

**Architecture:** `/docs` becomes the canonical human and agent entrypoint. Current facts, accepted decisions, runbooks, active plans, reviews, source analysis, and archives are explicitly separated. Third-party repositories are represented by a machine-readable source lock and stored outside the normal product source tree. First-party knowledge tooling moves to `/tools/knowledge` and is reviewed and tested like other Forge code. `bmad-runtime-lib` becomes a temporary compatibility pointer and is removed after the cutover window.

**Tech stack:** Markdown, JSON, Node.js `24.18.0`, pnpm `11.15.1`, Node built-in test runner, GitHub Actions, existing repository tooling. Do not add a documentation framework, site generator, database, or third-party parser in this migration.

## Global constraints

- The human-facing product name is **Forge**.
- The active architecture must work **without Entra**. Entra content may remain only as explicitly historical or rejected/alternative reference material.
- Do not blindly rename stable `sapphirus.*` schema IDs, package names, executable names, environment variables, or persisted identifiers. Their migration requires a separate compatibility decision and tests.
- Source code, tests, generated contracts, and fresh verification output remain stronger authority than documentation.
- Product build, runtime, installer, and release qualification must remain independent of the documentation/reference system.
- Preserve the user-reported 14-file stabilization work and re-baseline from the actual live worktree, not only the uploaded ZIP.
- Do not update all 105 legacy notes in place. Extract unique current value, then archive the original notes.
- Do not execute imported third-party scripts or build imported repositories.
- Do not introduce unsupported completion percentages. Use named denominators, task counts, capability states, and explicit evidence.
- Use official primary sources for external facts. Every external fact must have a review date and expiry/review-due date.
- Standard Markdown links must work in GitHub and VS Code. Obsidian support is optional and must not be required to navigate the documentation.
- No task may claim product, Azure, installer, Foundry, or pilot readiness based solely on documentation checks.
- No branch, push, or release action is implied by this plan. The executing team follows its normal repository authorization policy.

---

## Context inspected

- Latest uploaded Forge repository snapshot and integrated `bmad-runtime-lib`.
- Historical `bmad-runtime-lib(1).zip` baseline.
- 105 root notes, 45,710 lines, current knowledge layer, evidence registries, validators, manifests, and tests.
- Current `/docs` tree with 129 files, 10 ADRs, BMAD-100 status, Foundry operations, implementation packets, reviews, quality, security, and plans.
- Root README, package scripts, CI, CodeQL, CodeRabbit, `.codex/config.toml`, and provenance records.
- Imported BMAD Method, BMAD Builder, OpenClaw, Hermes, and Odysseus trees.
- Latest comprehensive source audit and the user-reported post-audit stabilization results.
- Deep-review evidence and the 105-note migration matrix generated with this plan.

---

## Requirements inventory

### Critical requirements

1. One unambiguous current documentation entrypoint.
2. One documented authority precedence model.
3. Forge/no-Entra current guidance.
4. Current capability, BMAD, Builder, provider, release, and known-gap status derived from current source evidence.
5. Legacy notes preserved without remaining visible as current instructions.
6. Reproducible source provenance for retained source-analysis claims.
7. First-party validators and current docs included in normal review and CI.
8. Clear team ownership, contribution rules, update triggers, and review cadence.
9. A takeover engineer must be able to understand the product, run it, verify it, and select a first task without reading the legacy archive.
10. No regression to product build independence.

### Non-functional requirements

- **Maintainability:** current docs should be concise, modular, and generated where facts already exist in manifests.
- **Reliability:** validation must distinguish stale facts, broken links, unavailable Git context, and archive-integrity failure.
- **Security:** vendor source remains non-executable; first-party tools are not hidden by broad path exclusions.
- **Legal/provenance:** every retained third-party source has URL, immutable identity, archive hash, license, purpose, and inclusion decision.
- **Reviewability:** generated files are deterministic; current facts have owners and verification dates.
- **Portability:** normal Markdown works without Obsidian.
- **Performance:** the fast docs gate should complete in under 15 seconds on the pinned Node toolchain, excluding external-network checks.
- **Accessibility:** headings, link text, tables, and terminology are understandable without internal project history.

### Good-to-haves

- Optional Obsidian graph/backlink support generated from the same Markdown.
- A small generated HTML team handbook can be added later, but it is not part of this migration.
- A searchable internal documentation site can be evaluated only after the file-based authority model is stable.

### Current-state constraints

- The uploaded snapshot has no `.git` directory.
- The actual developer worktree contains post-snapshot changes reported by the user.
- Existing `vault:check` proves frozen receipt integrity, not current semantic correctness.
- Imported source trees are partial or mislocated relative to several source-analysis notes.
- `/docs` already contains newer and more authoritative decisions than the living knowledge layer.
- The team and GitHub owner handles are not present in the archive. Logical owner roles must be mapped to real repository handles before CODEOWNERS is merged.

### Assumptions

- `/docs` remains inside the Forge repository and is available to every developer.
- The organization can store source-reference archives as internal build/release artifacts or in an approved internal object store.
- Forge maintainers prefer a small number of strong current documents over another 100-file current library.
- Obsidian remains optional; repository-native review is mandatory.

---

## Target information architecture

```text
docs/
├── README.md
├── current/
│   ├── product-state.md
│   ├── product-scope-and-modes.md
│   ├── capability-status.md
│   ├── architecture-and-authority.md
│   ├── authentication-and-model-access.md
│   ├── security-and-trust.md
│   ├── toolchain-and-verification.md
│   ├── release-readiness.md
│   └── roadmap-and-known-gaps.md
├── development/
│   ├── onboarding.md
│   ├── local-setup.md
│   ├── contribution-guide.md
│   ├── documentation-policy.md
│   ├── agent-development-workflow.md
│   └── testing-and-verification.md
├── architecture/
│   ├── system-map.md
│   ├── runtime-boundaries.md
│   ├── workspace-and-execution.md
│   ├── state-evidence-and-recovery.md
│   ├── model-connectivity.md
│   ├── bmad-method-and-builder.md
│   └── adr-index.md
├── adr/                       # existing ADRs retained
├── operations/                # existing runbooks retained and reconciled
├── bmad/                      # existing generated/status/skill docs retained
├── implementation-packets/    # active and completed status made explicit
├── plans/
│   ├── active/
│   └── completed/
├── reviews/
│   ├── active-findings.md
│   └── archive/
├── reference/
│   ├── README.md
│   ├── source-analysis/
│   │   ├── bmad-method-builder.md
│   │   ├── opencode.md
│   │   ├── hermes.md
│   │   ├── odysseus.md
│   │   └── historical-openclaw.md
│   └── manifests/
├── archive/
│   ├── README.md
│   ├── legacy-v6/
│   ├── preserved/
│   ├── handoffs/
│   └── library-audits/
└── _meta/
    ├── knowledge-manifest.json
    ├── claims.json
    ├── sources.json
    ├── pins.generated.json
    ├── note-index.generated.json
    └── schemas/

reference-sources.lock.json

tools/knowledge/
├── check-docs.mjs
├── check-authority.mjs
├── generate-index.mjs
├── generate-pins.mjs
├── verify-claims.mjs
├── verify-reference-sources.mjs
├── verify-archive-manifest.mjs
├── migrate-v1.mjs
└── *.test.mjs

bmad-runtime-lib/
├── README.md                  # temporary compatibility pointer
└── archive-manifest.json      # proves the original archive identity
```

The existing `/docs` subtrees should be retained where they already have clear purpose. This plan adds authority and navigation before moving files. It does not require a big-bang directory rename.

---

## Authority model

Every maintained document must have one `doc_type`:

```text
current | adr | runbook | plan | review | reference | archive
```

Authority precedence:

```text
source/tests/gates
  > accepted ADR
  > docs/current
  > runbook
  > active plan
  > dated review
  > source analysis
  > archive
```

Required metadata for maintained documents:

```yaml
---
title: "Forge product state"
doc_type: current
status: current
owner_role: forge-maintainers
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/currentHead"
source_paths:
  - README.md
  - package.json
supersedes: []
---
```

Rules:

- Exactly one of `source_commit` or `source_commit_from` is required for `current`, `review`, and `runbook` documents that make implementation claims. `source_commit_from` must resolve to an exact commit value through a JSON Pointer in a tracked baseline file.
- `plan` documents use `baseline_commit`, `status`, `owner_role`, and `exit_gate`; they never use `status: current`.
- `archive` documents do not require current links or current terminology, but must carry an archive banner and original hash.
- `reference` documents describe external or historical source evidence and cannot claim Forge behavior without linking to current repository evidence.
- The v2 validator must allow the note set to evolve from the manifest; it must not hardcode “exactly eight notes.”

---

## Knowledge evidence model v2

Use schema name:

```text
forge.knowledge.v2
```

A current claim record must contain these fields:

| Field | Required value |
|---|---|
| `id` | Stable Forge claim ID such as `FORGE-CAP-PRD-001` |
| `subject` | Stable subject such as `capability.bmad-prd` |
| `class` | `implemented`, `decision`, `external_fact`, `planned`, `blocked`, or `unknown` |
| `statement` | One falsifiable statement |
| `status` | `current` or `superseded` |
| `ownerRole` | A role present in the knowledge manifest |
| `verifiedAt` | ISO date from the verification run |
| `reviewDue` | ISO date enforced by policy |
| `evidence` | At least one repository/ADR/official-source record and, for implementation claims, a repository path or deterministic gate |
| `supersedes` | Claim IDs; empty array when none |

For the accepted PRD capability, repository evidence must name the actual commit captured in Task 0, `docs/bmad/BMAD-100-IMPLEMENTATION-STATUS-2026-07-30.md`, `packages/bmad-foundation/capability-catalog.json`, and the gate `node packages/bmad-foundation/scripts/generate-catalog.mjs --check` with expected exit `0`.

Required improvements over v1:

- Per-claim commits instead of one global anchor.
- Explicit claim class: `implemented`, `decision`, `external_fact`, `planned`, `blocked`, or `unknown`.
- Gate evidence as a first-class source.
- Review due dates.
- Owner roles.
- Supersession without cycles.
- Archive validation that works in a non-Git extraction without pretending repository commits were verified.
- Repository facts may use one strong source plus a deterministic gate; external platform claims require official primary evidence.
- Pins are generated from their owner manifests and are not manually duplicated.

---

## Iteration layering

### Iteration 1 — Takeover-ready authority reset

**Outcome:** A new team has one accurate entrypoint, one current product state, one architecture map, one no-Entra model-access statement, one capability status, and one onboarding path.

**Included:** Tasks 0–3 and the fast structural validator.

**Deferred:** Moving the 20,000 vendor files and fully consolidating source-analysis notes.

**Target effort:** 2–3 engineer-days.

### Iteration 2 — Archive and provenance separation

**Outcome:** Legacy content is preserved but no longer competes with current authority. Third-party source identity and licensing are machine-readable.

**Included:** Tasks 4–7.

**Deferred:** Scheduled external revalidation and optional documentation publishing.

**Target effort:** 4–7 engineer-days.

### Iteration 3 — Team governance and automated maintenance

**Outcome:** Documentation freshness is owned, reviewed, and enforced continuously. A new developer completes the onboarding walkthrough successfully.

**Included:** Tasks 8–11.

**Target effort:** 3–5 engineer-days.

**Total expected effort:** approximately 10–15 engineer-days for one engineer, or 6–9 working days with two coordinated engineers. Source-storage and legal-review availability can extend the source-externalization task.

---

## Vertical implementation slices

### Task 0 — Freeze the actual live baseline before migration

**Value:** Prevents the modernization from losing the user’s latest repairs or anchoring current docs to the older uploaded ZIP.

**Files:**

- Create: `docs/provenance/knowledge-migration-baseline.json`
- Create: `docs/reviews/forge-knowledge-baseline-2026-07-31.md`
- Do not modify `bmad-runtime-lib` in this task.

**Work:**

1. Enter an isolated worktree from the actual current repository.
2. Capture:
   - current branch;
   - HEAD commit;
   - `git status --short`;
   - the 14 user-reported stabilization changes if still uncommitted;
   - current package/toolchain pins;
   - current BMAD capability catalog hash;
   - current BMAD-100 run-state and status documents;
   - current `bmad-runtime-lib` file count, size, root-note manifest hash, living manifest hash, and source-snapshot counts.
3. Run the existing narrow checks without modifying files:

```powershell
node packages/bmad-foundation/scripts/generate-catalog.mjs --check
node --test packages/bmad-foundation/tests/bmad-capability-catalog.test.mjs
node tools/check-workflows.mjs
node tools/check-boundaries.mjs
node tools/verify-reference-vault.mjs
py -3 bmad-runtime-lib/_source_review/validate_library.py
```

4. Record every command, exact tool version, exit code, and limitation. Do not normalize failures into one “red” status.
5. Hash the historical July 16 archive and the integrated library root-note manifest.

**Acceptance:**

- Baseline names the real commit and dirty state.
- Post-audit stabilization work is accounted for.
- Product gates, docs gates, archive integrity, and environment limitations are separate fields.
- No repository content outside the two new evidence files changed.

**Review focus:** Correct commit and worktree identity; no accidental claim that the uploaded ZIP is the current working tree.

**Rollback:** Delete the two new evidence files.

---

### Task 1 — Lock documentation authority, Forge naming, and no-Entra direction

**Value:** Removes the three largest sources of future-team confusion before content migration begins.

**Files:**

- Create: `docs/adr/ADR-0011-documentation-authority-and-reference-boundary.md`
- Create: `docs/adr/ADR-0012-no-entra-authentication-and-model-access.md`
- Create: `docs/adr/ADR-0013-forge-name-and-legacy-identifier-compatibility.md`
- Modify: `README.md`
- Modify: `.codex/config.toml`
- Create: `docs/architecture/adr-index.md`

**Decision content:**

#### ADR-0011

- `/docs` is current team authority.
- `bmad-runtime-lib` is reference/archive only.
- Precedence is source/tests → ADR → current docs → runbook → plan → review → reference → archive.
- Plans and audits are never implementation proof.
- Current docs require owner, source commit, verification date, and review due date.

#### ADR-0012

- Forge current development and intended deployment must not require Entra.
- Prototype direct providers use host-held API keys and exact-context approval.
- Production no-Entra trust is a separate governed path; current gaps are documented rather than hidden behind fictitious Entra-shaped values.
- Entra/WAM content is historical or deferred, not a current prerequisite.
- The ADR must not claim the no-Entra production path is complete.

#### ADR-0013

- Human-facing product name is Forge.
- Stable `sapphirus.*`, `@sapphirus/*`, `SAPPHIRUS_*`, solution, executable, or persisted identifiers remain legacy compatibility IDs until separate migration proof exists.
- Active docs must label these as legacy internal identifiers, not as the product name.
- New identifiers use Forge naming unless constrained by compatibility.

**Acceptance:**

```powershell
rg -n "Develop Sapphirus|Sapphirus Desktop" README.md .codex docs/adr docs/architecture
```

Expected: no product-facing stale names; only explicit legacy-identifier discussion is allowed.

```powershell
rg -n "Entra" docs/adr/ADR-0012-no-entra-authentication-and-model-access.md
```

Expected: Entra appears only to state that it is not required/current and to delimit historical alternatives.

**Review focus:** The ADRs describe current decisions without pretending incomplete provider or production authentication work is finished.

**Rollback:** Revert the three ADRs and the two small entrypoint edits as one slice.

---

### Task 2 — Build the concise current Forge documentation layer

**Value:** Gives the takeover team accurate answers without reading legacy notes or dated audits.

**Files:**

- Create: `docs/README.md`
- Create: `docs/current/product-state.md`
- Create: `docs/current/product-scope-and-modes.md`
- Create: `docs/current/capability-status.md`
- Create: `docs/current/architecture-and-authority.md`
- Create: `docs/current/authentication-and-model-access.md`
- Create: `docs/current/security-and-trust.md`
- Create: `docs/current/toolchain-and-verification.md`
- Create: `docs/current/release-readiness.md`
- Create: `docs/current/roadmap-and-known-gaps.md`

**Required content:**

#### `docs/README.md`

Provide five routes only:

1. Understand Forge now.
2. Set up and run locally.
3. Understand architecture and trust boundaries.
4. Choose current work from the roadmap.
5. Find historical/reference material.

Do not reproduce detailed architecture on the index page.

#### `product-state.md`

- Current local desktop behavior.
- What is proven in source.
- What the current PoC can demonstrate.
- What remains disconnected or disabled.
- Exact source commit and verification date.

#### `product-scope-and-modes.md`

Separate:

- deterministic local mode;
- direct-provider development PoC;
- intended no-Entra managed production mode;
- unsupported/deferred modes.

#### `capability-status.md`

- Derive BMAD capability state from the current catalog/source accounting.
- List accepted, partial, planned, and blocked capabilities.
- Use `34/156`, partial-task identities, and named denominators only when freshly reconciled.
- State that the latest reported stabilization still had three BMAD catalog blockers unless the live worktree has since fixed them.

#### `authentication-and-model-access.md`

- Explain API-key prototype access.
- Explain current Foundry/Azure OpenAI endpoint distinctions as implemented by source.
- Explain that no-Entra production composition remains incomplete.
- Explicitly reject fake Entra configuration as a substitute for no-Entra design.

#### `release-readiness.md`

Keep separate rows for:

- source checks;
- desktop build;
- installer build;
- clean-machine lifecycle;
- signing/timestamping;
- Azure/support-plane qualification;
- pilot evidence.

**Content rules:**

- No inherited percentage estimates.
- No current statement copied from a dated audit without rechecking the live worktree.
- Every implementation claim links to repository evidence or a gate.
- Every page is under approximately 250 lines unless a table genuinely requires more.
- Product language is Forge.
- Cortex does not appear in current docs.
- Entra does not appear as a current requirement.

**Acceptance:**

A developer unfamiliar with the project can answer these questions using only `docs/README.md` and `docs/current`:

- What is Forge?
- What works now?
- What does not work now?
- Which AI/model modes exist?
- What is the current BMAD state?
- What is the current trust boundary?
- What gates prove source, installer, and production readiness?
- What should be built next?

**Review focus:** Source-backed truth and clear separation between implemented, planned, blocked, and historical.

**Rollback:** Remove `docs/current` and restore the previous README link until Task 3 is ready.

---

### Task 3 — Create the takeover onboarding and contribution path

**Value:** Converts documentation into a working team-development system rather than a passive archive.

**Files:**

- Create: `CONTRIBUTING.md`
- Create: `AGENTS.md`
- Create: `docs/development/onboarding.md`
- Create: `docs/development/local-setup.md`
- Create: `docs/development/contribution-guide.md`
- Create: `docs/development/documentation-policy.md`
- Create: `docs/development/agent-development-workflow.md`
- Create: `docs/development/testing-and-verification.md`
- Create: `.github/pull_request_template.md`
- Create: `.github/CODEOWNERS` after logical roles are mapped to actual repository handles

**Required onboarding flow:**

#### 15-minute orientation

1. Read `docs/README.md`.
2. Read `docs/current/product-state.md`.
3. Read `docs/current/roadmap-and-known-gaps.md`.
4. Review the system map and trust boundary.
5. Locate the current BMAD status and latest active plan.

#### 60-minute developer setup

Use exact commands from `package.json`, `.node-version`, `rust-toolchain.toml`, and `global.json`. Explain Windows/Tauri requirements, which checks are source-only, and which require Rust/.NET/Azure/signing infrastructure.

#### First contribution path

- Select one bounded task.
- Identify authority and affected docs.
- Run narrow tests first.
- Keep worktree state explicit.
- Update current docs only when behavior/decision changes.
- Archive completed plans rather than marking everything “current.”

#### AI-agent working agreement

`AGENTS.md` and `agent-development-workflow.md` must state:

- inspect branch/dirty state first;
- source/tests override docs;
- no broad generated-content rewrites;
- no concurrent agents on shared authority files;
- preserve unrelated user changes;
- use named verification gates;
- report exact limitations;
- update durable context only after evidence exists;
- never treat a plan or agent report as completion proof.

**CODEOWNERS logical roles:**

- Forge maintainers: `/docs/current`, root README, ADR index.
- Architecture owner: `/docs/adr`, `/docs/architecture`.
- BMAD owner: `/docs/bmad`, BMAD source analysis.
- Security/operations owner: `/docs/security`, `/docs/operations`, reference-source license decisions.
- DevEx owner: `/docs/development`, `/tools/knowledge`, docs workflow.

Before merge, each logical role must resolve to an actual repository user or team. The gate must reject unresolved example handles.

**Acceptance:**

- A teammate follows the setup guide without relying on personal paths, BigBrain, or old handoffs.
- PR template asks whether current docs, ADRs, runbooks, source lock, or archive status changed.
- CODEOWNERS contains only resolvable repository handles.
- `AGENTS.md` and `.codex/config.toml` agree on Forge naming, authority, no-Entra direction, and verification discipline.

**Review focus:** The guide is executable, not aspirational; commands exist in the current repository.

---

### Task 4 — Implement `forge.knowledge.v2` metadata and deterministic generation

**Value:** Retains the useful evidence model while removing stale hardcoded constraints.

**Files:**

- Create: `docs/_meta/schemas/knowledge-manifest.schema.json`
- Create: `docs/_meta/schemas/claims.schema.json`
- Create: `docs/_meta/schemas/sources.schema.json`
- Create: `docs/_meta/knowledge-manifest.json`
- Create: `docs/_meta/claims.json`
- Create: `docs/_meta/sources.json`
- Generate: `docs/_meta/pins.generated.json`
- Generate: `docs/_meta/note-index.generated.json`
- Create: `tools/knowledge/migrate-v1.mjs`
- Create: `tools/knowledge/generate-pins.mjs`
- Create: `tools/knowledge/generate-index.mjs`
- Create tests for every generator.

**Migration behavior:**

- Read v1 claims/sources/note catalog as input.
- Do not automatically promote v1 claims to current.
- Mark each imported claim `needs_reverification` unless fresh live-worktree evidence is recorded.
- Convert Sapphirus schema names to `forge.knowledge.v2` in the new metadata only.
- Preserve the original v1 files and hashes in the archive.
- Current note roles come from `knowledge-manifest.json`; no fixed count.
- Generate pins directly from owner files. The generated record contains observed values and source paths; it does not store a separately maintained expected value.

**Pin generation sources:**

```text
.node-version or .nvmrc
package.json packageManager and engines
apps/desktop-ui/package.json
Cargo.toml and rust-toolchain.toml
global.json
crates/desktop-app/tauri.conf.json
```

**Acceptance:**

- Changing pnpm or Vite in its owner manifest changes generated pins deterministically.
- Adding a current note requires only manifest data, not validator source changes.
- Current claims may use different source commits.
- Supersession cycles fail.
- A plan cannot be registered as current implementation evidence.
- An archive extraction without `.git` reports repository evidence as `not_verified_in_archive`, not `missing_source`.

**Verification:**

```powershell
node --test tools/knowledge/generate-pins.test.mjs tools/knowledge/generate-index.test.mjs tools/knowledge/migrate-v1.test.mjs
node tools/knowledge/generate-pins.mjs --check
node tools/knowledge/generate-index.mjs --check
```

**Review focus:** Determinism, migration safety, and no silent claim promotion.

---

### Task 5 — Build first-party documentation validation and move it out of the excluded vault

**Value:** Makes current documentation and its validators normal reviewed repository code.

**Files:**

- Create: `tools/knowledge/check-docs.mjs`
- Create: `tools/knowledge/check-authority.mjs`
- Create: `tools/knowledge/verify-claims.mjs`
- Create: `tools/knowledge/verify-reference-sources.mjs`
- Create: `tools/knowledge/verify-archive-manifest.mjs`
- Create corresponding `*.test.mjs` files.
- Modify: `package.json`
- Retain v1 Python tooling read-only until cutover; remove it only in Task 10.

**Package scripts:**

```json
{
  "docs:generate": "node tools/knowledge/generate-pins.mjs && node tools/knowledge/generate-index.mjs",
  "docs:check:generated": "node tools/knowledge/generate-pins.mjs --check && node tools/knowledge/generate-index.mjs --check",
  "docs:check": "node tools/knowledge/check-docs.mjs && node tools/knowledge/check-authority.mjs && node tools/knowledge/verify-claims.mjs --offline && node tools/knowledge/verify-reference-sources.mjs --offline && node tools/knowledge/verify-archive-manifest.mjs",
  "docs:test": "node --test tools/knowledge/check-docs.test.mjs tools/knowledge/check-authority.test.mjs tools/knowledge/generate-index.test.mjs tools/knowledge/generate-pins.test.mjs tools/knowledge/verify-claims.test.mjs tools/knowledge/verify-reference-sources.test.mjs tools/knowledge/verify-archive-manifest.test.mjs tools/knowledge/migrate-v1.test.mjs",
  "docs:verify": "pnpm docs:test && pnpm docs:check:generated && pnpm docs:check"
}
```

**`check-docs.mjs` must verify:**

- standard Markdown links across all maintained docs;
- wiki links when present;
- one H1 per maintained document;
- required metadata by `doc_type`;
- valid status values;
- no merge markers;
- no empty maintained files;
- no duplicate current titles;
- no broken source-path metadata;
- no generated-file drift;
- archive files are excluded from current-link and current-language checks but retain hashes.

**`check-authority.mjs` must verify:**

- exactly one team entrypoint;
- exactly one current product-state document;
- no `status: current` outside `docs/current`;
- plans and reviews do not claim current authority;
- no active document points to a legacy note as a stronger authority;
- Sapphirus appears in active docs only inside the legacy-identifier allowlist;
- Entra appears in active docs only in the no-Entra decision or explicitly historical/rejected sections;
- Cortex does not appear in active current architecture.

**`verify-claims.mjs` must verify:**

- evidence paths exist;
- source commits are valid when running in a Git checkout;
- expected gate commands are allowlisted and may be checked by identity without being executed automatically;
- claims past `reviewDue` fail current validation;
- unknown or blocked states remain explicit;
- repository facts do not rely only on prose plans.

**Test cases:**

- current note with stale Sapphirus product name fails;
- current note with Entra as required auth fails;
- allowed legacy identifier table passes;
- archived original Sapphirus note passes;
- broken link in `docs/README.md` fails;
- missing Git metadata produces a precise archive limitation;
- stale pin projection fails;
- one plan claiming “implemented” without repository/gate evidence fails;
- source-analysis path to missing snapshot fails;
- source lock with an invalid SHA-256 fails.

**Performance acceptance:**

```powershell
Measure-Command { pnpm docs:verify }
```

Target: under 15 seconds on the pinned Node toolchain with external network disabled.

**Review focus:** The checks fail for real authority drift without turning historical archives into false failures.

---

### Task 6 — Archive and consolidate the 105 root notes using the migration matrix

**Value:** Preserves the research investment while removing 45,000 lines from the normal current reading path.

**Input artifact:** `forge-library-note-migration-matrix.csv`.

**Disposition totals:**

| Action | Notes |
|---|---:|
| Archive directly | 83 |
| Extract useful content, then archive | 9 |
| Merge into a retained source-analysis note | 3 |
| Archive verbatim | 2 |
| Refresh and retain | 2 |
| Rewrite | 2 |
| Replace entrypoint, then archive | 2 |
| Merge, then archive | 1 |
| Replace with machine-readable authority | 1 |

**Files:**

- Create: `docs/archive/README.md`
- Create: `docs/archive/legacy-v6/`
- Create: `docs/archive/preserved/`
- Create: `docs/archive/library-audits/`
- Create: `docs/reference/source-analysis/`
- Create: `docs/reference/manifests/legacy-v6-manifest.json`
- Modify: `bmad-runtime-lib/README.md`
- Create: `bmad-runtime-lib/archive-manifest.json`

**Migration rules:**

1. Do not rewrite archived note bodies to modern terminology.
2. Add an archive banner through a sidecar index or wrapper, not by mutating the two byte-preserved notes.
3. Preserve original relative path, SHA-256, authority class, original title, and supersession target.
4. Extract only unique, current-relevant content into maintained docs.
5. Remove repeated generic templates rather than copying them into the new system.
6. Replace hand-maintained navigation with generated indexes.
7. Keep legacy links working through an archive path map for one migration window.
8. Delete empty `start/getting-started.md`, `start/onboarding.md`, and `tools/browser.md` after their replacements are live.
9. Remove personal `.obsidian/workspace.json` and stale bookmarks. Retain portable app/core-plugin settings only when the team elects to keep optional Obsidian support.

**Source-analysis consolidation:**

- Notes 83 and 100 → one BMAD Method/Builder source analysis.
- Notes 84 and 85 → one explicitly historical OpenClaw review.
- Notes 86 and 87 → one Hermes review.
- Note 88 → refreshed Odysseus clean-room review.
- Note 89 → concise cross-source comparison rewritten around current OpenCode/BMAD direction.
- Note 90 → unique agent-development practices move to `docs/development/agent-development-workflow.md`; stale implementation sequence is archived.
- Note 92 → replaced by `reference-sources.lock.json` and a concise provenance explanation.

**Acceptance:**

- All 105 notes appear exactly once in the migration ledger.
- Original hashes are reproducible.
- No archived note appears in the current navigation except through an explicit “history/reference” route.
- `docs/current` contains no link to a legacy note as implementation authority.
- The current documentation is materially shorter than the root legacy corpus.

**Verification:**

```powershell
node tools/knowledge/check-docs.mjs
node tools/knowledge/check-authority.mjs
node tools/knowledge/verify-archive-manifest.mjs
```

**Review focus:** No unique evidence lost; no historical text accidentally promoted.

**Rollback:** Restore paths from `archive-manifest.json`; current docs remain independently usable.

---

### Task 7 — Externalize imported source repositories and create a real source lock

**Value:** Removes roughly 282 MiB and more than 20,000 vendor files from the normal Forge source/review boundary while improving provenance.

**Files:**

- Create: `reference-sources.lock.json`
- Create: `docs/reference/README.md`
- Create: `docs/reference/source-intake-policy.md`
- Modify: `THIRD_PARTY_NOTICES.md`
- Modify: `.gitignore`
- Remove after verified externalization:
  - `bmad-runtime-lib/_source_review/BMAD-METHOD-main`
  - `bmad-runtime-lib/_source_review/bmad-builder-main`
  - `bmad-runtime-lib/_source_review/openclaw-main`
  - `bmad-runtime-lib/_source_review/hermes-agent-main`
  - `bmad-runtime-lib/_source_review/odysseus-dev`
  - `bmad-runtime-lib/_full`
- Preserve/move first-party scripts before deleting `_source_review`.

**Lock record fields:**

| Field | Validation rule |
|---|---|
| `id` | Stable source ID, for example `bmad-method-6.10.0` |
| `purpose` | Explicit Forge use: sealed source, source analysis, or historical comparison |
| `repositoryUrl` | Verified canonical HTTPS repository URL |
| `version` | Exact package/release version when available |
| `commit` | Verified 40-character immutable Git commit |
| `archiveSha256` | Exactly 64 lowercase hexadecimal characters |
| `license` | SPDX expression plus component exceptions where required |
| `acquiredAt` | ISO-8601 UTC timestamp |
| `storageLocator` | Real approved internal artifact locator that the team can resolve |
| `includedPaths` | Minimal paths needed for the stated purpose |
| `excludedPaths` | Websites, media, dependencies, restricted assets, and unrelated files |
| `releaseDecision` | `sealed_input`, `reference_only`, `clean_room_pattern_only`, `excluded`, or `legal_review_required` |

The implementation may not merge with example values. Every checked-in record must contain a real canonical URL, immutable identity, hash, license, and approved internal locator.

**Source policy:**

- BMAD Method and Builder: align reference identity with the exact sealed sources used by `packages/bmad-foundation`; do not maintain a second divergent source copy.
- OpenCode: use the current pinned source under `/library/opencode` or an external lock as the active comparable-agent reference.
- OpenClaw: historical comparison only unless the team explicitly adopts it again.
- Hermes: retain only verified source-analysis evidence and the minimal source paths required to reproduce it.
- Odysseus: AGPL clean-room requirements only unless legal approves a different use.
- Restrictive PowerPoint assets remain excluded.
- No imported source script is executed by Forge CI.

**Offline reproducibility:**

Add a verifier that can:

1. Confirm lock shape and hashes without downloading anything.
2. Verify a locally supplied archive against the lock.
3. Materialize a temporary read-only review tree outside the repository.
4. Confirm included/excluded path policy.
5. Produce a review receipt without changing product source.

**Acceptance:**

- No full third-party repository remains under `bmad-runtime-lib`.
- Reference analyses cite `source-id:path@commit`, not personal paths or missing `_full` directories.
- Every retained source claim can be traced to a lock record.
- Repository clone and CodeQL/CodeRabbit boundaries no longer need to exclude an entire mixed first-party/vendor directory.
- `THIRD_PARTY_NOTICES.md` and lock decisions agree.

**Review focus:** Provenance, licenses, reproducibility, and no accidental deletion before a verified external copy exists.

**Rollback:** Restore the exact source archive from the approved storage locator and validate its hash; never reconstruct from an unpinned latest branch.

---

### Task 8 — Rebuild current architecture and source-analysis references around Forge’s actual direction

**Value:** Makes the documentation useful for future development rather than merely clean.

**Files:**

- Create or rewrite:
  - `docs/architecture/system-map.md`
  - `docs/architecture/runtime-boundaries.md`
  - `docs/architecture/workspace-and-execution.md`
  - `docs/architecture/state-evidence-and-recovery.md`
  - `docs/architecture/model-connectivity.md`
  - `docs/architecture/bmad-method-and-builder.md`
  - `docs/reference/source-analysis/bmad-method-builder.md`
  - `docs/reference/source-analysis/opencode.md`
  - `docs/reference/source-analysis/hermes.md`
  - `docs/reference/source-analysis/odysseus.md`
  - `docs/reference/source-analysis/historical-openclaw.md`

**Required architecture content:**

- `desktop-app` as local authority.
- Renderer as typed presentation client.
- Local workspace broker and sensitive-path policy.
- Governed reads, context review, decisions, edits, checkpointing, rollback, and recovery.
- ADR-0009’s general coding-agent direction.
- ADR-0010’s native Rust agent-runtime proposal/status and turn-scoped egress.
- Current safe read tools versus planned write/shell/Git tools.
- BMAD Method as the organizing interface and current capability denominator.
- Builder as inactive/partial until install/activate/execute lifecycle is proven.
- Direct-provider PoC versus no-Entra production support-plane target.
- Exact distinction between Azure OpenAI `/openai/v1/`, Foundry project endpoints, and the support-plane architecture as implemented—not merely planned.
- Current package/release and updater limitations.

**Source-analysis rules:**

- A source-analysis document explains what was observed, what Forge adopted, what Forge rejected, and the source identity.
- It does not prescribe current Forge behavior unless an accepted ADR or current source implements it.
- Every quoted path resolves against `reference-sources.lock.json`.
- Historical OpenClaw findings are not silently relabelled OpenCode findings.
- Clean-room boundaries are explicit.

**Acceptance:**

- A reviewer can trace every architecture statement to source, ADR, or current claim.
- OpenCode is represented as the current comparable coding-agent reference.
- No current architecture document depends on missing `_full` paths.
- Web-managed dual delivery appears only in archive/history.
- Current model-access docs do not require Entra.

**Review focus:** No architecture laundering—planned or reference behavior must not be written as implemented Forge behavior.

---

### Task 9 — Add documentation CI and correct review/security boundaries

**Value:** Prevents the new system from becoming another stale library.

**Files:**

- Create: `.github/workflows/docs.yml`
- Modify: `.github/codeql-config.yml`
- Modify: `.coderabbit.yaml`
- Modify: `package.json`
- Modify: `.github/pull_request_template.md`

**Workflow design:**

#### Pull-request gate

Trigger when any of these change:

```text
docs/**
README.md
CONTRIBUTING.md
AGENTS.md
reference-sources.lock.json
tools/knowledge/**
package.json
.node-version
.nvmrc
Cargo.toml
rust-toolchain.toml
global.json
apps/desktop-ui/package.json
```

Run:

```powershell
pnpm install --frozen-lockfile
pnpm docs:verify
git diff --check
```

The gate is offline and deterministic.

#### Scheduled external freshness job

Run weekly:

- verify official external links and review due dates;
- verify source artifact availability without executing source;
- open or update a tracking issue when a fact becomes overdue;
- do not silently rewrite docs or dependency pins.

#### Review boundary corrections

- Remove `bmad-runtime-lib/**` broad exclusions after first-party tools/current docs are moved.
- Exclude only the external vendor materialization directory, which is not checked in.
- Include `tools/knowledge/**`, `docs/current/**`, metadata schemas, and source locks in CodeRabbit review.
- CodeQL need not scan Markdown, but it must scan all first-party JavaScript validation code.

**CI separation rule:**

Keep product verification and docs verification distinct:

- `verify:source` proves product source.
- `docs:verify` proves current documentation integrity.
- Both may be required checks.
- External-network freshness is scheduled and separately reported.

**Acceptance:**

- A PR that changes a toolchain pin without regenerating docs fails.
- A PR that introduces a broken current-doc link fails.
- A PR that makes Entra a current requirement fails.
- A PR that adds a current claim without evidence fails.
- A PR that only changes archived historical prose does not require rewriting current claims unless the archive manifest drifts.
- First-party knowledge tooling receives normal code review.

**Review focus:** Low noise, deterministic failures, and correct separation from runtime qualification.

---

### Task 10 — Cut over entrypoints and retire the v1 vault tooling safely

**Value:** Completes the transition without leaving two systems active.

**Files:**

- Modify: `README.md`
- Modify: `.codex/config.toml`
- Modify: `HANDOFF.md` or archive it under `docs/archive/handoffs/`
- Modify: `docs/provenance/reference-vault.md` into a migration/archive record
- Modify: `package.json`
- Replace: `bmad-runtime-lib/README.md` with a compatibility notice
- Remove after evidence preservation:
  - `bmad-runtime-lib/knowledge-base/current`
  - `bmad-runtime-lib/knowledge-base/evidence`
  - `bmad-runtime-lib/_source_review/*.py`
  - old `vault:test` and `vault:check` scripts
  - stale `.obsidian/workspace.json`

**Compatibility window:**

For one repository milestone:

- `bmad-runtime-lib/README.md` points to `docs/README.md` and the archive manifest.
- Old `vault:check` may remain as `docs:check:legacy-archive` but must be named as archive integrity, not current validation.
- After the milestone, remove the compatibility directory if no automation or team workflow uses it.

**Cutover checks:**

```powershell
pnpm docs:verify
pnpm verify:source
git diff --check
rg -n "knowledge-base/current|Vault Map|Start Here - Sapphirus" README.md AGENTS.md .codex docs/current docs/development docs/architecture
rg -n "Sapphirus|Entra|Cortex" docs/current docs/development docs/architecture
```

Expected:

- no live links to v1 current authority;
- no product-facing Sapphirus name;
- no Entra requirement;
- no Cortex current architecture;
- only allowlisted legacy-ID or historical statements remain.

**Acceptance:**

- `/docs/README.md` is the only current team entrypoint.
- v1 files are archived with hashes.
- New scripts and metadata pass.
- Existing product source gate is not weakened.
- A fresh clone does not require `bmad-runtime-lib` to build, test, run, or understand the project.

**Review focus:** No dual authority left behind; no evidence deleted before archive verification.

---

### Task 11 — Run a real team-takeover validation and establish recurring maintenance

**Value:** Proves the documentation works for people rather than only for validators.

**Files:**

- Create: `docs/reviews/team-takeover-validation.md`
- Create: `docs/current/maintenance-cadence.md` or add the cadence to `documentation-policy.md`
- Update metadata review dates based on the result.

**Takeover exercise:**

A developer who did not author the migration must complete these tasks without consulting the legacy archive or the original maintainer:

1. Explain Forge’s purpose and current product modes.
2. Identify what works and what is blocked.
3. Locate the local setup and run commands.
4. Identify the authority boundary between renderer and Rust host.
5. Explain the current no-Entra model-access direction.
6. Identify accepted, partial, and planned BMAD work.
7. Run the fast docs gate.
8. Run one narrow product gate.
9. Select an active task and identify its owner, acceptance checks, and related ADRs.
10. Find the relevant historical/source-analysis material without mistaking it for current behavior.

Record time, confusion points, broken instructions, and corrections. Do not mark the migration complete until the exercise succeeds.

**Maintenance cadence:**

#### Per pull request

Update current docs when a change affects:

- product behavior or mode;
- architecture boundary;
- authentication/provider route;
- BMAD capability state;
- Builder lifecycle;
- toolchain pin;
- verification/release gate;
- operational procedure;
- source-reference identity.

#### Weekly

- Review current roadmap/known gaps.
- Reconcile active plan status with source and gates.
- Archive superseded handoffs.

#### Monthly

- Review current-document `reviewDue` dates.
- Re-run source-reference availability and license checks.
- Reconcile capability and release status.

#### Quarterly or before a pilot/release

- Independent docs-vs-source audit.
- Team onboarding replay.
- Reference-source provenance review.
- Archive and delete obsolete active plans.

**Acceptance:**

- Takeover exercise completes without legacy-author intervention.
- Every current doc has an owner and future review date.
- No overdue current claim remains current.
- A named team role owns each maintenance cadence item.

**Review focus:** Human usability and sustainable ownership.

---

## Parallel execution model

After Tasks 0 and 1 are accepted, work may proceed in four isolated workstreams:

| Workstream | Owns | Must not edit |
|---|---|---|
| A — Current authority | `docs/current`, current architecture, ADR index | archive paths, source lock, tooling |
| B — Archive and source provenance | `docs/archive`, `docs/reference`, `reference-sources.lock.json` | current docs, package scripts |
| C — Tooling and CI | `tools/knowledge`, metadata schemas, docs workflow | prose content except test fixtures |
| D — Onboarding and governance | `CONTRIBUTING.md`, `AGENTS.md`, development guides, CODEOWNERS | current product-state claims |

Integration-owned files:

```text
README.md
package.json
.codex/config.toml
docs/README.md
docs/_meta/knowledge-manifest.json
.github/pull_request_template.md
```

Only the integration owner edits those files. Workstreams submit requested changes through a small integration patch rather than modifying them concurrently.

Integration order:

```text
Task 1 decisions
→ Task 2 current layer
→ Tasks 3, 4, 6, 7, 8 in dependency-safe parallel work
→ Task 5 tooling integration
→ Task 9 CI
→ Task 10 cutover
→ Task 11 independent validation
```

---

## Verification gates

### Gate A — Baseline integrity

- Actual commit and dirty state recorded.
- Legacy archive hash recorded.
- Post-audit stabilization work accounted for.

### Gate B — Authority correctness

- One current entrypoint.
- No conflicting current authorities.
- Forge naming and no-Entra decision accepted.

### Gate C — Current-content accuracy

- Current docs verified against live source.
- No unsupported percentages.
- Capability and release states use named evidence.

### Gate D — Archive completeness

- All 105 notes mapped.
- Original hashes preserved.
- Preserved-verbatim notes unchanged.

### Gate E — Source provenance

- Every retained source has real URL, immutable identity, archive hash, license, purpose, and storage locator.
- All source-analysis paths resolve against the lock.

### Gate F — Tooling

- Unit tests pass.
- Generated files are byte-current.
- Fast docs gate completes under 15 seconds.
- Archive extraction without Git produces correct limitations.

### Gate G — CI/review

- Required docs workflow passes.
- First-party tooling is not excluded from review/security scans.
- External freshness is separate and scheduled.

### Gate H — Human takeover

- Independent teammate completes the takeover exercise.
- All confusion points are corrected and rechecked.

---

## Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Rewriting history while renaming | Loss of provenance | Preserve original files/hashes; put modern interpretation in new docs |
| Breaking stable `sapphirus.*` IDs | Contract/persistence failure | Separate human-facing rename from compatibility migration ADR |
| Deleting source snapshots before safe storage | Lost reproducibility | Verify real storage locator and hash before removal |
| Promoting plans to facts during rewrite | False authority | Claim classes and source/gate evidence required |
| Excessive link churn | Broken references and difficult review | Add index first; migrate in slices; maintain temporary path map |
| Validator becomes noisy | Team disables the gate | Separate current/archive rules; offline fast gate; unit-test every rule |
| Two agents overwrite shared indexes | Lost changes and contradictory metadata | One integration owner; generated index; explicit file ownership |
| No-Entra docs outrun implementation | False product-readiness claim | Separate current prototype, intended production model, and known gaps |
| External facts age again | Repeated stale baseline | Review due dates and scheduled official-source checks |
| Obsidian-specific features reduce portability | Team cannot review in normal tools | Standard Markdown is mandatory; Obsidian optional |
| Archive remains too visible | New team still follows old plans | Remove from current navigation; archive banner and authority checker |

---

## Definition of done

The modernization is complete only when all statements below are true:

1. `/docs/README.md` is the single current team entrypoint.
2. The documentation authority ADR is accepted.
3. The no-Entra model-access ADR is accepted and accurately scoped.
4. The Forge naming/legacy-ID ADR is accepted.
5. Current docs reflect the actual live worktree and latest stabilization changes.
6. Current docs contain no unsupported readiness percentages.
7. Current docs do not present Entra as required.
8. Current docs do not present Cortex as part of Forge architecture.
9. Active product text uses Forge.
10. Legacy technical identifiers are explicitly documented rather than blindly renamed.
11. Every current document has owner, source commit, verification date, and review due date.
12. All 105 legacy notes are accounted for in the migration ledger.
13. Original archive hashes verify.
14. No legacy note remains current authority.
15. Source-analysis documents resolve against real pinned source identities.
16. Partial source snapshots are not described as complete.
17. Full vendor repositories and unrelated media are outside the normal Forge source tree.
18. First-party knowledge tooling is under `/tools/knowledge`.
19. Broad CodeQL/CodeRabbit exclusion of the mixed library is removed.
20. `pnpm docs:verify` passes on the pinned toolchain.
21. The docs PR workflow is required and deterministic.
22. Product verification remains independent and is not weakened.
23. Empty onboarding placeholders and stale personal Obsidian state are removed.
24. Handoffs and completed plans have an archive lifecycle.
25. An independent teammate completes the takeover exercise.
26. The team assigns real owners and review cadence.
27. `bmad-runtime-lib` is reduced to a compatibility pointer or removed after the migration window.

---

## Immediate takeover milestone

Before the full migration is complete, the team should deliver this two-day minimum set:

1. Tasks 0 and 1 complete.
2. `docs/README.md` live.
3. All nine `docs/current` pages live and reviewed.
4. Onboarding/local setup/contribution guide live.
5. Pin generation and basic link/authority checks live.
6. Old `Start Here.md` points to `/docs/README.md` and clearly says it is legacy.
7. Current docs use Forge and no-Entra architecture.
8. A takeover engineer completes the 15-minute orientation.

This milestone gives the team a trustworthy development starting point while source externalization and archival consolidation continue.

---

## Handoff

**Next primary module:** implementation, because the authority model and migration slices are now sufficiently defined to execute without another broad design phase.

**Required review points:**

- Architecture review after Task 1.
- Source-truth review after Task 2.
- Tooling review after Task 5.
- Security/legal/provenance review before Task 7 deletion.
- Independent change review before Task 10 cutover.
- Human takeover review in Task 11.

**Recommended execution depth:** high for Tasks 1, 4, 5, 7, and 10; medium for content migration; independent reviewer required for source provenance and final authority cutover.

