# Forge `bmad-runtime-lib` Deep Review

**Review date:** 2026-07-31  
**Primary target:** `bmad-runtime-dev-main/bmad-runtime-lib` from the latest uploaded Forge source snapshot  
**Historical baseline:** `bmad-runtime-lib(1).zip`, uploaded 2026-07-16  
**Post-snapshot context included:** the user-reported 14-file stabilization pass completed after the uploaded source snapshot  
**Posture:** read-only audit; no repository files were changed

## Executive verdict

`bmad-runtime-lib` contains valuable research, source reviews, historical architecture work, and the beginnings of a mechanically validated knowledge system. It should not be discarded.

It is, however, **not safe to use as the current development authority for the takeover team**.

The most accurate description is:

> **High-value reference archive; low-trust current knowledge base.**

The library currently mixes five different things under one directory:

1. 105 large legacy architecture and implementation notes.
2. A newer eight-note “current” knowledge layer.
3. First-party Python validation and generation tools.
4. Imported third-party source snapshots and media.
5. Obsidian workspace state and navigation.

Those layers have different ownership, review, security, update, and retention requirements. Treating them as one vault is now the central maintainability problem.

### Recommended decision

Use the repository’s existing `/docs` tree as the **single canonical team documentation system**. Reposition `bmad-runtime-lib` as a deliberately bounded historical/reference package:

- current facts and team guidance move to `/docs/current`, `/docs/development`, and accepted ADRs;
- useful source analysis moves to `/docs/reference/source-analysis`;
- legacy numbered notes become a frozen archive, not active navigation;
- imported upstream repositories move out of the normal source tree and are represented by a machine-readable source lock;
- first-party knowledge validators move to `/tools/knowledge` and become normal reviewed code;
- optional Obsidian compatibility may remain, but GitHub/VS Code-compatible Markdown becomes the baseline.

This avoids another wholesale rewrite while preserving the research investment.

---

## Scope and evidence inspected

### Repository evidence

- Root `README.md`, `package.json`, `.codex/config.toml`, CI workflows, CodeQL and CodeRabbit configuration.
- All 105 root Markdown notes under `bmad-runtime-lib`.
- The eight current authority notes under `bmad-runtime-lib/knowledge-base/current`.
- `claims.json`, `sources.json`, `pins.json`, `note-catalog.json`, both manifests, and the recorded vault-validation receipt.
- First-party library scripts under `bmad-runtime-lib/_source_review` and their tests.
- Current `/docs` structure: ADRs, implementation packets, BMAD status, Foundry operations, quality, security, reviews, plans, and provenance.
- Imported BMAD Method, BMAD Builder, OpenClaw, Hermes, and Odysseus source trees.
- Historical archive `bmad-runtime-lib(1).zip`.
- Latest full-project audit and its 52-finding ledger.
- User-reported post-audit stabilization results.

### Commands run

```text
python bmad-runtime-lib/_source_review/validate_library.py
python -m unittest discover -s bmad-runtime-lib/_source_review/tests -v
node tools/verify-reference-vault.mjs
node --test tools/tests/closed-manifest.test.mjs
```

Additional read-only scans measured file counts, sizes, terminology, frontmatter, links, source-path references, repeated blocks, CI integration, and archive drift.

### Source-quality note

The uploaded repository is a ZIP extraction without `.git`. Repository-commit validation therefore cannot prove that the recorded July 16 commit exists in this extraction. This is an environment limitation, but it also exposes a design weakness: the validator treats “not running in a Git checkout” as the same class of failure as “recorded commit unavailable.” The modernization plan separates these cases.

---

## Quantitative inventory

| Area | Measured state |
|---|---:|
| Total files under `bmad-runtime-lib` | **21,035** |
| Uncompressed size | **306,143,548 bytes — approximately 292 MiB** |
| Root Markdown notes | **105** |
| Root Markdown lines | **45,710** |
| Root Markdown bytes | **2.61 MiB** |
| Files under `_source_review` | **20,873** |
| `_source_review` size | **approximately 282 MiB** |
| Living-knowledge Markdown notes | **10** total, including 8 current notes and 2 spec/plan notes |
| Registered claims | **17** |
| Registered sources | **28** — 22 repository, 6 external official |
| Registered toolchain pins | **8** |
| Root notes catalogued as historical | **87** |
| Root notes catalogued as superseded | **92 of 105** |
| Substantial repeated-content groups | **20 detected** |
| Broken normal Markdown links | **2** |
| Broken wiki links in the scanned set | **0** |
| First-party living-knowledge tests | **20/20 passed** |
| Frozen `vault:check` | **passed** |
| Live Python library validator | **failed** |

