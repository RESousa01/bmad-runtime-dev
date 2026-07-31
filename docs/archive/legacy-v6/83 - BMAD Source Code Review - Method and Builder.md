---
title: "BMAD Source Code Review - Method and Builder"
aliases:
  - "BMAD Source Evidence"
  - "Source Review - BMAD Method and Builder"
tags:
  - bmad-runtime
  - vault/source-and-research
section: "Source and Research"
order: 83
vault_role: "source-review"
project: Sapphirus BMAD Runtime
status: source-evidence
reviewed_on: 2026-07-09
source_archives:
  - "C:\\Users\\rodrigocsousa\\source\\bmad-runtime-dev\\BMAD-METHOD-main.zip"
  - "C:\\Users\\rodrigocsousa\\source\\bmad-runtime-dev\\bmad-builder-main.zip"
---

# BMAD Source Code Review - Method and Builder

> Preliminary package/install/provenance review. Its facts remain evidence, but it is not the complete Method/Builder semantic audit. Current semantic authority is [[100 - BMAD Method and Builder Deep Comprehension Audit]]. Apply delivery-specific conclusions through [[93 - Split Web and Windows Desktop Architecture Plans]] and [[99 - Dual-Delivery Contract and Conformance Specification]].

This note records the first-pass source/package facts that updated the implementation plan. Read the full semantic and productization correction in [[100 - BMAD Method and Builder Deep Comprehension Audit]], then use [[13 - BMAD Kernel, Package Loader, and Help Advisor]], [[14 - Builder Studio and SkillOps]], [[39 - BMAD Package Format]], and [[69 - BMAD Validation Rules]]. Where this note calls Convert an upstream Builder capability or treats upstream manifests as final installation evidence, note `100` supersedes it.

## Reviewed Archives

| Archive | Reviewed focus |
|---|---|
| `BMAD-METHOD-main.zip` | Installer, module registry, core/BMM module contracts, help CSV schema, skill validator, web bundle metadata. |
| `bmad-builder-main.zip` | Builder module contract, Builder skills, module packaging docs, eval format, command/reference docs. |

The archives were extracted locally under `_source_review/` for review.

## Snapshot Identity And Review Limits

| Source | Declared identity in this snapshot | License / reuse boundary | Snapshot evidence |
|---|---|---|---|
| BMAD Method | npm package `bmad-method` `6.10.0` | MIT code license. The license also carries an explicit BMad trademark notice, so code reuse and product naming are separate decisions. | `BMAD-METHOD-main.zip` SHA-256 `A7C049038099B99081FBD03D22C6A5180EDD88DEE656BB37C4276B1CC31B4A32` |
| BMAD Builder | npm metadata `bmad-builder` `2.1.0`; module descriptor `module_version: 1.0.0` | MIT code license with the same BMad trademark notice. `package.json` is marked `private`, so this source snapshot must not be treated as proof of a publishable npm artifact. | `bmad-builder-main.zip` SHA-256 `D3C70744A9875623B01856CC907CF558324BACC920F0D860C36AD2788A4D2852` |

The ZIP snapshots do not contain Git metadata, so commit SHA, tag provenance, and working-tree state could not be verified. Before implementation, Source Intake must record upstream URL, tag or commit, archive digest, license digest, acquisition date, and the exact fixtures generated from that source. This review inspected source and ran dependency-free Python compile/help smokes; it is not an upstream release certification and did not run the full Node/Python test suites.

## Key Source Facts

