---
title: 'Builder Commands Reference'
description: Complete reference for all capabilities, modes, and paths available in the Agent Builder, Workflow Builder, and Module Builder
---

Reference for the three core BMad Builder skills: the Agent Builder (`bmad-agent-builder`), the Workflow Builder (`bmad-workflow-builder`), and the Module Builder (`bmad-module-builder`).

## Capabilities Overview

| Capability           | Menu Code | Agent Builder                         | Workflow Builder                                                                    |
| -------------------- | --------- | ------------------------------------- | ----------------------------------------------------------------------------------- |
| **Build Process**    | BP        | Build, edit, convert, or fix agents   | Build, edit, convert, or fix workflows and utilities                                |
| **Quality Optimize** | QO        | Validate and optimize existing agents | Validate and optimize existing workflows and utilities                              |
| **Convert**          | CW        | -                                     | Convert any skill to BMad-compliant, outcome-driven equivalent with comparison report |

Both capabilities support autonomous/headless mode via `--headless` / `-H` flags.

## Skill Naming

| Context        | Agent Pattern              | Workflow Pattern       |
| -------------- | -------------------------- | ---------------------- |
| **Standalone** | `agent-{name}`             | `{name}`               |
| **Module**     | `{modulecode}-agent-{name}`| `{modulecode}-{name}`  |

Names must be kebab-case and match the folder name. Agents should include `agent` in the name. For module-based skills, the user chooses the module code prefix during the build.

:::caution[Reserved Prefix]
The `bmad-` prefix is reserved for official BMad creations. User-built skills should not include it. If converting a skill that already has a `bmad-` prefix, retain it unless the user requests a rename.
:::

## Build Process (BP)

The core creative path. Six phases of conversational discovery take you from a rough idea to a complete, tested skill folder.

### Input Types

Both builders accept any of these as a starting point.

| Input                             | What Happens                                              |
| --------------------------------- | --------------------------------------------------------- |
| A rough idea or description       | Guided discovery from scratch                             |
| An existing BMad skill path       | Edit mode. Analyze what exists, determine what to change  |
| A non-BMad skill, tool, or code   | Convert to BMad-compliant structure                       |
| Documentation, API specs, or code | Extract intent and requirements automatically             |

### Interaction Modes

| Mode           | Behavior                                                                                     | Best For                                     |
| -------------- | -------------------------------------------------------------------------------------------- | -------------------------------------------- |
| **Guided**     | The builder walks through decisions, clarifies ambiguities, ensures completeness             | Production skills, first-time builders       |
| **YOLO**       | Brain-dump your idea; the builder guesses its way to a finished skill with minimal questions | Quick prototypes, experienced builders       |
| **Autonomous** | Fully headless; no interactive prompts, proceeds with safe defaults                          | CI/CD, batch processing, orchestrated builds |

### Build Phases

| Phase | Agent Builder                                                                                    | Workflow Builder                                                                                      |
| ----- | ------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------- |
| 1     | **Discover Intent**: understand the vision; detect agent type (stateless, memory, or autonomous) through natural questions | **Discover Intent**: understand the vision; accepts any input format                                  |
| 2     | **Capabilities Strategy**: internal commands, external skills, scripts; evolvable capability decision | **Classify Skill Type**: Simple Utility, Simple Workflow, or Complex Workflow; module membership      |
| 3     | **Gather Requirements**: identity, persona memory seeds, First Breath territories, PULSE behaviors, folder dominion | **Gather Requirements**: name, description, stages, config variables, output artifacts, dependencies  |
| 4     | **Draft & Refine**: present outline, iterate until ready                                         | **Draft & Refine**: present plan, clarify gaps, iterate until ready                                   |
| 5     | **Build**: generate skill structure per agent type, lint gate                                     | **Build**: generate skill structure, lint gate                                                        |
| 6     | **Summary**: present results, offer Quality Optimize                                             | **Summary**: present results, run unit tests if scripts exist, offer Quality Optimize                 |

### Agent Builder: Phase 1 Agent Type Detection

The builder determines the agent type through natural questions, not a menu:

| Question (asked naturally)                          | If No          | If Yes                     |
| --------------------------------------------------- | -------------- | -------------------------- |
| Does this agent need to remember between sessions?  | Stateless      | Memory or Autonomous       |
| Should the user be able to teach it new things?     | Fixed capabilities | Evolvable capabilities |
| Does it operate autonomously between sessions?      | Memory         | Autonomous                 |

