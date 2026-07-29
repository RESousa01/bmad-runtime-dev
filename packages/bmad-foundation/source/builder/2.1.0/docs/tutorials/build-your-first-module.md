---
title: 'Build Your First Module'
description: Create a complete BMad module from idea to installable package using the Module Builder
---

This tutorial takes you from an initial idea to a working, installable BMad module with help registration and configuration.

## What You'll Learn

- Planning a module with the Ideate Module (IM) capability
- Choosing between a single agent and multiple workflows
- Building individual skills with the Agent and Workflow Builders
- Scaffolding a setup skill with Create Module (CM)
- Validating your module with Validate Module (VM)

:::note[Prerequisites]

- BMad Builder module registered in your project (run `bmad-bmb-setup` on first use)
- Basic understanding of agents and workflows (see **[What Are Agents](/explanation/what-are-bmad-agents.md)** and **[What Are Workflows](/explanation/what-are-workflows.md)**)
:::

:::tip[Quick Path]
Already have your skills built? Skip to **Step 3: Scaffold the Module** to package them. Just need to validate an existing module? Jump to **Step 4: Validate**.
:::

## Understanding Modules

A BMad module bundles skills so they're discoverable and configurable. The Module Builder offers two approaches depending on what you're building:

| Approach              | When to Use                                  | What Gets Generated                                             |
| --------------------- | -------------------------------------------- | --------------------------------------------------------------- |
| **Setup skill**       | Folder of 2+ skills                          | Dedicated `{code}-setup` skill with config and help assets      |
| **Self-registration** | Single standalone skill                      | Registration embedded in the skill's own `assets/` folder       |

Both produce the same registration artifacts: `module.yaml` (identity and config variables) and `module-help.csv` (capability entries), which register with `bmad-help`.

See **[What Are Modules](/explanation/what-are-modules.md)** for the architecture behind these choices.

## Step 1: Plan Your Module

Start with the Ideate Module capability.

:::note[Example]
**You:** "I want to ideate a module"

**Builder:** Starts a brainstorming session to explore the module's purpose, audience, and capability structure.
:::

The ideation session covers:

| Topic             | What You'll Decide                                                        |
| ----------------- | ------------------------------------------------------------------------- |
| **Vision**        | Problem space, target users, core value                                   |
| **Architecture**  | Single agent, multiple workflows, or hybrid                               |
| **Agent types**   | For each agent: stateless, memory, or autonomous (see [What Are Agents](/explanation/what-are-bmad-agents.md)) |
| **Memory**        | For multi-agent modules: personal memory, shared module memory, or both |
| **Module type**   | Standalone or expansion of another module                                 |
| **Skills**        | Each planned skill's purpose, capabilities, and relationships             |
| **Configuration** | Custom install questions and variables                                    |
| **Dependencies**  | External CLI tools, MCP servers, web services                             |

The output is a **plan document** saved to your reports folder. You'll reference it when building each skill.

## Step 2: Build Your Skills

Now build each skill individually.

| Skill Type          | Builder          | Menu Code |
| ------------------- | ---------------- | --------- |
| Agent               | Agent Builder    | BA        |
| Workflow or utility | Workflow Builder | BW        |

Share the plan document as context when building each skill so the builder knows how it fits into the module. For agents, the builder will detect the right type (stateless, memory, or autonomous) through conversational discovery and adapt the build process accordingly.

:::caution[Build Before Packaging]
Build and test each skill before scaffolding the module. The Create Module step reads your finished skills to generate accurate help entries.
:::

## Step 3: Scaffold the Module

Run Create Module (CM) to package your finished skills.

:::note[Example]
**You:** "I want to create a module" or provide the path to your skills folder (or a single skill).

**Builder:** Reads your skills, detects whether this is a multi-skill or single-skill module, confirms the approach, and scaffolds the output.
:::

### Multi-skill modules

The builder generates a dedicated setup skill:

```
your-skills-folder/
├── {code}-setup/                # Generated setup skill
│   ├── SKILL.md                 # Setup instructions
│   ├── scripts/                 # Config merge and cleanup scripts
│   │   ├── merge-config.py
│   │   ├── merge-help-csv.py
│   │   └── cleanup-legacy.py
│   └── assets/
│       ├── module.yaml          # Module identity and config vars
│       └── module-help.csv      # Capability entries
├── your-agent-skill/
├── your-workflow-skill/
└── ...
```

### Standalone modules

The builder embeds registration into the skill itself:

```
your-skill/
├── SKILL.md                     # Updated with registration check
├── assets/
│   ├── module-setup.md          # Self-registration reference
│   ├── module.yaml              # Module identity and config vars
│   └── module-help.csv          # Capability entries
├── scripts/
│   ├── merge-config.py          # Config merge script
│   └── merge-help-csv.py        # Help CSV merge script
└── ...
```

A `.claude-plugin/marketplace.json` is also generated at the parent level for distribution.

## Step 4: Validate

Run Validate Module (VM) to check for structural and quality issues.

:::note[Example]
**You:** "Validate my module at ./my-skills-folder"

**Builder:** Runs structural and quality checks, then reports findings.
:::

| Check Type     | What It Catches                                                        |
| -------------- | ---------------------------------------------------------------------- |
| **Structural** | Missing files, orphan entries, duplicate menu codes, broken references |
| **Quality**    | Inaccurate descriptions, missing capabilities, poor entry quality      |

Fix any findings and re-validate until clean.

## What You've Built

Your module is ready to distribute. Multi-skill modules install through the setup skill; standalone modules self-register on first run. Either way, capabilities appear in `bmad-help` and configuration is persisted automatically.

## Quick Reference

| Capability       | Menu Code | When to Use                                        |
| ---------------- | --------- | -------------------------------------------------- |
| Ideate Module    | IM        | Planning a new module from scratch                    |
| Build an Agent   | BA        | Creating an agent skill                               |
| Build a Workflow | BW        | Creating a workflow or utility skill                   |
| Create Module    | CM        | Packaging skills into an installable module            |
| Validate Module  | VM        | Checking completeness and accuracy                     |

## Common Questions

### Do I need to ideate before creating?

No. If you already know what your module should contain, skip straight to Create Module (CM). Ideation helps when you're still shaping the concept.

### Can I add skills to a module later?

Yes. Build the new skill and re-run Create Module (CM) on the folder. The anti-zombie pattern ensures the existing setup skill is replaced cleanly.

### What if my module only has one skill?

The Module Builder handles this automatically. Give it a single skill and it recommends the **standalone self-registering** approach, where registration embeds directly in the skill and triggers on first run or when the user passes `setup`/`configure`.

### Can my module extend another module?

Yes. Tell the builder during ideation or creation that your module is an expansion. Your help CSV entries can reference the parent module's capabilities in their before/after ordering fields.

## Getting Help

- **[What Are Modules](/explanation/what-are-modules.md)**: Concepts and architecture
- **[Module Configuration](/explanation/module-configuration.md)**: Setup skill internals and config patterns
- **[Builder Commands Reference](/reference/builder-commands.md)**: All builder capabilities
- **[Discord](https://discord.gg/gk8jAdXWmj)**: Community support

:::tip[Key Takeaway]
The workflow is IM, then BA/BW for each skill, then CM to package, then VM to verify. Single-skill modules need no extra setup infrastructure.
:::