### Imported-source footprint

| Snapshot | Files in the integrated library | Size |
|---|---:|---:|
| OpenClaw | 12,807 | ~140.4 MiB |
| Hermes | 5,873 | ~101.0 MiB |
| Odysseus | 1,300 | ~27.2 MiB |
| BMAD Method | 584 | ~10.5 MiB |
| BMAD Builder | 299 | ~2.7 MiB |

The source snapshots, media, fonts, and generated site assets dominate the library’s size and review surface. They are reference material, not Forge product code.

---

## Checklist status

| Checklist | Status | Evidence | Confidence |
|---|---|---|---|
| Current product accuracy | **Blocked** | Current notes are anchored to 2026-07-16 and omit Forge/no-Entra/native-agent/BMAD-100 changes | High |
| Authority clarity | **Blocked** | `/docs`, `knowledge-base/current`, root notes, `Start Here`, `Vault Map`, and provenance docs disagree | High |
| Archive integrity | **Mixed** | Root-note manifest is exact; imported source completeness claims do not match the integrated paths | High |
| Navigation/onboarding | **At risk** | Two broken entry links, empty onboarding files, no `/docs/README.md`, stale Obsidian state | High |
| Documentation maintainability | **At risk** | 105 flat notes, 92 superseded, large repeated template blocks | High |
| Validation/tooling | **At risk** | Useful tests exist, but the green frozen check does not prove current semantic validity | High |
| CI/review integration | **Blocked** | Library excluded from CodeQL/CodeRabbit and absent from normal source CI | High |
| Source provenance/licensing | **At risk** | Good prose ledger, but missing/partial trees and no machine-readable authoritative source lock | High |
| Team ownership/governance | **Missing** | No CODEOWNERS, contribution guide, docs owner model, or review cadence | High |
| Optional Obsidian usability | **Stale** | Workspace references Sapphirus, missing files, and paths that no longer exist | High |

---

## Ranked findings

### Critical findings

#### C-01 — The “current” knowledge layer is stale and materially wrong

- **Status:** Broken as current authority.
- **Evidence:** Every current note is anchored to commit `f18ef124a6e61754391b793a1b37a8a0f67491ab` with a `research_cutoff` of 2026-07-16. It still names Sapphirus, lists pnpm `11.12.0` and Vite `8.1.4`, and describes deterministic Help/managed brokerage as the shortest path to a real AI desktop.
- **Current source conflict:** The project is now Forge; package metadata pins pnpm `11.15.1` and Vite `8.1.5`; the source contains direct OpenAI/Azure OpenAI prototype providers, ADR-0009/0010, OpenCode adaptation work, BMAD-100 status, and substantially newer audits.
- **User-direction conflict:** Active development must work without Entra. The library contains 191 Entra mentions across 47 Forge-owned notes and does not have a current no-Entra architecture note.
- **Impact:** New maintainers can select the wrong architecture, authentication model, provider route, product scope, or implementation sequence.
- **Confidence:** High.
- **Next module:** Implementation after this modernization plan is approved.

#### C-02 — There is no single documentation authority

- **Status:** Broken governance.
- **Evidence:**
  - `knowledge-base/current` says numbered notes are historical.
  - `note-catalog.json` marks 92 root notes superseded.
  - `Start Here.md` still routes implementation work through numbered notes 01, 08, 51, 61, 72, and 93–100.
  - `Vault Map.md` calls note 93 “current authority” and note 51 “the canonical implementation sequence.”
  - `docs/provenance/reference-vault.md` says notes 93–100 override older text.
  - Newer accepted/proposed ADRs and implementation packets under `/docs` materially supersede those notes.