For memory and autonomous agents, the builder also determines **relationship depth**: deep (calibration-style First Breath with open-ended discovery) or focused (configuration-style First Breath with guided questions).

### Agent Builder: Phase 2 Capabilities Strategy

Determines the mix of internal and external capabilities, plus script opportunities.

| Capability Type           | Description                                                                             |
| ------------------------- | --------------------------------------------------------------------------------------- |
| **Internal commands**     | Prompt-driven actions, each gets a file in `references/`                                |
| **External skills**       | Standalone skills the agent invokes by registered name                                  |
| **Scripts**               | Deterministic operations offloaded from the LLM (validation, data processing, file ops) |
| **Evolvable capabilities**| If enabled: user can teach the agent new capabilities over time via authoring reference  |

### Agent Builder: Phase 3 Requirements

Requirements differ by agent type. Stateless agents need identity and capabilities. Memory and autonomous agents need everything below.

**All agent types:**

| Requirement          | Description                                                                         |
| -------------------- | ----------------------------------------------------------------------------------- |
| **Identity**         | Who is this agent? Communication style, decision-making philosophy                  |
| **Capabilities**     | Internal commands, external skills, scripts                                         |
| **Folder dominion**  | Read boundaries, write boundaries, explicit deny zones                              |

**Memory and autonomous agents add:**

| Requirement                  | Description                                                                    |
| ---------------------------- | ------------------------------------------------------------------------------ |
| **Identity seed**            | 2-3 sentences of personality DNA for PERSONA.md                                |
| **Species-level mission**    | Domain-specific purpose statement for CREED.md                                 |
| **Core values**              | 3-5 values that guide behavior                                                 |
| **Standing orders**          | Surprise-and-delight + self-improvement, adapted to the domain with examples   |
| **CREED seeds**              | Philosophy, boundaries, anti-patterns (behavioral + operational)               |
| **BOND territories**         | Domain-specific areas to learn about the owner                                 |
| **First Breath territories** | Discovery questions beyond the universal set                                   |

**Autonomous agents add:**

| Requirement              | Description                                                                    |
| ------------------------ | ------------------------------------------------------------------------------ |
| **PULSE behaviors**      | Default wake behavior, domain-specific autonomous tasks                        |
| **Named task routing**   | Tasks invoked via `--pulse {task-name}`                                        |
| **Frequency & quiet hours** | How often to wake, when not to                                              |

### Workflow Builder: Phase 2-3 Details

**Skill type classification** determines template and structure.

| Type                 | Signals                                                                       | Structure                                                                           |
| -------------------- | ----------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| **Simple Utility**   | Composable building block, clear input/output, usually mostly script-driven   | Single SKILL.md, scripts folder                                                     |
| **Simple Workflow**  | Fits in one SKILL.md, a few sequential steps, optional autonomous             | SKILL.md with inline steps, optional prompts and resources                          |
| **Complex Workflow** | Multiple stages, branching prompt flows, progressive disclosure, long-running | SKILL.md for routing, `prompts/` for stage details, `resources/` for reference data |

**Workflow-specific requirements** gathered in Phase 3:

| Requirement             | Simple Utility | Simple Workflow | Complex Workflow                         |
| ----------------------- | -------------- | --------------- | ---------------------------------------- |
| **Input/output format** | Yes            | -               | -                                        |
| **Composability**       | Yes            | -               | -                                        |
| **Steps**               | -              | Numbered steps  | Named stages with progression conditions |
| **Headless mode**       | -              | Optional        | Optional                                 |
| **Config variables**    | -              | Core + custom   | Core + module-specific                   |
| **Module sequencing**   | Optional       | Optional        | Recommended                              |

### Build Output

The output structure depends on the agent type.

**Stateless agents:**

```
{skill-name}/
├── SKILL.md              # Full identity + persona + capabilities
├── references/           # Capability prompts
├── agents/               # Subagent definitions (if needed)
├── scripts/              # Deterministic scripts
│   └── tests/            # Unit tests for scripts
└── assets/               # Templates (if needed)
```

**Memory and autonomous agents:**

