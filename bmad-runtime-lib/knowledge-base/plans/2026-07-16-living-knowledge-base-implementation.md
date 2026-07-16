# Living Knowledge Base Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the reference vault into a commit-anchored, evidence-backed living knowledge base while preserving legacy evidence and product independence.

**Architecture:** Keep the 105 root notes as the legacy evidence layer and add a small `knowledge-base/current/` authority layer. Machine-readable claim, source, note-catalog, and pin registries are validated offline by a standard-library Python module integrated into the existing library validator; the frozen root manifest remains separate from a new living-layer manifest.

**Tech Stack:** Markdown, JSON, Python 3.10 standard library, `unittest`, existing Git/Node vault verification.

## Global Constraints

- Work only on `codex/living-knowledge-base` in `C:\tmp\bmad-kb`.
- Do not modify the user's original worktree or `bmad-runtime-lib/.obsidian/workspace.json`.
- Do not make product build, test, runtime, packaging, or CI depend on the library.
- Do not edit `_full/`, preserved imported sources, or unrelated parent-repository files.
- Treat commit `982887595caaade305fdd886909c6785c48d5e16` as the initial evidence anchor; later migration commits may be recorded as documentation provenance but cannot retroactively prove product behavior.
- Record uncommitted behavior as `WORKTREE_CANDIDATE`, never `IMPLEMENTED_FACT`.
- External claims use official primary sources and a second official corroborating source where available; single-source exceptions are explicit.
- All generated JSON is UTF-8 with LF and a trailing newline.

---

### Task 1: Living knowledge validation core

**Files:**
- Create: `bmad-runtime-lib/_source_review/tests/test_living_knowledge.py`
- Create: `bmad-runtime-lib/_source_review/living_knowledge.py`
- Modify: `bmad-runtime-lib/_source_review/validate_library.py`

**Interfaces:**
- Consumes: vault root `Path` and repository root `Path`.
- Produces: `ValidationResult(errors: list[str], warnings: list[str])` and `validate_living_knowledge(vault_root, repository_root)`.

- [ ] **Step 1: Write failing tests for missing registries and malformed claim IDs**

Create temporary vault fixtures and assert that validation reports missing `claims.json`, `sources.json`, `note-catalog.json`, `pins.json`, and rejects claim IDs outside `KB-[A-Z]+-[0-9]{3}`.

```python
def test_missing_registries_fail_closed(self):
    result = validate_living_knowledge(self.vault, self.repo)
    self.assertIn("knowledge-base/evidence/claims.json is missing", result.errors)

def test_claim_ids_are_closed(self):
    self.write_minimum_registries(claim_id="claim-one")
    result = validate_living_knowledge(self.vault, self.repo)
    self.assertIn("claims.json: invalid claim id 'claim-one'", result.errors)
```

- [ ] **Step 2: Run RED**

Run: `py -3 -m unittest discover -s bmad-runtime-lib/_source_review/tests -p "test_living_knowledge.py" -v`

Expected: import failure because `living_knowledge.py` does not exist.

- [ ] **Step 3: Implement the minimal validator module**

Implement:

```python
@dataclass
class ValidationResult:
    errors: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)

def validate_living_knowledge(vault_root: Path, repository_root: Path) -> ValidationResult:
    result = ValidationResult()
    evidence = vault_root / "knowledge-base" / "evidence"
    for name in ("claims.json", "sources.json", "note-catalog.json", "pins.json"):
        if not (evidence / name).is_file():
            result.errors.append(f"knowledge-base/evidence/{name} is missing")
    return result
```

Add closed enums and structural checks for claim/source/catalog records. Integrate the returned errors and warnings into `validate_library.py` without changing the existing root checks.

- [ ] **Step 4: Run GREEN and regression checks**

Run: `py -3 -m unittest discover -s bmad-runtime-lib/_source_review/tests -v`

Expected: all living-validator tests pass.

Run: `py -3 bmad-runtime-lib/_source_review/validate_library.py`

Expected: failure only because the real registries do not exist yet.

- [ ] **Step 5: Commit**

```powershell
git add -- bmad-runtime-lib/_source_review/living_knowledge.py bmad-runtime-lib/_source_review/tests/test_living_knowledge.py bmad-runtime-lib/_source_review/validate_library.py
git commit -m "test(vault): define living knowledge validation"
```

---

### Task 2: Evidence registries and complete root-note catalog