- **Impact:** Two competent developers can follow different “canonical” documents and both believe they are correct.
- **Confidence:** High.
- **Next module:** Architecture/documentation implementation.

#### C-03 — Imported source evidence claims do not match the source trees present

- **Status:** Broken evidence integrity.
- **Evidence:** Notes 84–92 repeatedly identify `_full/o/openclaw-main` and `_full/h/hermes-agent-main` as complete authoritative source trees. In the integrated library:
  - `_full/o/openclaw-main` does not exist;
  - `_full/h/hermes-agent-main` contains only 30 infographic-related files and no `package.json`;
  - the actual OpenClaw tree is under `_source_review` with 12,807 files, while the notes claim 21,980 regular files;
  - the actual Hermes tree has 5,873 files, while the notes claim 6,205 regular files;
  - at least 21 explicit source-path references do not resolve as written.
- **Impact:** Source-derived architectural claims cannot be reproduced from the checked-in library. A new team may mistake partial snapshots for complete reviewed evidence.
- **Confidence:** High.
- **Next module:** Source-provenance implementation and independent review.

#### C-04 — A green vault check can coexist with a red live validator

- **Status:** False confidence.
- **Evidence:**
  - `node tools/verify-reference-vault.mjs` passes because it verifies that a July 16 validation receipt and the frozen manifests have not changed.
  - `python bmad-runtime-lib/_source_review/validate_library.py` fails on current pin drift and cannot verify 22 repository sources in the extracted snapshot.
  - The frozen check does not evaluate whether current facts, links, names, claims, source paths, or authentication decisions are still correct.
- **Impact:** CI or reviewers can report “reference vault verified” while the current guidance is stale or inconsistent.
- **Confidence:** High.
- **Next module:** Validation-tooling implementation.

#### C-05 — The entire library is outside normal security and code-review coverage

- **Status:** Risky boundary design.
- **Evidence:** Both `.github/codeql-config.yml` and `.coderabbit.yaml` exclude `bmad-runtime-lib/**`. This excludes imported vendor material, but also excludes Forge-owned validators, generators, manifests, claims, and current authority notes.
- **CI evidence:** `verify:source` does not call `vault:check`; the source workflow runs `verify:source`; therefore neither the frozen vault check nor the live validator is a normal pull-request gate.
- **Impact:** First-party knowledge tooling and current authority can drift without the same review and security controls as the rest of the repository.
- **Confidence:** High.
- **Next module:** CI/tooling implementation.

### Watch findings

#### W-01 — The root library is mostly superseded but remains visually dominant

- **Evidence:** 87 of 105 root notes are classed historical and 92 are marked superseded. They remain at the top level, numbered, large, and heavily cross-linked.
- **Impact:** File position and numbering communicate authority more strongly than the small note-catalog registry does.
- **Confidence:** High.

#### W-02 — Large template blocks create the appearance of detail without proportional current value

- **Evidence:** Six generic blocks appear in 43 notes; several additional invariant/state/acceptance blocks appear in 41–42 notes; component edge-case and release sections repeat in 20 notes.
- **Impact:** Maintainers must read tens of thousands of lines to find unique decisions. Updating a common rule requires many manual edits and creates hidden contradictions.
- **Confidence:** High.

#### W-03 — Human-facing naming and stable technical identifiers are not separated

- **Evidence:** The library contains 455 Sapphirus references and only four Forge references. Current source still contains some stable `sapphirus.*` schemas and package identifiers for compatibility.
- **Impact:** A blanket rename could break contracts, while doing nothing leaves takeover documentation branded incorrectly.
- **Confidence:** High.
- **Required distinction:** Rename product-facing text to Forge. Preserve legacy package/schema IDs until a compatibility ADR and migration prove they can change safely.

#### W-04 — Entra and web-managed architecture remain overrepresented

- **Evidence:** Entra appears 191 times, `web_managed` 180 times, and `windows_local` 187 times in the library’s own notes. The current user direction is no-Entra and the current product is a Windows desktop application.
- **Impact:** Historical alternatives occupy more documentation surface than the current implementation path.
- **Confidence:** High.

#### W-05 — Current OpenCode/native-agent decisions are absent