```
{skill-name}/
├── SKILL.md              # Lean bootloader (~30 lines of content)
├── references/
│   ├── first-breath.md   # First Breath conversation guide
│   ├── memory-guidance.md          # Session close and curation practices
│   ├── capability-authoring.md     # If evolvable capabilities enabled
│   └── {capability}.md             # Outcome-focused capability prompts
├── assets/               # Sanctum seed templates
│   ├── INDEX-template.md
│   ├── PERSONA-template.md
│   ├── CREED-template.md
│   ├── BOND-template.md
│   ├── MEMORY-template.md
│   ├── CAPABILITIES-template.md
│   └── PULSE-template.md          # Autonomous agents only
├── agents/               # Subagent definitions (if needed)
└── scripts/
    ├── wake.py           # Loads the whole sanctum in one pass on activation
    ├── init-sanctum.py   # Creates sanctum folder, copies templates, generates CAPABILITIES.md
    └── tests/
```

The seed templates contain real content from the discovery phases, not placeholders. The `wake.py` and init scripts are parameterized with the skill name, file lists, and evolvable flag.

**Workflow builder** output remains the same regardless of agent type:

```
{skill-name}/
├── SKILL.md              # Skill instructions
├── prompts/              # Stage prompts for complex workflows
├── resources/            # Reference data
├── agents/               # Subagent definitions for parallel processing
├── scripts/              # Deterministic scripts
│   └── tests/            # Unit tests for scripts
└── templates/            # Building blocks for generated output
```

### Lint Gate

Before completing the build, both builders run deterministic validation.

| Script                   | What It Checks                                                                            |
| ------------------------ | ----------------------------------------------------------------------------------------- |
| `scan-path-standards.py` | Path conventions: `{project-root}` for project-scope, `./` for same-folder references, bare paths for cross-directory skill-internal, no double-prefix        |
| `scan-scripts.py`        | Script portability, PEP 723 metadata, agentic design, unit test presence                  |

Critical issues block completion. Warnings are noted but don't block.

## Quality Optimize (QO)

Validation and optimization for existing skills. Runs deterministic lint scripts for instant structural checks and LLM scanner subagents for judgment-based analysis, all in parallel.

### Pre-Scan Checks

In interactive mode, the optimizer:

1. Checks for uncommitted changes and recommends committing first
2. Asks if the skill is currently working as expected

In autonomous mode, both checks are skipped and noted as warnings in the report.

### Scan Pipeline

The optimizer runs three tiers of analysis.

**Tier 1: Lint scripts** (deterministic, zero tokens, instant):

| Script                   | Focus                            |
| ------------------------ | -------------------------------- |
| `scan-path-standards.py` | Path convention violations       |
| `scan-scripts.py`        | Script portability and standards |

**Tier 2: Pre-pass scripts** (extract metrics for LLM scanners):

| Script                        | Agent Builder                       | Workflow Builder                |
| ----------------------------- | ----------------------------------- | ------------------------------- |
| Structure/integrity pre-pass  | `prepass-structure-capabilities.py` | `prepass-workflow-integrity.py` |
| Prompt metrics pre-pass       | `prepass-prompt-metrics.py`         | `prepass-prompt-metrics.py`     |
| Execution dependency pre-pass | `prepass-execution-deps.py`         | `prepass-execution-deps.py`     |

**Tier 3: LLM scanners** (judgment-based, run as parallel subagents):

| Scanner                       | Agent Builder Focus                                                        | Workflow Builder Focus                                                                       |
| ----------------------------- | -------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| **Structure / Integrity**     | Structure, capabilities, identity, memory setup, consistency               | Logical consistency, description quality, progression conditions, type-appropriate structure |
| **Prompt Craft**              | Token efficiency, anti-patterns, persona voice, overview quality           | Token efficiency, anti-patterns, overview quality, progressive disclosure                    |
| **Execution Efficiency**      | Parallelization, subagent delegation, memory loading, context optimization | Parallelization, subagent delegation, read avoidance, context optimization                   |
| **Cohesion**                  | Persona-capability alignment, gaps, redundancies                           | Stage flow coherence, purpose alignment, complexity appropriateness                          |
| **Enhancement Opportunities** | Script automation, autonomous potential, edge cases, delight               | Creative edge-case discovery, experience gaps, assumption auditing                           |

### Report Synthesis

