---
title: "BMAD Validation Rules"
aliases:
  - "69 - BMAD Validation Rules"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 69
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: bmad-validation-rules
status: v6-modernized-implementation-guide
validated_on: 2026-07-09
---



# BMAD Validation Rules

## V6.17 package validation portability

Validation rule IDs, descriptor normalization, compatibility evaluation, diagnostics, and golden fixtures are shared. C# and Rust validators must produce equivalent canonical outcomes for the same package bytes. Delivery-specific rules then validate cloud catalog/worker compatibility or desktop host/OS/capability compatibility.

Desktop validation verifies signature/inventory, supported host/schema/BMAD range, required capabilities, platform assets, revocation/offline-grace state, and declared executable/setup behavior. Validation does not activate or execute a package; local activation and any hooks require separate policy/evidence.

## V6.18 current-authority validation overlay

This overlay is the current authority for Method and Builder validation. It is grounded in [[100 - BMAD Method and Builder Deep Comprehension Audit]] and supersedes conflicting universal naming, step, metadata, eval-format, or activation rules later in this historical reference.

### Validation profiles

Resolve a profile before applying rules:

| Profile | Applies to | Important boundary |
|---|---|---|
| `MethodOfficialSkillV6` | Official Method skills | Official `bmad-*` naming and applicable Method path rules. |
| `MethodStepWorkflowV6` | Method step-file workflows | Applicable `STEP-*`, sequence, pause, and forward-loading rules. |
| `BuilderOutcomeSkillV2` | User-created workflows/utilities | Inline-first, descriptive progressive disclosure; no mandatory numbered steps or `bmad-` prefix. |
| `BuilderUserAgentV2` | Builder agent output | Agent metadata plus stateless/memory/autonomous archetype checks. |
| `BuilderModuleV2` | Builder standalone/setup-skill modules | Full module/help/setup and roster consistency. |
| `InstalledPackage` | Installed Method/Builder workspace | Install-profile manifests, help, config, provenance, and file integrity. |
| `SapphirusPromotion` | Candidate release envelope | Cross-delivery compatibility, security, rehearsal, evaluation, signing, and activation eligibility. |

Profile ambiguity is a blocking import finding. Method official naming and step rules must never be applied blindly to Builder user output.

### Source metadata versus platform metadata

Upstream `SKILL.md` frontmatter requires `name` and `description`; body content must be present and the name must match the directory under the applicable profile. Inputs, outputs, invocation metadata, permissions, owner scope, and compatibility are normalized Sapphirus envelope fields, not universal upstream frontmatter requirements.

Upstream module fields are profile-specific. Parse the observed `module.yaml`, preserve unknown data without trusting it, and validate fields that the resolved source profile requires. Sapphirus then validates its richer outer descriptor separately; do not invent source-required dependency, capability, target, or version fields.

### Configuration graphs

Validate three independent graphs:

1. Method central TOML: `_bmad/config.toml`, `config.user.toml`, and the two `_bmad/custom/` config overlays with the source merge semantics below.
2. Per-skill customization TOML: baked base, team override, personal override; validate block type (`agent` or `workflow`), scalar/array types, merge behavior, stable package/skill identity, and declared field use.
3. Compatibility YAML: Method per-module YAML and Standalone Builder root YAML/help. Validate only under the matching `MethodCliV6` or `StandaloneBuilderSetupV2` install profile and never allow the standalone cleanup path to mutate Method CLI manifests.

Customization activation hooks, completion hooks, persistent facts, `file:` globs, and `skill:` references are untrusted prompt extensions. File references stay in allowed roots, skill references resolve through declared dependencies, and hook-requested side effects still require normal proposal/Airlock authority.

### Builder rule families

