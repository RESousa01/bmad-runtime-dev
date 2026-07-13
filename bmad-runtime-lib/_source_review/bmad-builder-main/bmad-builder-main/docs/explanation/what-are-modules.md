---
title: 'What Are BMad Modules?'
description: How agents and workflows combine into installable, configurable modules within the BMad ecosystem
---

BMad modules package agents and workflows into installable units with shared configuration and help system registration. A module can be a full suite of related skills or a single standalone skill that wants to be discoverable and configurable.

## Distribution: Plugins and Marketplaces

At the distribution level, a BMad module is a **plugin**: a package of skills with a `.claude-plugin/` manifest. How you structure it depends on what you're shipping:

| Structure           | When to Use                                                  | Manifest                                                  |
| ------------------- | ------------------------------------------------------------ | --------------------------------------------------------- |
| **Single plugin**   | One module (standalone or multi-skill)                       | `.claude-plugin/marketplace.json` with one plugin entry   |
| **Marketplace**     | A repo that ships multiple modules                           | `.claude-plugin/marketplace.json` with multiple plugin entries |

The `.claude-plugin/` convention originates from Claude Code, but the format works across multiple skills platforms. The BMad installer supports installing custom modules from any Git host (GitHub, GitLab, Bitbucket, self-hosted) or local file paths. See the [BMad Method install guide](https://docs.bmad-method.org/how-to/install-custom-modules/) for details.

The Module Builder generates the appropriate `marketplace.json` during the Create Module (CM) step - but you will want to verify it lists the proper relative paths to the skills you want to deliver with your module.

This also means you can include remote URL skills in your own module to combine them.

## What a Module Contains

| Component           | Multi-Skill Module                                      | Standalone Module                                          |
| ------------------- | ------------------------------------------------------- | ---------------------------------------------------------- |
| **Skills**          | Two or more agents/workflows                            | A single agent or workflow                                 |
| **Registration**    | Dedicated `{code}-setup` skill                          | Built into the skill itself (`assets/module-setup.md`)     |
| **module.yaml**     | In the setup skill's `assets/`                          | In the skill's own `assets/`                               |
| **module-help.csv** | In the setup skill's `assets/`                          | In the skill's own `assets/`                               |
| **Distribution**    | Plugin with multiple skill folders                      | Plugin with single skill folder + `marketplace.json`       |

For multi-skill modules, the setup skill is the glue; it registers all capabilities in one step. For standalone modules, the skill handles its own registration on first run or when the user passes `setup`/`configure`.

## Agent vs. Workflow vs. Both

The first architecture decision when planning a module is whether to use a single agent, multiple workflows, or a combination.

| Architecture                       | When It Fits                                                                 | Trade-offs                                                                                                    |
| ---------------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| **Single agent with capabilities** | All capabilities serve the same user journey and benefit from shared context | Simpler to maintain, better memory continuity, seamless UX. Can feel monolithic if capabilities are unrelated |
| **Multiple workflows**             | Capabilities serve different user journeys or require different tools        | Each workflow is focused and composable. Users switch between skills explicitly                               |
| **Hybrid**                         | Some capabilities need persistent persona/memory while others are procedural | Best of both worlds but more skills to build and maintain                                                     |

:::tip[Agent-First Thinking]
Many users default to building multiple single-purpose agents. Consider whether one agent with rich internal capabilities and routing would serve users better. A single agent accumulates context, maintains memory across interactions, and provides a smoother experience.
:::

## Multi-Agent Modules and Memory

Modules with multiple agents introduce a memory architecture decision. BMad agents exist on a spectrum from stateless (no memory) through memory agents (personal sanctum) to autonomous agents (sanctum + PULSE). In a multi-agent module, you choose both the agent type for each skill and whether agents should share memory across the module.

| Pattern                              | When It Fits                                                                            |
| ------------------------------------ | --------------------------------------------------------------------------------------- |
| **Personal memory only**                | Agents have distinct domains with minimal overlap                                       |
| **Personal + shared module memory**     | Agents have their own context but also learn shared things about the user or project    |
| **Shared memory only**                  | All agents serve the same domain; consider whether a single agent is the better design |
| **Mixed types**                         | Some agents need memory (coaches, companions) while others are stateless (formatters, validators) |

**Example:** A social creative module with a podcast expert, a viral video expert, and a blog expert. Each memory agent maintains its own sanctum with what it has done with the user (episode topics, video formats, blog themes). But they all also contribute to a module-level memory folder that captures the user's communication style, favorite catchphrases, content preferences, and brand voice.

Each agent should still be self-contained with its own capabilities, even if this means duplicating some common functionality. A podcast expert that can independently handle a full session without needing the blog expert is better than one that depends on shared state to function.

See **[What Are BMad Agents](/explanation/what-are-bmad-agents.md)** for the three agent types, and **[Agent Memory and Personalization](/explanation/agent-memory-and-personalization.md)** for details on how the sanctum architecture works.

## Standalone vs. Expansion Modules

| Type           | Description                                                                                                               |
| -------------- | ------------------------------------------------------------------------------------------------------------------------- |
| **Standalone** | Provides complete, independent value. Does not depend on another module being installed                                   |
| **Expansion**  | Extends an existing module with new capabilities. Should still provide utility even if the parent module is not installed |

Expansion modules can reference the parent module's capabilities in their help CSV ordering (before/after fields). This lets a new capability slot into the parent module's natural workflow sequence.

Even expansion modules should be designed to work independently. The parent module being absent should degrade gracefully, not break the expansion.

## Configuration and Registration

Modules register with a project through three files in `{project-root}/_bmad/`:

| File               | Purpose                                                                |
| ------------------ | ---------------------------------------------------------------------- |
| `config.yaml`      | Shared settings committed to git, module section keyed by module code |
| `config.user.yaml` | Personal settings (gitignored), user name, language preferences       |
| `module-help.csv`  | Capability registry, one row per action users can discover            |

Registration is what makes a module visible to `bmad-help`. Without it, the help system cannot discover, recommend, or track completion of the module's capabilities.

Not every module needs configuration. If skills work with sensible defaults, registration can focus purely on help entries. See **[Module Configuration](/explanation/module-configuration.md)** for details on when configuration adds value and how the help CSV columns work.

## External Dependencies

Some modules depend on tools outside the BMad ecosystem.

| Dependency Type  | Examples                                             |
| ---------------- | ---------------------------------------------------- |
| **CLI tools**    | `docker`, `terraform`, `ffmpeg`                      |
| **MCP servers**  | Custom or third-party Model Context Protocol servers |
| **Web services** | APIs that require credentials or configuration       |

When a module has external dependencies, the setup skill should check for their presence and guide users through installation or configuration.

## UI and Visualization

Modules can include user interfaces: dashboards, progress views, interactive visualizations, or even full web applications. A UI skill might show shared progress across the module's capabilities, provide a visual map of how skills relate, or offer an interactive way to navigate the module's features.

Not every module needs a UI. But for complex modules with many capabilities, a visual layer makes the experience much more accessible.

## Building a Module

The Module Builder (`bmad-module-builder`) provides three capabilities for the module lifecycle:

1. **Ideate Module (IM)**: Brainstorm and plan through creative facilitation
2. **Create Module (CM)**: Package skills as an installable module. Detects whether you have a folder of skills (generates a setup skill) or a single skill (embeds self-registration directly)
3. **Validate Module (VM)**: Verify structural integrity and entry quality for both multi-skill and standalone modules

See the **[Builder Commands Reference](/reference/builder-commands.md)** for detailed documentation on each capability.