| Area | Source finding | Plan impact |
|---|---|---|
| BMAD Method package | `bmad-method` version `6.10.0` exposes CLI bins `bmad` and `bmad-method` through `tools/installer/bmad-cli.js`. | Package import and install rehearsal must model the real CLI and generated `_bmad` folder. |
| Runtime prerequisites | Method requires Node `>=20.12.0`; Builder requires Node `>=22.0.0`; Method docs also require Python `>=3.10` and `uv`. | Worker images need explicit runtime variants for Method import, Builder rehearsal, and validator runs. |
| Python floor is contract-specific | Method advertises Python `>=3.10`, but `resolve_config.py`, `resolve_customization.py`, and multiple core skill helpers require Python `3.11+` for stdlib `tomllib`; Builder scripts declare a mixture of `>=3.9` and `>=3.10`. | Do not model one global "BMAD Python version." Record a required runtime per script/package operation and test the exact worker image used for it. |
| Installed manifests | Installer generates `_bmad/_config/manifest.yaml`, `skill-manifest.csv`, `files-manifest.csv`, central `config.toml`, `config.user.toml`, and override files under `_bmad/custom/`. | BMAD Kernel must parse installed manifests/config as first-class runtime evidence. |
| Assembled help catalog | Installer merges every installed `module-help.csv` into `_bmad/_config/bmad-help.csv` (`tools/installer/core/installer.js`); the `bmad-help` skill reads that assembled file at runtime and uses `_meta` rows for module documentation links. | Help Advisor consumes the assembled catalog, not per-module CSVs; see [[13 - BMAD Kernel, Package Loader, and Help Advisor]]. |
| Config resolver script | `src/scripts/resolve_config.py` performs a four-layer TOML merge (`config.toml` → `config.user.toml` → `custom/config.toml` → `custom/config.user.toml`) with typed merge semantics; stdlib-only, Python 3.11+, invoked via `uv run`. | Kernel config resolution must reproduce this exact order and semantics; see [[69 - BMAD Validation Rules]]. |
| Deterministic validator rules | `tools/validate-skills.js` implements rule IDs `SKILL-01..07`, `PATH-02`, `STEP-01/06/07`, `SEQ-02`, `TPL-01` with severities and a `--strict` HIGH+ gate and `--json` evidence output. | Validation reports should reuse these rule IDs verbatim; see [[69 - BMAD Validation Rules]]. |
| Config ownership | Installer-managed config is regenerated on install; `_bmad/custom/*` is user/team-owned. Builder docs also define per-skill custom files. | Runtime must preserve custom overrides and avoid writing installer-managed config except through an approved install/update path. |
| Module registry | `bmad-modules.yaml` is the bundled official registry and supports channels, aliases, deprecation, plugin marketplace metadata, npm packages, and module definitions. | Registry parsing must support official and external modules without assuming a remote marketplace lookup. |
| Module help schema | `module-help.csv` columns are `module,skill,display-name,menu-code,description,action,args,phase,preceded-by,followed-by,required,output-location,outputs`. | Help Advisor capability graph must be built from these exact columns. |
| BMM module | `src/bmm-skills/module.yaml` defines module `bmm`, default-selected, artifact folder prompts, and an agent roster. | Parser must preserve config prompts, result path expressions, directories, and `agents` definitions. |
| Core module | `src/core-skills/module.yaml` defines shared prompts such as `user_name`, `project_name`, language settings, and `output_folder`. | Config resolver needs scoped defaults and project-root/result expression handling. |
| Web bundles | `web-bundles/bundles.json` schema version `1.0` lists ChatGPT/Gemini-ready bundles with personas, knowledge files, browsing/deep-research flags, and release metadata. | Package format reference should include web bundles as a separate source contract. |
| Skill validator | `tools/skill-validator.md` defines deterministic rules for `SKILL.md`, path references, step files, sequence loading, templates, and variable references. | Validation rules must import these checks and treat validator output as package gate evidence. |
| Builder module | Builder package `bmad-builder` version `2.1.0` is a BMad expansion module for creating agents, workflows, and modules. | Builder Studio should wrap/import/rehearse Builder outputs instead of inventing an unrelated authoring model. |
| Package version is not module schema version | Builder `package.json` says `2.1.0` while `skills/module.yaml` says `module_version: 1.0.0`. | Persist `sourcePackageVersion`, `moduleVersion`, and runtime compatibility separately; never collapse them into one `version` field. |
| Builder skills | Builder includes `bmad-agent-builder`, `bmad-workflow-builder`, `bmad-module-builder`, `bmad-eval-runner`, and `bmad-bmb-setup`. | Builder Studio MVP should recognize these source skills and their command surfaces. |
| Builder output model | Builder docs define skills as the universal packaging format: agents, workflows, and modules are all skills with `SKILL.md` plus optional resources/scripts/templates. | Runtime should validate Builder outputs using the same package/skill parser, with extra gates for memory/autonomous agents. |
| Module packaging | Multi-skill modules use a setup skill with `assets/module.yaml` and `assets/module-help.csv`; standalone modules self-register with `assets/module-setup.md`, `assets/module.yaml`, and `assets/module-help.csv`. | Package import must support both module shapes. |
| Eval format | Builder evals use `evals.json`, `triggers.json`, setup overlays, fixtures, and headless prompts such as `Run headless.` | Builder validation needs an eval/rehearsal worker contract, not only static linting. |
| Eval isolation is not containment | `bmad-eval-runner/scripts/run_evals.py` creates a clean working directory and a reduced environment, then invokes an adapter with host `PATH` and selected credentials through `subprocess.run`; its own help states there is no container or terminal isolation. | Run Builder evals only inside an Airlock-approved isolated worker. A clean directory is a reproducibility feature, not a security boundary. |
| Two installed configuration profiles exist | Method CLI installs preserve per-module `_bmad/<module>/config.yaml` for skill compatibility while also generating canonical `_bmad/config.toml`, `config.user.toml`, custom TOML overrides, manifests, and `_config/bmad-help.csv`. Direct `bmad-bmb-setup` instead documents shared root `config.yaml`, `config.user.yaml`, and `module-help.csv`, then removes legacy module directories. | Model these as explicit `BmadInstallProfile` values `MethodCliV6` and `StandaloneBuilderSetupV2`. Normalize both into the existing `BmadPackageDescriptor` and `BmadConfigLayer` contracts; do not silently merge their files or assume they are the same layout. |
| Builder repository instructions have drifted | Builder `AGENTS.md` describes a removed `src/` YAML architecture and npm commands such as `npm test` and `validate:schemas` that are absent from `package.json`; the current implementation lives under `skills/` and has script-local Python tests. | Treat root contributor instructions as unverified for this snapshot. Build Sapphirus gates from executable package scripts and source fixtures, and fail source intake when declared commands or paths do not exist. |
| BMAD is a product/domain foundation, not an operations runtime | Method and Builder define workflows, skills, help/catalog metadata, authoring, validation, and evaluation. They do not provide Sapphirus owner scope, durable run state, Airlock policy, tenant authorization, isolated workers, evidence import, or rollback. | Preserve BMAD semantics as the product kernel while implementing operational authority in the Sapphirus runtime and governance planes. |