| Rule family | Required checks |
|---|---|
| `BUILDER-*` | Build/Edit/Analyze intent is explicit; headless result envelope is schema-valid; draft and memlog/evidence paths are separate; no generated text advances lifecycle. |
| `CUSTOM-*` | TOML parses; field types/defaults agree with schema; declared scalars are actually consumed; no policy/permission override; file/skill references are bounded and resolvable. |
| `AGENT-*` | Agent metadata is complete; archetype matches emitted files; memory/autonomous bootloaders and seeds are complete; init is transactionally rehearsable; PULSE declares behavior but does not create a schedule. |
| `MODULE-*` | Full YAML parse; profile-required identity/version/questions; exact help header; member coverage; setup implementation; standalone activation integration; agent roster/customize agreement; no unresolved manual setup requirements. |
| `HELP-*` | All 13 columns parse; field types and module/skill/action identity are valid; menu codes are unique in scope; predecessor/follower references resolve or declare an external dependency. |
| `EVAL-*` | Eval schema profile resolves; IDs and paths are safe; fixtures are bounded; baseline/candidate/adapters are immutable; pass/fail semantics are explicit; grader evidence is present when required. |
| `PKG-*` | Allowlisted inventory; no symlink/reparse/path escape/case collision; authoring/eval/runtime-state exclusions; deterministic hash and normalized descriptor. |
| `SCRIPT-*` | Script runtime and dependencies are declared; argv is typed; output roots are bound; candidate-controlled adapters/env passthrough are forbidden; setup/cleanup runs only in disposable rehearsal. |

### Safe eval/rehearsal rules

- Adapter IDs resolve only through an operator-owned, digest-pinned registry. A candidate-adjacent `adapter.json` is data to flag, never an executable command definition.
- Reject absolute paths, `..` traversal, unsafe archive entries, symlinks/junctions/reparse points, device files, and any resolved path outside the approved package or run root.
- Copy immutable candidate bytes into a sealed run; do not symlink the source tree. Keep source and fixtures read-only.
- Do not pass arbitrary host environment variables or provider credentials named by package content. Credentials are delivery-owned, least-privileged, and adapter-bound.
- Parse the structured result. Upstream `run_evals.py` and `run_triggers.py` may return process success while cases fail or skip.
- Baseline and variant require independent quality comparison over frozen cases; timing/token aggregation alone is not a pass.
- Preserve complete but redacted transcripts, artifacts, timing, grading, policy, adapter, model, tool-availability, and candidate hashes as evidence.

### Memory and autonomy gates

Memory and autonomous output validation can occur before runtime support, but promotion to an active capability is deferred until the owner-scoped sanctum and scheduler contracts exist. Sanctum data is never package inventory. Validate transactional First Breath initialization, manifest/schema version, bounded loads, retention/redaction, checkpoint/repair, and absence of executable code loaded from mutable memory.

Learned capabilities/scripts and memory deletions must produce governed proposals. Autonomous packages install disabled and cannot become active without schedule ownership, budget, quiet hours, capability allowlist, policy evaluation, and reversible activation.

### Gate semantics

Deterministic source findings, LLM quality findings, security findings, rehearsal facts, eval grades, approval, signing, registration, and activation are separate records. Advisory LLM grades never clear deterministic or security failures. Unknown/missing mandatory scanner, storage, dependency, license, rehearsal, or evaluator state fails closed.

## 1. File presence rules

| File | Required when | Validation |
|---|---|---|
| `SKILL.md` | every skill | source frontmatter `name` and `description`, body content, profile-specific naming/path rules; richer platform metadata is validated in the Sapphirus envelope. |
| `module.yaml` | every source module | profile-required identity/config/roster fields; richer dependency, capability, target, compatibility, and release metadata belongs in the normalized envelope when absent upstream. |
| `module-help.csv` | user-visible capability catalog | unique menu codes, valid phases/actions, output hints. |
| `bmad-modules.yaml` | registry/install source | module refs, channels, installation metadata. |
| `_bmad/config.toml` | Method central team config | parse, type, install-profile, and merge validation. |
| `config.user.toml` | Method central user config | allowed fields, ownership, and merge priority. |
| `_bmad/custom/*.toml` | config and per-skill overrides | graph/profile classification, type-safe merge, bounded references, no policy override, preserve unknown fields. |
| `_bmad/config.yaml`, `config.user.yaml`, per-module `config.yaml` | compatibility profiles only | distinguish Method compatibility YAML from Standalone Builder root YAML; ambiguity blocks import. |
| references/assets/scripts/agents/templates and profile variants | package-provided runtime files | allowlisted inventory, risk classification, dependency/runtime declaration, allowed output roots. |