**Files:**
- Create: `bmad-runtime-lib/knowledge-base/evidence/claims.json`
- Create: `bmad-runtime-lib/knowledge-base/evidence/sources.json`
- Create: `bmad-runtime-lib/knowledge-base/evidence/note-catalog.json`
- Create: `bmad-runtime-lib/knowledge-base/evidence/pins.json`
- Create: `bmad-runtime-lib/_source_review/generate_note_catalog.py`
- Modify: `bmad-runtime-lib/_source_review/tests/test_living_knowledge.py`

**Interfaces:**
- Consumes: `manifest.json` root-note records.
- Produces: one explicit catalog record per root Markdown note and closed registry schema version `sapphirus.living-knowledge.v1`.

- [ ] **Step 1: Add failing coverage tests**

```python
def test_catalog_covers_every_root_manifest_note(self):
    self.write_minimum_registries(catalog_paths=[])
    result = validate_living_knowledge(self.vault, self.repo)
    self.assertIn("note-catalog.json: root-note coverage mismatch", result.errors)
```

- [ ] **Step 2: Run RED**

Run: `py -3 -m unittest discover -s bmad-runtime-lib/_source_review/tests -v`

Expected: catalog coverage test fails.

- [ ] **Step 3: Implement deterministic catalog generation**

Generate explicit records from the frozen manifest. Classify `05 - Preserved Source Context.md` and `06 - Preserved Critical Review.md` as `preserved_verbatim`; classify source-review notes `83` through `92` and `100` as `source_evidence`; classify operational notes `73`, `75`, `Library Quality Report.md`, `Start Here.md`, and `Vault Map.md` as `supporting_reference`; classify remaining root notes as `historical`. Each record includes `path`, `authorityClass`, `reason`, and `supersededBy: "knowledge-base/current/00-current-product-state.md"` when historical guidance can be mistaken for current authority.

- [ ] **Step 4: Create initial closed registries**

Use empty `claims` and `sources` arrays initially, explicit pin records, and the complete generated catalog. Pin records cover `.nvmrc`, root `package.json`, `global.json`, `Cargo.toml`, and `apps/desktop-ui/package.json` through `exact_text`, `json_path`, or `regex` modes.

- [ ] **Step 5: Run GREEN**

Run: `py -3 -m unittest discover -s bmad-runtime-lib/_source_review/tests -v`

Expected: all tests pass.

Run: `py -3 bmad-runtime-lib/_source_review/validate_library.py`

Expected: zero living-registry structural errors; root manifest remains verified.

- [ ] **Step 6: Commit**

```powershell
git add -- bmad-runtime-lib/knowledge-base/evidence bmad-runtime-lib/_source_review/generate_note_catalog.py bmad-runtime-lib/_source_review/tests/test_living_knowledge.py
git commit -m "feat(vault): classify legacy knowledge evidence"
```

---

### Task 3: Commit-anchored claims and current authority notes

**Files:**
- Modify: `bmad-runtime-lib/knowledge-base/evidence/claims.json`
- Modify: `bmad-runtime-lib/knowledge-base/evidence/sources.json`
- Create: `bmad-runtime-lib/knowledge-base/current/00-current-product-state.md`
- Create: `bmad-runtime-lib/knowledge-base/current/01-architecture-and-ownership.md`
- Create: `bmad-runtime-lib/knowledge-base/current/02-capability-matrix.md`
- Create: `bmad-runtime-lib/knowledge-base/current/03-security-and-trust.md`
- Create: `bmad-runtime-lib/knowledge-base/current/04-contracts-and-persistence.md`
- Create: `bmad-runtime-lib/knowledge-base/current/05-toolchain-and-dependencies.md`
- Create: `bmad-runtime-lib/knowledge-base/current/06-verification-and-release-readiness.md`
- Create: `bmad-runtime-lib/knowledge-base/current/07-risks-roadmap-and-open-decisions.md`
- Modify: `bmad-runtime-lib/_source_review/tests/test_living_knowledge.py`

**Interfaces:**
- Consumes: committed repository evidence at `98288759`, official URLs recorded with retrieval date `2026-07-16`, and the claim/source registry schema.
- Produces: eight authoritative notes whose frontmatter includes `authority: current`, `repository_commit`, `research_cutoff`, and a closed inline `claim_ids` list.

- [ ] **Step 1: Add failing claim-reference tests**

Assert that every current note has required frontmatter, every referenced claim exists, every claim has at least two source IDs unless `singleSourceException` is non-empty, and every current claim is referenced by at least one current note.

- [ ] **Step 2: Run RED**

Run: `py -3 -m unittest discover -s bmad-runtime-lib/_source_review/tests -v`

Expected: failure because the current notes and claim references are absent.

- [ ] **Step 3: Research and record sources**

