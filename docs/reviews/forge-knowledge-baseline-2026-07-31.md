---
title: "Forge knowledge migration baseline"
doc_type: review
status: completed
owner_role: forge-maintainers
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - docs/provenance/knowledge-migration-baseline.json
supersedes: []
---

# Forge knowledge migration baseline

Date: 2026-07-31  
Implementation branch: `main`  
Implementation commit: `7f8182fa3ba3461ca6c2d387aedc91663399e2f7`

## Outcome

The migration baseline is captured from the actual current repository, not the uploaded audit snapshot. The main worktree was clean when captured. The primary developer worktree pointed at the same commit and contained only a pre-existing untracked `.tmp/` directory, which this migration does not use as evidence or modify.

The machine-readable record is [knowledge-migration-baseline.json](../provenance/knowledge-migration-baseline.json).

## Post-audit stabilization

Commit `7f8182fa3ba3461ca6c2d387aedc91663399e2f7` (`feat: stabilize BMAD truth and Forge local PoC`) is the live repository representation of the post-snapshot stabilization work:

- 34 files changed;
- 1,037 insertions;
- 244 deletions;
- BMAD catalog/source-accounting truth, UI projections, provider validation, local PoC guidance, and supporting infrastructure/tests changed together.

The audit described an earlier 14-file pass. The baseline does not preserve that stale count as current truth; it records the actual committed slice.

## Toolchain

| Tool | Required | Observed | Result |
|---|---:|---:|---|
| Node.js | 24.18.0 | 24.18.0 at `C:\Users\rodri\AppData\Local\pnpm\bin\node.EXE` | Used for checks |
| Default PATH Node.js | 24.18.0 | 24.11.1 at `C:\Program Files\nodejs\node.exe` | Environment drift |
| pnpm | 11.15.1 | 11.15.1 | Match |
| Rust/Cargo | 1.97.0 | 1.97.0 | Match |
| .NET SDK | 10.0.302 | 10.0.302 | Match |
| Python | — | 3.10.0 | Existing v1 validator only |

The pinned Node binary was invoked explicitly. A shell that resolves the default Node binary first does not satisfy the repository pin.

## Narrow gate results

| Check | Exit | Result |
|---|---:|---|
| `node packages/bmad-foundation/scripts/generate-catalog.mjs --check` | 0 | Catalog and projections byte-current |
| `node --test packages/bmad-foundation/tests/bmad-capability-catalog.test.mjs` | 0 | 9/9 tests passed |
| `node tools/check-workflows.mjs` | 0 | 11 workflows resolved |
| `node tools/check-boundaries.mjs` | 0 | Architecture boundaries verified |
| `node tools/verify-reference-vault.mjs` | 0 | Frozen 105-note receipt integrity passed |
| `py -3 bmad-runtime-lib/_source_review/validate_library.py` | 1 | Live validation correctly exposed stale pnpm and Vite pins |

The green frozen vault check and red live validator are different evidence classes. The frozen check does not prove that current facts are correct.

## BMAD evidence

- Capability catalog SHA-256: `02ed1d13ccb2e9780b64624b04f1086c74b8fb10fb8aac1d7a858a586dc196bb`.
- BMAD status document SHA-256: `50907bb3e4c528d97ddabbac90b7bccba87d827ac104d78ce4c9d71c0bde33f9`.
- `.bmad100/run-state.json` SHA-256: `fdb5baffce9a57802a86f2834e407deff18044d76f7b288615af0cc8e0f6e15d`.
- The run-state says `running`, records `MTH-08` as current, and is anchored to older commit `2521f082aeab491157dc4222ecef76b3b5c2e5ac`.

That stale run-state is useful campaign evidence, but it cannot prove current whole-plan completion.

## Library inventory

| Item | Count/size |
|---|---:|
| Total files | 21,032 |
| Total bytes | 306,080,908 |
| Root Markdown notes | 105 |
| Root Markdown bytes | 2,614,251 |
| Root manifest SHA-256 | `5a37443db5fd922eba8d6a3499eb58a1f8b2291419bde318e6cce3b4aba80010` |
| Living manifest SHA-256 | `04da293a97fad7a246a25f231e3bf60cbba78c385aa1bc3c74069e037dbb290d` |

Imported snapshots account for 20,893 files across BMAD Method, BMAD Builder, OpenClaw, Hermes, Odysseus, and `_full`.

## Limitation

The historical `bmad-runtime-lib(1).zip` archive referenced by the review package is not available in the current Windows workspace or Downloads folder. Its SHA-256 cannot be truthfully recorded. Gate A therefore remains partial until that exact archive is supplied; the integrated root manifest and the modernization review package are hashed independently.