- **Evidence:** The library’s own root/current documentation contains no OpenCode references, while active `/docs` includes OpenCode reviews and ADR-0009/0010 defining Forge as a general coding agent with a native Rust agent runtime.
- **Impact:** The library does not explain the product direction that matters most for future development.
- **Confidence:** High.

#### W-06 — The living-knowledge schema is brittle and brand-bound

- **Evidence:** Validators hardcode `sapphirus.living-knowledge.v1`, exactly eight current notes, a fixed external-host allowlist, and one shared repository anchor for all current notes.
- **Impact:** Adding a current topic requires changing validator code; claims cannot evolve independently; schema naming preserves the old product name.
- **Confidence:** High.

#### W-07 — Pin values are duplicated instead of derived

- **Evidence:** `pins.json` repeats values already owned by `.nvmrc`, `package.json`, `Cargo.toml`, and `global.json`; pnpm and Vite have already drifted.
- **Impact:** The knowledge base becomes another package manifest requiring manual synchronization.
- **Confidence:** High.

#### W-08 — Current percentage estimates are not mechanically supportable

- **Evidence:** `02-capability-matrix.md` warns that statuses are not percentages, then gives ten percentage estimates with uncertainty bands. The latest audit and BMAD-100 denominator now use different, more explicit evidence.
- **Impact:** Teams may present or plan against numbers that have no stable denominator or automated calculation.
- **Confidence:** High.

#### W-09 — The team has no documentation ownership or contribution contract

- **Evidence:** No CODEOWNERS, CONTRIBUTING file, AGENTS.md, documentation policy, owner metadata, review due dates, or archival rules.
- **Impact:** Documentation freshness depends on individual memory and agent behavior.
- **Confidence:** High.

#### W-10 — Onboarding is incomplete

- **Evidence:** `start/getting-started.md`, `start/onboarding.md`, and `tools/browser.md` are empty. `Start Here.md` contains two broken links. `/docs` has 129 files but no README or index.
- **Impact:** A takeover engineer has no short, reliable path from product purpose to local setup, architecture, gates, known gaps, and a first contribution.
- **Confidence:** High.

#### W-11 — The repository has three different meanings of “library”

- **Evidence:**
  - `bmad-runtime-lib` is a reference vault;
  - `/library/opencode` is an upstream source checkout;
  - `knowledge-base` is an active claim system inside the vault.
- **Impact:** Tooling and team discussion can confuse current docs, imported source, and historical research.
- **Confidence:** High.

#### W-12 — Imported repositories create unnecessary review, storage, and legal surface

- **Evidence:** More than 20,000 vendor files include source, websites, media, fonts, changelogs, bundled JavaScript, and assets. The library’s own ledger already identifies MIT, Apache-2.0, restrictive Anthropic asset terms, and AGPL boundaries.
- **Impact:** Repository clones, scans, backups, diffs, and legal inventories become heavier even though the product does not build or execute these files.
- **Confidence:** High.

#### W-13 — Obsidian support is stale and currently stronger than repository-native usability

- **Evidence:** `.obsidian/workspace.json` uses `main-sapphirus-vault` and references missing `_full` paths, old files, personal local paths, and archived notes. Standard Markdown links are not comprehensively checked.
- **Impact:** The vault appears curated in Obsidian but is less reliable in GitHub, VS Code, and automated review—the tools a future team is most likely to use.
- **Confidence:** High.

#### W-14 — The historical archive and active product documentation are not lifecycle-separated

- **Evidence:** Current notes, old plans, source audits, generated manifests, Obsidian state, and imported source all live under one root and share one exclusion policy.
- **Impact:** Archival immutability and current-document editability conflict. A manifest that freezes everything discourages legitimate updates; an editable current layer weakens archive guarantees.
- **Confidence:** High.

### Healthy foundations worth preserving

#### I-01 — Root-note byte integrity is mechanically verified

The root manifest correctly normalizes line endings and verifies the exact 105-note set, lines, bytes, and SHA-256 values.

#### I-02 — The living-knowledge tests are substantive

All 20 tests passed. They cover conflicting claims, supersession cycles, broken current-note links, source rules, pins, current-authority placement, and manifest closure.