Repository sources must use exact paths and commit locators. External sources must be official primary pages. Record Node 24.18.0 using the official release and official archive pages; TypeScript 7.0.2/no-public-API using the official TypeScript 7 announcement plus the repository pin as applicability evidence; React 19.2.7 using the official React versions page plus the repository pin; Vite 8.1 using the official announcement and releases page. Do not claim that a project pin is the latest version unless the official source directly proves it on the cutoff date.

- [ ] **Step 4: Populate material claims**

Cover Windows-local scope, `desktop-app` authority, untrusted renderer, optional vault boundary, D1 reads, sealed Help composition, D3 governed edits, non-integrated D2 production path, unsigned/unproven packaging, contract generation, persistence authority, exact toolchain pins, deferred web option, and current release blockers. Use `UNKNOWN` for Rust advisory status and any unverified clean-machine behavior.

- [ ] **Step 5: Write the eight current notes**

Each factual paragraph cites claim IDs. Capability status uses `implemented`, `implemented_not_product_integrated`, `scaffolded`, `planned`, `blocked`, or `unknown`. No percentage readiness score is introduced without a separately defined model.

- [ ] **Step 6: Run GREEN and content review**

Run: `py -3 -m unittest discover -s bmad-runtime-lib/_source_review/tests -v`

Expected: all tests pass.

Run: `py -3 bmad-runtime-lib/_source_review/validate_library.py`

Expected: zero errors and warnings.

- [ ] **Step 7: Commit**

```powershell
git add -- bmad-runtime-lib/knowledge-base/current bmad-runtime-lib/knowledge-base/evidence/claims.json bmad-runtime-lib/knowledge-base/evidence/sources.json bmad-runtime-lib/_source_review/tests/test_living_knowledge.py
git commit -m "docs(vault): publish current runtime authority"
```

---

### Task 4: Mechanical toolchain and authority-drift enforcement

**Files:**
- Modify: `bmad-runtime-lib/_source_review/living_knowledge.py`
- Modify: `bmad-runtime-lib/_source_review/tests/test_living_knowledge.py`
- Modify: `bmad-runtime-lib/knowledge-base/evidence/pins.json`

**Interfaces:**
- Consumes: pin records with `exact_text`, `json_path`, and `regex` selectors.
- Produces: deterministic mismatch errors naming pin ID, evidence path, expected value, and observed value.

- [ ] **Step 1: Add failing pin-drift and authority tests**

```python
def test_json_pin_drift_is_reported(self):
    self.write_pin(mode="json_path", path="package.json", selector=["packageManager"], expected="pnpm@11.12.0")
    self.write_repo_json("package.json", {"packageManager": "pnpm@11.9.0"})
    result = validate_living_knowledge(self.vault, self.repo)
    self.assertTrue(any("pin pnpm mismatch" in item for item in result.errors))
```

Also assert that no note outside `knowledge-base/current/` can declare `authority: current` and that current notes cannot cite `WORKTREE_CANDIDATE` as implemented.

- [ ] **Step 2: Run RED**

Run: `py -3 -m unittest discover -s bmad-runtime-lib/_source_review/tests -v`

Expected: new drift tests fail.

- [ ] **Step 3: Implement selector evaluation and authority checks**

Resolve repository-relative paths physically beneath the repository root; reject traversal and symlinks escaping the root. Compare exact text after one trailing-newline trim, traverse closed JSON key lists, and require one regex capture group for regex pins.

- [ ] **Step 4: Run GREEN**

Run: `py -3 -m unittest discover -s bmad-runtime-lib/_source_review/tests -v`

Expected: all tests pass.

Run: `py -3 bmad-runtime-lib/_source_review/validate_library.py`

Expected: all declared pins match repository evidence.

- [ ] **Step 5: Commit**

```powershell
git add -- bmad-runtime-lib/_source_review/living_knowledge.py bmad-runtime-lib/_source_review/tests/test_living_knowledge.py bmad-runtime-lib/knowledge-base/evidence/pins.json
git commit -m "feat(vault): detect knowledge authority drift"
```

---

### Task 5: Living-layer manifest and entrypoint migration

**Files:**
- Modify: `bmad-runtime-lib/_source_review/regenerate_manifest.py`
- Modify: `bmad-runtime-lib/_source_review/validate_library.py`
- Modify: `bmad-runtime-lib/_source_review/tests/test_living_knowledge.py`
- Create: `bmad-runtime-lib/knowledge-base/manifest.json`
- Modify: `bmad-runtime-lib/Start Here.md`
- Modify: `bmad-runtime-lib/75 - Library Validation Protocol.md`
- Modify: `bmad-runtime-lib/Library Quality Report.md`
- Modify: `bmad-runtime-lib/manifest.json`
- Modify: `docs/provenance/vault-validation.json`