Source-code aligned validation also recognizes generated installer manifests:

| File | Required when | Validation |
|---|---|---|
| `_bmad/_config/manifest.yaml` | installed BMAD workspace | parse, source hash/provenance, installed module list. |
| `_bmad/_config/skill-manifest.csv` | installed BMAD workspace | exact columns `canonicalId,name,description,module,path`; paths exist or produce findings. |
| `_bmad/_config/files-manifest.csv` | installed BMAD workspace | exact columns `type,name,module,path,hash`; hashes match when files are available. |

## 2. Structural rules

- Skill directory must have stable ID.
- Skill frontmatter must parse under the resolved profile; any outer envelope schema version must be known.
- Module version, when present or required by the resolved profile, must be semantic or explicitly supported.
- Declared dependencies must resolve; undeclared textual skill/tool/runtime requirements produce findings and block promotion when required.
- Help CSV rows must map to actual capability IDs.
- No duplicate menu codes within same module/phase.
- Output paths must be under allowed artifact roots.
- Scripts/templates are inventoried and risk-classified.
- Unknown metadata is preserved but not trusted.
- Package import is deterministic: same package hash produces same parsed catalog or same validation errors.

## 3. Config merge rules

The reviewed BMAD Method source resolves central config with a four-layer TOML merge (`src/scripts/resolve_config.py`, stdlib `tomllib`, Python 3.11+, invoked via `uv run`). Highest priority last:

```text
1. _bmad/config.toml               (installer-owned team)
2. _bmad/config.user.toml          (installer-owned user)
3. _bmad/custom/config.toml        (human-authored team, committed)
4. _bmad/custom/config.user.toml   (human-authored user, gitignored)
```

Source merge semantics (same rules in `resolve_customization.py`):

- scalars: override wins;
- tables: deep merge;
- arrays of tables where every item shares a `code` or `id` key: merge by that key;
- all other arrays: append.

The Sapphirus BMAD Kernel must reproduce this order and these semantics when interpreting installed workspaces, then apply its own run/session overrides as a runtime-only layer on top.

Validation requirements:

- later layers can override allowed runtime config fields;
- no layer can override Airlock policy, operator policy, command allowlist, network mode, or secret handling;
- invalid TOML blocks activation;
- deprecated fields produce warnings and migration suggestions;
- unknown fields are preserved in parsed metadata but excluded from policy decisions.

## 4. Security rules

- Package cannot define Airlock policy.
- Package cannot grant itself command/network permissions.
- Setup scripts are disabled or approval-gated in v1.
- Prompt-injection patterns are findings, not automatic execution blockers unless severe.
- Package cannot request secret values directly.
- Package cannot define external export destinations without approval policy.
- Package cannot mark generated files as trusted system instructions.

## 5. Registration gates

```text
parsed
→ structure_valid
→ config_valid
→ catalog_valid
→ security_reviewed
→ installation_rehearsed
→ invocation_rehearsed
→ registered
```

Invalid package never becomes active catalog capability.

## 6. Required validation fixtures

| Fixture | Purpose |
|---|---|
| minimal valid module | proves happy-path import. |
| missing `SKILL.md` | rejects incomplete package. |
| duplicate menu code | rejects ambiguous help catalog. |
| invalid TOML overlay | rejects bad config layer. |
| package attempts Airlock override | confirms policy boundary. |
| script requiring network | confirms approval-gated setup. |
| unknown metadata | confirms preservation without trust. |
| existing presentation workflow adapter package | proves adapter path. |

## 7. Deterministic Skill Validator Rules

The reviewed BMAD Method source defines deterministic validation in `tools/skill-validator.md`. Runtime workers should be able to run the equivalent command in a controlled package-validation job:

