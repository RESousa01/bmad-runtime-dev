---
title: 'Module Configuration and the Setup Skill'
description: How BMad modules handle user configuration through a setup skill, when to use configuration vs. alternatives, and how to register with the help system
---

BMad modules register their capabilities with the help system and optionally collect user preferences. Multi-skill modules use a dedicated **setup skill** for this. Single-skill standalone modules handle registration themselves on first run.

When you create your own module, you can either add a configuration skill or embed the feature in every skill following the standalone pattern. For modules with more than 1-2 skills, a setup skill is the better choice.

## When You Need Configuration

Most modules should not need configuration at all. Before adding configurable values, consider whether a simpler alternative exists.

| Approach              | When to Use                                                                                                                                               |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Sensible defaults** | The variable has one clearly correct answer for most users that could be overridden or updated by the specific skill that needs it the first time it runs |
| **Agent memory**      | Your module follows the agent pattern and the agent can learn preferences through conversation                                                            |
| **Configuration**     | The value genuinely varies across projects and cannot be inferred at runtime                                                                              |

:::tip[Standalone Skills]
If you are building a single standalone agent or workflow, you do not need a separate setup skill. The Module Builder can package it as a **standalone self-registering module** where the registration logic is embedded directly in the skill via an `assets/module-setup.md` reference file, and runs on first activation or when the user passes `setup`/`configure`.
:::

## Configuration vs Customization

Module configuration (this doc) and per-skill customization (`customize.toml`) are different surfaces with different jobs. Configuration is about install-time answers: paths, language, team preferences, per-module install answers, and the agent roster. You still author `module.yaml` as the source of truth; at install the installer flows module-level answers and the `agents:` roster into `_bmad/config.toml` (and `config.user.toml` for user-scoped answers) at the project root, where many skills consume them. Customization is about per-skill behavior overrides: activation hooks, persistent facts, swappable templates. It lives in `_bmad/custom/{skill-name}.toml` and is scoped to one skill.

