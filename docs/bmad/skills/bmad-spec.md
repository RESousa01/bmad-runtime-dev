# bmad-spec

- **rowId**: `method:src/bmm-skills/2-plan-workflows/bmad-spec/SKILL.md`
- **canonicalId**: `bmm:bmad-spec`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/2-plan-workflows/bmad-spec/SKILL.md`
- **runtime projection**: `packages/bmad-foundation/runtime/method/6.10.0/spec.instructions.md`
- **normalized envelope**: `packages/bmad-foundation/normalized/bmad-spec.package.json`

## What it does

Distills reviewed intent into a governed Spec workspace. Every run derives
`SPEC.md` and `.memlog.md`; optional companion files retain load-bearing detail,
and an interactive Story Breakdown may add `stories.yaml`. Create, update,
no-change review, missing-input, and missing-slug behavior comes from the
pinned source rather than from caller-invented actions.

## Runtime contract

- **Actions**: none. The canonical catalog keeps `action: null`.
- **Modes**: interactive and headless share the same required artifact
  semantics. Headless never performs the interactive-only story breakdown.
- **Output location**: `output_folder`, defaulting to
  `{output_folder}/specs/spec-{slug}/`.
- **Artifact kinds**: document and file set. `SPEC.md` and `.memlog.md` are
  required; companions and interactive `stories.yaml` are optional.
- **Lifecycle**: activation, intent resolution, distillation, coherence
  validation, preservation validation, completion. The generic Method session
  service owns create, bind, review, advance, complete, cancel, fail, retry,
  repair, restart, resume, and stale-source rebind.
- **Authority**: the adapter declares typed file-read, file-write, process,
  subagent, and external-handoff intents. These declarations grant no effects;
  the host must prepare and approve them. Network and browser authority remain
  distinct and are not requested.

## Field-by-field semantic adaptation

| Source field or behavior | Sapphirus projection | Reason |
| --- | --- | --- |
| `SKILL.md` activation and operation | Sealed managed instructions plus ordered workflow stages | Source prose becomes inert data consumed by the generic Method runtime. |
| `customize.toml` | Hash-pinned resource | Customization remains reviewable input and receives no direct authority. |
| `assets/headless-schemas.md` | Hash-pinned resource and bounded headless response contract | Keeps source parity without treating a schema as executable code. |
| `assets/spec-template.md` | Hash-pinned resource and required `SPEC.md` expectation | Preserves the five-field Spec kernel. |
| `assets/stories-schema.md` | Hash-pinned resource and optional interactive artifact | Preserves the interactive-only Story Breakdown boundary. |
| Direct workspace reads/writes in source prose | Typed effect intents prepared by the host | All effects remain fail-closed behind the authority boundary. |
| Source process/subagent/handoff language | Separate typed intents with unavailable host replacements until approved | Prevents source content from self-granting process or handoff authority. |
| Source create/update detection | Durable Method-session context and reviewed memory-log state | Keeps recovery, retry, repair, restart, resume, and rebind deterministic. |

## Compatibility decision

**adapted.** The source semantics are preserved, but execution is deliberately
translated into the shared Sapphirus Method contract and durable session
lifecycle. No compatibility aliases are introduced for this capability.