## Source-Aligned Plan Changes

1. BMAD Method and Builder are the product foundation from the first slice: Method supplies workflow/artifact/help semantics; Builder supplies authoring/quality/eval semantics.
2. Phase 0 must capture golden installed fixtures for both `MethodCliV6` and `StandaloneBuilderSetupV2`, then normalize them into versioned `BmadPackageDescriptor` and `BmadConfigLayer` records.
3. The first executable slice must run a sealed, pinned BMAD skill/workflow fixture and record package, skill, workflow-step, config, and artifact hashes. The full arbitrary package loader can still arrive later.
4. The BMAD Kernel must parse both source contracts and installed `_bmad` runtime artifacts without giving package code execution authority.
5. The Help Advisor must build its graph from `module-help.csv`, `module.yaml agents`, skill frontmatter, installed manifests, and artifact state.
6. Config resolution must distinguish installation profile, installer-managed config, user config, custom overrides, and legacy per-module YAML. Ambiguity is a blocking import finding.
7. Builder Studio v1 remains narrow, but it should align with actual Builder capabilities: build, import, convert, validate, rehearse, register.
8. Validation must include deterministic skill validation, module/package structural validation, security review, install rehearsal, invocation rehearsal, and optional eval runner evidence in an isolated worker.
9. Package format docs must include web bundles, plugin marketplace metadata, setup-skill module shape, standalone module shape, installed manifests, and separate package/module/runtime compatibility versions.
10. Source intake must verify that documented commands and paths exist before any upstream snapshot is promoted to a foundation fixture.

## Local Source Pointers

- BMAD Method package: `_source_review/BMAD-METHOD-main/BMAD-METHOD-main/package.json`
- Method installer: `_source_review/BMAD-METHOD-main/BMAD-METHOD-main/tools/installer/`
- Method module registry: `_source_review/BMAD-METHOD-main/BMAD-METHOD-main/bmad-modules.yaml`
- BMM module contract: `_source_review/BMAD-METHOD-main/BMAD-METHOD-main/src/bmm-skills/module.yaml`
- Core module contract: `_source_review/BMAD-METHOD-main/BMAD-METHOD-main/src/core-skills/module.yaml`
- Skill validator: `_source_review/BMAD-METHOD-main/BMAD-METHOD-main/tools/skill-validator.md`
- Web bundles: `_source_review/BMAD-METHOD-main/BMAD-METHOD-main/web-bundles/bundles.json`
- Builder package: `_source_review/bmad-builder-main/bmad-builder-main/package.json`
- Builder module contract: `_source_review/bmad-builder-main/bmad-builder-main/skills/module.yaml`
- Builder setup profile: `_source_review/bmad-builder-main/bmad-builder-main/skills/bmad-bmb-setup/`
- Builder eval runner: `_source_review/bmad-builder-main/bmad-builder-main/skills/bmad-eval-runner/scripts/run_evals.py`
- Builder reference docs: `_source_review/bmad-builder-main/bmad-builder-main/docs/reference/`
- Builder explanation docs: `_source_review/bmad-builder-main/bmad-builder-main/docs/explanation/`