Use configuration when the value is cross-cutting (every skill needs to know the output folder). Use customization when the value shapes one skill's behavior (this workflow's brief template). Some values legitimately fit both surfaces; the [End-User Customization Guide](https://docs.bmad-method.org/how-to/customize-bmad/) includes a decision table for that case. For the author-side decision about whether to expose customization at all, see [Customization for Authors](/explanation/customization-for-authors.md).

## What Module Registration Does

Module registration serves two purposes:

| Purpose               | What Happens                                                                              |
| --------------------- | ----------------------------------------------------------------------------------------- |
| **Configuration**     | Collects user preferences and writes them to shared config files                          |
| **Help registration** | Adds the module's capabilities to the project-wide help system so users can discover them |

### Why Register with the Help System?

The `bmad-help` skill reads `module-help.csv` to understand what capabilities are available, detect which ones have been completed (by checking output locations for artifacts), and recommend next steps based on the dependency graph. Without registration, `bmad-help` cannot discover or recommend your module's capabilities beyond what it knows basically from skill headers. The help system provides richer detail: arguments, relationships to other skills, inputs and outputs, and any other authored metadata. If a skill has multiple capabilities, each one gets its own help entry.

### Two Registration Paths

| Path                  | When to Use                                               | How It Works                                                                    |
| --------------------- | --------------------------------------------------------- | ------------------------------------------------------------------------------- |
| **Setup skill**       | Multi-skill modules (2+ skills)                           | A dedicated `{code}-setup` skill handles registration for all skills            |
| **Self-registration** | Single-skill standalone modules                           | The skill itself registers on first run or when user passes `setup`/`configure` |

The Module Builder detects which path to use based on what you give it: a folder of skills triggers the setup skill approach, a single skill triggers the standalone approach.

## Configuration Files

Setup skills write to three files in `{project-root}/_bmad/`:

| File               | Scope                    | Contains                                                                                        |
| ------------------ | ------------------------ | ----------------------------------------------------------------------------------------------- |
| `config.yaml`      | Shared, committed to git | Core settings at root level, plus a section per module with metadata and module-specific values |
| `config.user.yaml` | Personal, gitignored     | User-only settings like `user_name` and `communication_language`                                |
| `module-help.csv`  | Shared, committed to git | One row per capability the module exposes                                                       |

Core settings (like `output_folder` and `document_output_language`) live at the root of `config.yaml` and are shared across all modules. Each module also gets its own section keyed by its module code.

## The module.yaml File

Each module declares its identity and configurable variables in an `assets/module.yaml` file. For multi-skill modules, this lives inside the setup skill. For standalone modules, it lives in the skill's own `assets/` folder. This file drives both the prompts shown to the user and the values written to config.

```yaml
code: mymod
name: 'My Module'
description: 'What this module does'
module_version: 1.0.0
default_selected: false
module_greeting: >
  Welcome message shown after setup completes.

my_output_folder:
  prompt: 'Where should output be saved?'
  default: '{project-root}/_bmad-output/my-module'
  result: '{project-root}/{value}'
```

Variables with a `prompt` field are presented to the user during setup. The `default` value is used when the user accepts defaults. Adding `user_setting: true` to a variable routes it to `config.user.yaml` instead of the shared config.

:::caution[Literal Token]
`{project-root}` is a literal token in config values. Never substitute it with an actual path. It signals to the consuming tool that the value is relative to the project root.
:::

## Help Registration Without Configuration

You may not need any configurable values but still want to register your module with the help system. Registration is still worthwhile when:

- The skill description in SKILL.md frontmatter cannot fully convey what the module offers while staying concise
- You want to express capability sequencing, phase constraints, or other metadata the CSV supports
- An agent has many internal capabilities that users should be able to discover
- Your module has more than about three distinct things it can do

For simpler cases, these alternatives are often sufficient:

| Alternative                   | What It Provides                                                                                                                         |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| **SKILL.md overview section** | A concise summary at the top of the skill body; the `--help` system scans this section to present user-facing help, so keep it succinct |
| **Script header comments**    | Describe purpose, usage, and flags at the top of each script                                                                             |

If these cover your discoverability needs, you can skip the setup skill entirely.

## The module-help.csv File

The CSV registers the module's capabilities with the help system. Each row describes one capability that users can discover and invoke. The file has 13 columns:

```csv
module,skill,display-name,menu-code,description,action,args,phase,preceded-by,followed-by,required,output-location,outputs
```

### Column Guide

| Column              | Purpose                                                                                                                                      |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| **module**          | Module display name. Groups entries in help output                                                                                          |
| **skill**           | Skill folder name (e.g., `bmad-agent-builder`); must match the actual directory name                                                        |
| **display-name**    | User-facing label shown in help menus (e.g., "Build an Agent")                                                                               |
| **menu-code**       | 1-3 letter shortcode displayed as `[CODE]` in help, unique across the module, intuitive mnemonic                                            |
| **description**     | What this capability does. Concise, action-oriented, specific enough for `bmad-help` to route correctly                                     |
| **action**          | Action name within the skill. Distinguishes capabilities when one skill exposes multiple (e.g., `build-process`, `quality-optimizer`)        |
| **args**            | Arguments the capability accepts (e.g., `[-H] [path]`), shown in help output                                                               |
| **phase**           | When the capability is available: `anytime` or a workflow phase like `1-analysis`, `2-planning`                                             |
| **preceded-by**     | Capabilities that should complete before this one (this capability is preceded by them): format `skill-name:action`, comma-separated for multiple |
| **followed-by**     | Capabilities that should run after this one (this capability is followed by them), same format as `preceded-by`                              |
| **required**        | `true` if this is a blocking gate for phase progression, `false` otherwise                                                                   |
| **output-location** | Config variable name (e.g., `output_folder`, `bmad_builder_reports`); `bmad-help` resolves from config to scan for completion artifacts     |
| **outputs**         | File patterns `bmad-help` looks for in the output location to detect completion (e.g., "quality report", "agent skill")                      |

### How bmad-help Uses These Entries

The `preceded-by`/`followed-by` columns create a **dependency graph** that `bmad-help` walks to recommend next steps. `required=true` entries are blocking gates; `bmad-help` will not suggest later-phase capabilities until required gates pass. The `output-location` and `outputs` columns enable **completion detection**: `bmad-help` scans those paths for matching artifacts to determine what's been done.

### Example Entry

```csv
module,skill,display-name,menu-code,description,action,args,phase,preceded-by,followed-by,required,output-location,outputs
BMad Builder,bmad-agent-builder,Build an Agent,BA,"Create, edit, convert, or fix an agent skill.",build-process,"[-H] [description | path]",anytime,,bmad-agent-builder:quality-analysis,false,output_folder,agent skill
```

During registration, these rows are merged into the project-wide `_bmad/module-help.csv`, replacing any existing rows for this module (anti-zombie pattern).

## Anti-Zombie Pattern

Both merge scripts use an anti-zombie pattern: before writing new values for a module, they remove all existing entries for that module's code. This prevents stale configuration or help entries from persisting across module updates. Running setup a second time is always safe.

## Legacy Directory Cleanup

After config data is migrated and individual files are cleaned up by the merge scripts, a separate cleanup step removes the installer's per-module directory trees from `_bmad/`. These directories contain skill files that are already installed in the tool's skills directory. They are redundant once the config has been consolidated.

Before removing any directory, the cleanup script verifies that every skill it contains exists at the installed location. Directories without skills (like `_config/`) are removed directly. The script is idempotent; running setup again after cleanup is safe.

## Design Guidance

Configuration is for **basic, project-level settings**: output folders, language preferences, feature toggles. Keep the number of configurable values small.

| Pattern                | Configuration Role                                                                                              |
| ---------------------- | --------------------------------------------------------------------------------------------------------------- |
| **Agent pattern**      | Prefer agent memory for per-user preferences. Use config only for values that must be shared across the project |
| **Workflow pattern**   | Use config for output locations and behavior switches that vary across projects                                 |
| **Skill-only pattern** | Use config sparingly. If the skill works with sensible defaults, skip config entirely                           |

Extensive workflow customization (step overrides, conditional branching, template selection) is a separate concern and will be covered in a dedicated document.

## Creating a Module with the Module Builder

The **Module Builder** (`bmad-module-builder`) automates module creation. It offers three capabilities:

| Capability          | Menu Code | What It Does                                                                            |
| ------------------- | --------- | --------------------------------------------------------------------------------------- |
| **Ideate Module**   | IM        | Brainstorm and plan a module through facilitative discovery; produces a plan document  |
| **Create Module**   | CM        | Package skills as an installable BMad module (setup skill or standalone self-registering)|
| **Validate Module** | VM        | Check that a module's structure is complete, accurate, and properly registered           |

**For a folder of skills (multi-skill module):**

1. Run **Ideate Module (IM)** to brainstorm and plan
2. Build each skill using the **Agent Builder (BA)** or **Workflow Builder (BW)**
3. Run **Create Module (CM)**. It generates a dedicated `-setup` skill with `module.yaml`, `module-help.csv`, and merge scripts
4. Run **Validate Module (VM)** to verify everything is wired correctly

**For a single skill (standalone module):**

1. Build the skill using the **Agent Builder (BA)** or **Workflow Builder (BW)**
2. Run **Create Module (CM)** with the skill path. It embeds self-registration directly into the skill (`assets/module-setup.md`, `assets/module.yaml`, `assets/module-help.csv`) and generates a `marketplace.json` for distribution
3. Run **Validate Module (VM)** to verify

The Module Builder auto-detects single vs. multi-skill input and recommends the appropriate approach.

See **[What Are Modules](/explanation/what-are-modules.md)** for concepts and architecture decisions, or the **[Builder Commands Reference](/reference/builder-commands.md)** for detailed capability documentation.