**Interfaces:**
- Consumes: all tracked `knowledge-base/current/*.md` and `knowledge-base/evidence/*.json` except generated `knowledge-base/manifest.json`.
- Produces: deterministic living-layer file records and refreshed legacy manifest/provenance hashes.

- [ ] **Step 1: Add failing living-manifest tests**

Assert sorted relative paths, SHA-256, byte counts, LF generation, exact file-set coverage, and rejection of stale manifest records.

- [ ] **Step 2: Run RED**

Run: `py -3 -m unittest discover -s bmad-runtime-lib/_source_review/tests -v`

Expected: living-manifest tests fail.

- [ ] **Step 3: Extend deterministic generation**

Keep legacy root-manifest semantics unchanged and add a separate `knowledge-base/manifest.json`. Use `write_text(..., encoding="utf-8", newline="\n")` for both outputs.

- [ ] **Step 4: Migrate human entrypoints**

Make `Start Here.md` route first to `knowledge-base/current/00-current-product-state.md`, label the numbered root library as legacy/supporting evidence, update the validation protocol with exact local commands, and add a dated quality-report entry. Do not alter `.obsidian/workspace.json`.

- [ ] **Step 5: Regenerate integrity records**

Run: `py -3 bmad-runtime-lib/_source_review/regenerate_manifest.py`

Then run the Python validator, compute the new validator and root-manifest SHA-256 values, and update only those fields plus `validatedAtUtc` in `docs/provenance/vault-validation.json`.

- [ ] **Step 6: Run GREEN and frozen verifier**

Run: `py -3 -m unittest discover -s bmad-runtime-lib/_source_review/tests -v`

Run: `py -3 bmad-runtime-lib/_source_review/validate_library.py`

Run: `node tools/verify-reference-vault.mjs`

Expected: all checks pass with zero errors and warnings.

- [ ] **Step 7: Commit**

```powershell
git add -- bmad-runtime-lib/_source_review bmad-runtime-lib/knowledge-base/manifest.json "bmad-runtime-lib/Start Here.md" "bmad-runtime-lib/75 - Library Validation Protocol.md" "bmad-runtime-lib/Library Quality Report.md" bmad-runtime-lib/manifest.json docs/provenance/vault-validation.json
git commit -m "feat(vault): activate living knowledge validation"
```

---

### Task 6: Clean-snapshot proof, review, and durable handoff

**Files:**
- Modify: `bmad-runtime-lib/knowledge-base/current/06-verification-and-release-readiness.md`
- Modify: `bmad-runtime-lib/knowledge-base/current/07-risks-roadmap-and-open-decisions.md`
- Modify: `C:/Users/rodri/source/BigBrain/03-projects/sapphirus-bmad-runtime.md` after explicit write approval.

**Interfaces:**
- Consumes: committed migration branch and all validation commands.
- Produces: clean-snapshot proof, final finding disposition, and BigBrain maintenance record.

- [ ] **Step 1: Verify a clean Git archive**

Commit any verification-note corrections, archive the branch into a fresh short path under `C:\tmp`, and run:

```powershell
py -3 bmad-runtime-lib\_source_review\validate_library.py
node tools\verify-reference-vault.mjs
git status --porcelain
```

Expected: validators pass, status is empty.

- [ ] **Step 2: Run focused senior review**

Review architecture ownership, renderer/host separation, cloud/local authority, security/consent wording, contract/persistence facts, toolchain evidence, source quality, uncertainty, and supersession clarity. Every finding records severity, evidence, impact, confidence, and disposition.

- [ ] **Step 3: Fix review findings test-first where behavior changes**

Add or adjust validator tests before validator fixes. Documentation-only corrections use the explicit red exception: before/after claim-to-source comparison plus validator proof.

- [ ] **Step 4: Refresh manifests after final documentation edits**

Regenerate both manifests, refresh the provenance record, and rerun all three validation commands.

- [ ] **Step 5: Update BigBrain**

Record the living authority path, repository commit, research cutoff, claim taxonomy, validation commands, current limitations, and maintenance trigger. Do not copy raw logs or the full catalog.

- [ ] **Step 6: Final commit and checkpoint**

```powershell
git add -- bmad-runtime-lib docs/provenance/vault-validation.json
git commit -m "docs(vault): close living knowledge migration"
```

Run `git diff HEAD^ --check`, the complete validator suite, and `git status --short`. Handoff to change review; do not claim shipping or merge readiness.