#### I-03 — The claim/source/pin distinction is the right idea

Separating repository facts, official external facts, toolchain pins, and note authority is a sound foundation. The problem is stale data and an overly rigid v1 schema, not the concept.

#### I-04 — The product is correctly independent of the library

The runtime and release build do not depend on the reference library. The modernization must preserve this property.

#### I-05 — License risks are not being ignored

The source-adoption ledger explicitly distinguishes MIT, Apache-2.0, restrictive asset terms, and AGPL clean-room boundaries. This should become machine-readable rather than being discarded.

---

## Content disposition

### Keep and modernize

- BMAD Method/Builder source analysis, after reconciling it with the exact sealed source currently used by `packages/bmad-foundation`.
- The claim/source/pin evidence model, migrated to `forge.knowledge.v2`.
- Deterministic manifests and hash validation for frozen archives.
- Source-intake and license-decision concepts.
- The strongest local-authority, Airlock, evidence, rollback, and BMAD semantic research—after extraction into concise current architecture docs or source-analysis references.
- Optional Obsidian metadata that does not compromise normal Markdown use.

### Extract, then archive

- Useful boundaries from notes 94–98.
- Agent development discipline from note 90.
- Valid current technology rationale from notes 76, 82, 91, and the fact-check note.
- Unique test, threat-model, trace, and recovery details not already represented in code, ADRs, or current `/docs`.

### Archive without treating as current

- Web-managed and dual-delivery architecture.
- Old phase plans, week-by-week plans, master sequences, and implementation checklists.
- Old audits and correction logs.
- Old Sapphirus product and presentation files.
- Generic component templates repeated across dozens of notes.

### Replace

- `Start Here.md` and `Vault Map.md` → generated `/docs/README.md` and metadata-driven indexes.
- `pins.json` hardcoded values → generated repository pin projection.
- prose-only source snapshot ledger → `reference-sources.lock.json` plus license decisions.
- frozen validation receipt as current proof → current docs CI plus a separate immutable archive receipt.
- Entra-centric current guidance → explicit no-Entra model-access and credential architecture.

### Remove from the normal repository source tree

- Full or partial OpenClaw/Hermes/Odysseus source repositories and unrelated media/site assets.
- Empty `start/` and `tools/` placeholders unless they are populated during the cutover.
- Personal Obsidian workspace state; retain only shared portable settings when useful.

---

## Target authority model

The future team should use this precedence, from strongest to weakest:

1. **Executable source, tests, generated contracts, and current verification output** — what the product actually does.
2. **Accepted ADRs** — why an architecture decision is binding.
3. **`/docs/current`** — concise source-backed description of the current product, generated or freshly verified against a named commit.
4. **Operational runbooks** — exact supported procedures, owners, and rollback paths.
5. **Active implementation plans** — intended future work; never evidence that behavior already exists.
6. **Dated reviews and audits** — point-in-time findings with a source commit and expiration/review date.
7. **Source-analysis references** — patterns and provenance, not Forge behavior authority.
8. **Legacy archive** — historical context only.

A document at a lower level may not override a higher level. This rule should be enforced in metadata and review templates, not merely described in prose.

---

## Overall assessment

| Dimension | Assessment |
|---|---|
| Research depth | Strong |
| Historical preservation | Strong but structurally mixed |
| Current product accuracy | Blocked |
| Source-evidence reproducibility | Blocked |
| Team onboarding | Inadequate |
| Maintenance cost | High |
| Automation foundation | Promising but misapplied |
| Suitability as future team authority today | **No** |
| Suitability after the proposed migration | **Yes** |

The correct remediation is not to update all 105 notes in place. That would preserve the same structural failure with newer wording. The team needs an authority reset, concise current documentation, a frozen legacy archive, externalized source snapshots, reviewed first-party tooling, and ownership rules.

---

## Generated supporting evidence

- `forge-library-review-evidence.json` — machine-readable inventory, command results, terminology, links, duplication, source snapshots, and CI integration.
- `forge-library-note-migration-matrix.csv` — recommended disposition for all 105 root notes.
- `forge-library-unresolved-source-paths.json` — unresolved source-path citations found in the root notes.