```text
node tools/validate-skills.js --json <skill-dir>
```

The command is evidence-producing, not a direct API action. It must run only inside an approved validation/rehearsal worker.

| Rule family | Required checks |
|---|---|
| `SKILL-*` | `SKILL.md` exists; frontmatter has `name` and `description`; `name` matches directory; description is usable as a trigger and no longer than the source limit. |
| `PATH-*` | Internal refs are relative from the originating file; external workspace refs use `{project-root}` or config variables; `{installed_path}` is invalid; one skill must not reference another skill's internal files directly. |
| `STEP-*` | For `MethodStepWorkflowV6` only: step files use `step-NN-description.md`; applicable workflows have 2 to 10 steps; each step has a goal and next-step guidance; menu pauses halt before presenting choices; no forward loading. Builder v2 inline-first skills use their own profile. |
| `SEQ-*` | Skill invocation language uses "invoke"; sequences do not assume future files are already loaded. |
| `TPL-*` | Templates do not use unsupported `{{.var}}` syntax and variable references are defined. |
| `REF-*` | References are resolvable, scoped, and not used to smuggle execution authority. |

The deterministic pass in `tools/validate-skills.js` implements these exact rule IDs, which the runtime validation report should reuse verbatim so findings map one-to-one to the source validator:

| Rule ID | Deterministic check |
|---|---|
| `SKILL-01` | `SKILL.md` exists. |
| `SKILL-02` / `SKILL-03` | Frontmatter has `name` / `description`. |
| `SKILL-04` | Under `MethodOfficialSkillV6`, name matches `^bmad-[a-z0-9]+(-[a-z0-9]+)*$` (lowercase, hyphens, no forbidden substrings). Builder user skills use their profile-specific naming and reserve `bmad-`. |
| `SKILL-05` | `name` equals the directory basename. |
| `SKILL-06` | Description quality: length limits plus a "Use when"/"Use if" trigger phrase. |
| `SKILL-07` | `SKILL.md` has body content after frontmatter. |
| `PATH-02` | No `{installed_path}` variable. |
| `STEP-01` | Under `MethodStepWorkflowV6`, step filenames match `step-NN[a-z]?-description.md`. |
| `STEP-06` | Under `MethodStepWorkflowV6`, step frontmatter has no `name`/`description` (skill-level only). |
| `STEP-07` | Under `MethodStepWorkflowV6`, step count between 2 and 10. |
| `SEQ-02` | No time estimates ("takes N min", "ETA", "estimated time"). |
| `TPL-01` | Template files contain no compile-time `{{.var}}` substitutions. |

Findings carry `CRITICAL`/`HIGH`/`MEDIUM`/`LOW` severity; the source gate is `--strict` failing on HIGH or above, and `--json` output is the machine-readable evidence format the validation worker should archive.

## 8. Builder Module and Eval Validation

Builder validation must cover both static package structure and runtime rehearsal.

| Gate | Required evidence |
|---|---|
| Builder skill output | Valid profile-specific `SKILL.md`; optional references/assets/scripts/agents/customization inventoried; official/user naming rules kept separate; authoring evidence excluded from runtime bytes. |
| Module Builder output | Multi-skill setup implementation or standalone self-registration implementation is complete with module/help assets, roster agreement, activation wiring, and no unresolved setup requirements. |
| Module help | Exact header, unique menu codes, valid module/skill/action identity, phases, predecessor/follower links, required flags, outputs, and output locations. |
| Eval format | Resolve `LegacyArtifactEvalV1`, `BuilderCaseV2`, or trigger schema; normalize fixtures/overlays/headless conventions; bind an operator-owned adapter and explicit result semantics before promotion. |
| Install rehearsal | Package installs into a clean workspace and produces expected `_bmad` manifests/config without touching `_bmad/custom` overrides. |
| Invocation rehearsal | A representative action runs through proposal, Airlock, approved worker execution, logs, manifest import, and evidence. |