After all scanners complete, the optimizer synthesizes results into a unified report saved to `{bmad_builder_reports}/{skill-name}/quality-scan/{timestamp}/`.

In interactive mode, it presents a summary with severity counts and offers next steps:

- Apply fixes directly
- Export checklist for manual fixes
- Discuss specific findings

In autonomous mode, it outputs structured JSON with severity counts and the report file path.

### Optimization Guidance

Not every suggestion should be applied. The optimizer communicates these decision rules:

- **Keep phrasing** that captures the intended voice. Leaner is not always better for persona-driven skills
- **Keep content** that adds clarity for the AI even if a human finds it obvious
- **Prefer scripting** for deterministic operations; **prefer prompting** for creative or judgment-based tasks
- **Reject changes** that flatten personality unless a neutral tone is explicitly wanted

## Convert (CW)

One-command conversion of any existing skill into a BMad-compliant, outcome-driven equivalent. Takes a non-conformant skill (bloated, poorly structured, or just not following BMad practices) and produces a clean version. Unlike the Build Process's edit/rebuild modes, `--convert` always runs headless and produces a visual comparison report.

### Usage

```
--convert <path-or-url> [-H]
```

The `--convert` flag implies headless mode. Accepts a local skill path or a URL (not limited to remote; local file paths work too).

### Process

| Step | What Happens |
| ---- | ------------ |
| **1. Capture** | Fetch or read the original skill, save a copy for comparison |
| **2. Rebuild** | Full headless rebuild from intent: extract what the skill achieves, apply BMad outcome-driven best practices  |
| **3. Report** | Measure both versions, categorize what changed and why, generate an interactive HTML comparison report |

### Comparison Report

The HTML report includes:

| Section | Content |
| ------- | ------- |
| **Hero banner** | Overall token reduction percentage |
| **Metrics table** | Lines, words, characters, sections, files, estimated tokens, with visual bars |
| **What changed** | Categorized differences (bloat removal, structural reorganization, best-practice alignment) with severity and examples |
| **What survived** | Content that earns its place: instructions the LLM wouldn't follow correctly without being told  |
| **Verdict** | One-sentence summary of the conversion |

Reports are saved to `{bmad_builder_reports}/convert-{skill-name}/`.

### When to Use Convert vs Build Process

| Scenario | Use |
| -------- | --- |
| You have any non-BMad-compliant skill and want it converted fast | `--convert` |
| You have a bloated skill and want a lean replacement with a comparison report | `--convert` |
| You want to interactively discuss what to change | Build Process (Edit mode) |
| You want to rethink a skill from scratch with full discovery | Build Process (Rebuild mode) |
| You want a detailed quality analysis without rebuilding | Quality Optimize |

## Module Builder

The Module Builder (`bmad-module-builder`) handles module-level planning, scaffolding, and validation. It operates at a higher level than the Agent and Workflow Builders; it orchestrates what those builders produce into a cohesive, installable module.

### Capabilities Overview

| Capability          | Menu Code | What It Does                                                                                                    |
| ------------------- | --------- | --------------------------------------------------------------------------------------------------------------- |
| **Ideate Module**   | IM        | Brainstorm and plan a module through creative facilitation                                                      |
| **Create Module**   | CM        | Package skills as an installable module: setup skill for multi-skill, self-registration for standalone           |
| **Validate Module** | VM        | Check structural integrity and entry quality for both multi-skill and standalone modules                        |

### Ideate Module (IM)

A brainstorming session that helps you plan your module from scratch. The builder acts as a creative collaborator, drawing out ideas, exploring possibilities, and guiding you toward the right architecture.

| Aspect          | Detail                                          |
| --------------- | ----------------------------------------------- |
| **Interaction** | Interactive only; no headless mode              |
| **Input**       | An idea or rough description                    |
| **Output**      | Plan document saved to `{bmad_builder_reports}` |

**What it covers:**

- Problem space exploration and creative brainstorming
- Architecture decision: single agent with capabilities vs. multiple skills vs. hybrid
- Standalone module or expansion of an existing module
- External dependencies (CLI tools, MCP servers)
- UI and visualization opportunities
- Setup skill extensions beyond configuration
- Per-skill capability definitions with help CSV metadata
- Configuration variables and sensible defaults

The plan document uses a resumable template with YAML frontmatter, so long brainstorming sessions survive context compaction.

**After ideation:** Build each planned skill using the Agent Builder (BA) or Workflow Builder (BW), then return to Create Module (CM) to scaffold the module.

### Create Module (CM)

Packages built skills as an installable BMad module. Auto-detects single-skill vs. multi-skill input and recommends the appropriate approach. Supports `--headless` / `-H`.

| Aspect          | Detail                                                                                      |
| --------------- | ------------------------------------------------------------------------------------------- |
| **Interaction** | Guided or headless                                                                          |
| **Input**       | Path to a skills folder or single skill (or SKILL.md file), optional plan document          |
| **Output**      | Setup skill for multi-skill modules, or self-registration files for standalone modules      |

**What it does:**

1. Reads the SKILL.md files to understand each skill
2. Detects single vs. multi-skill and confirms the packaging approach with the user
3. Collects module identity (name, code, description, version, greeting)
4. Defines help CSV entries: capabilities, menu codes, ordering, relationships
5. Captures configuration variables and external dependencies
6. Scaffolds the module infrastructure

**Multi-skill output:** A dedicated `{code}-setup/` folder with merge scripts, cleanup scripts, and a generic SKILL.md.

**Standalone output:** `assets/module-setup.md`, `assets/module.yaml`, and `assets/module-help.csv` embedded in the skill, plus merge scripts in `scripts/` and a `.claude-plugin/marketplace.json` for distribution. The skill's SKILL.md is updated to check for registration on activation.

### Validate Module (VM)

Verifies that a module's structure is complete and accurate. Auto-detects multi-skill modules (with setup skill) and standalone modules (with self-registration). Combines a deterministic validation script with LLM-based quality assessment.

| Aspect          | Detail                                                 |
| --------------- | ------------------------------------------------------ |
| **Interaction** | Interactive                                            |
| **Input**       | Path to the module's skills folder or single skill     |
| **Output**      | Validation report                                      |

**Structural checks** (script-driven):

| Check                  | What It Catches                                                                             |
| ---------------------- | ------------------------------------------------------------------------------------------- |
| Module structure       | Missing setup skill or standalone files (`module-setup.md`, merge scripts)                  |
| Coverage               | Skills without CSV entries, orphan entries for nonexistent skills                           |
| Menu codes             | Duplicate codes across the module                                                           |
| References             | Before/after fields pointing to nonexistent capabilities                                    |
| Required fields        | Missing skill name, display name, menu code, or description in CSV rows                     |
| module.yaml            | Missing code, name, or description                                                          |

**Quality assessment** (LLM-driven):

- Description accuracy: does each entry match what the skill actually does?
- Description quality: concise, action-oriented, specific, not overly verbose
- Completeness: are all distinct capabilities registered as separate rows?
- Ordering: do before/after relationships make sense?
- Menu codes: are they intuitive and memorable?

## Trigger Phrases

| Intent    | Phrases                                                 | Builder  | Route                             |
| --------- | ------------------------------------------------------- | -------- | --------------------------------- |
| Build new | "create/build/design an agent"                          | Agent    | `prompts/build-process.md`        |
| Build new | "create/build/design a workflow/skill/tool"             | Workflow | `prompts/build-process.md`        |
| Edit      | "edit/modify/update an agent"                           | Agent    | `prompts/build-process.md`        |
| Edit      | "edit/modify/update a workflow/skill"                   | Workflow | `prompts/build-process.md`        |
| Convert   | "convert this to a BMad agent"                          | Agent    | `prompts/build-process.md`        |
| Convert   | "convert this to a BMad skill"                          | Workflow | `prompts/build-process.md`        |
| Convert   | `--convert <path-or-url>`                               | Workflow | `./references/convert-process.md` |
| Optimize  | "quality check/validate/optimize/review agent"          | Agent    | `prompts/quality-optimizer.md`    |
| Optimize  | "quality check/validate/optimize/review workflow/skill" | Workflow | `prompts/quality-optimizer.md`    |
| Ideate    | "ideate module/plan a module/brainstorm a module"       | Module   | `./references/ideate-module.md`   |
| Create    | "create module/build a module/scaffold a module"        | Module   | `./references/create-module.md`   |
| Validate  | "validate module/check module"                          | Module   | `./references/validate-module.md` |
