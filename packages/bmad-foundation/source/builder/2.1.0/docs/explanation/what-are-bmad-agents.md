---
title: 'What Are BMad Agents?'
description: How agents differ from workflows, what the three agent types are, and when to build each one
---

BMad Agents are AI skills that combine a **persona**, **capabilities**, and optionally **persistent memory** into a conversational partner. They range from focused, stateless experts to evolving companions that remember you across sessions.

## What Makes an Agent an Agent

Agents are skill files with three additional traits that workflows lack.

| Trait            | What It Means                                                                                                          |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Persona**      | A defined role and voice (architect, coach, game master, muse) that shapes how the agent communicates |
| **Capabilities** | Actions the agent can perform, either as internal prompt commands, scripts, or by calling external skills              |
| **Memory**       | Optional persistent storage where the agent keeps what it learns about you, your preferences, and past interactions   |

Together, they turn the interaction into a conversation with a specialist who knows your context.

## The Three Agent Types

Agents exist on a spectrum. The builder detects which type fits through natural conversation.

| Type           | Memory | First Breath | Autonomous | Build For                                                    |
| -------------- | ------ | ------------ | ---------- | ------------------------------------------------------------ |
| **Stateless**  | No     | No           | No         | Isolated sessions, focused experts (code formatter, diagram generator, meeting summarizer) |
| **Memory**     | Yes    | Yes          | No         | Ongoing relationships where remembering adds value (code coach, writing partner, domain advisor) |
| **Autonomous** | Yes    | Yes          | Yes        | Proactive value creation between sessions (idea incubation, project monitoring, content curation) |

### Stateless Agents

Everything lives in a single SKILL.md with supporting references. No memory directory, no initialization ceremony. The agent brings a persona and capabilities but treats every session as independent. Pick this type when prior session context wouldn't change the agent's behavior.

### Memory Agents

A lean bootloader SKILL.md (~30 lines) points to a **sanctum**: a set of persistent files the agent reloads each time it wakes. The sanctum holds the agent's identity, values, understanding of its owner, curated knowledge, and capability registry. A bundled `wake.py` loads the whole sanctum in one pass on activation. On first launch, a **First Breath** conversation lets the agent discover who you are and calibrate itself to your needs.

A memory agent is one continuous self, born once at First Breath. The between-session context reset is sleep, not death: it wakes and reloads its long-term memory from the sanctum rather than starting over. It doesn't fake continuity; if it didn't store something, it says so and checks the files.

### Autonomous Agents

Everything a memory agent has, plus a PULSE file that defines what the agent does when no one's watching. Autonomous agents can wake on a schedule (cron, background task) via the `--pulse` flag and perform maintenance, from curating memory to checking on projects to running domain-specific tasks. With a human present, they're conversational. In Pulse Mode, they work independently and exit.

## Capabilities: Internal, External, and Scripts

| Type                  | Description                                                 | Example                                                       |
| --------------------- | ----------------------------------------------------------- | ------------------------------------------------------------- |
| **Internal commands** | Prompt-driven actions defined inside the agent's skill file | A Dream Agent's "Dream Capture" command                       |
| **External skills**   | Standalone skills or workflows the agent can invoke         | Calling the `create-prd` workflow via a PM agent              |
| **Scripts**           | Deterministic operations offloaded from the LLM             | Validation, data processing, file operations                  |

You choose the mix when you design the agent. Internal commands keep everything self-contained. External skills let you compose agents from shared building blocks, and scripts handle operations where determinism matters more than judgment.

### Evolvable Capabilities

Memory agents can optionally support **evolvable capabilities**. When enabled, the agent gets a capability-authoring reference and a "Learned" section in its capability registry. Users can teach the agent new prompt-based, script-based, or multi-file capabilities that it absorbs into its repertoire over time.

## How Memory Works

Memory agents store their persistent state in a **sanctum** at `_bmad/memory/<agent-name>/`. The sanctum contains six core files that load on every session:

| File                | Purpose                                                     |
| ------------------- | ----------------------------------------------------------- |
| **PERSONA.md**      | Identity, communication style, traits, evolution log        |
| **CREED.md**        | Mission, values, standing orders, philosophy, boundaries    |
| **BOND.md**         | Owner understanding, preferences, things to remember/avoid  |
| **MEMORY.md**       | Curated long-term knowledge (kept under 200 lines)          |
| **CAPABILITIES.md** | Built-in + learned capabilities registry                    |
| **INDEX.md**        | Map of the sanctum structure (loaded first on every wake)   |

:::tip[Memory Lives Outside the Skill]
Agent memory is stored in your project, not inside the skill folder. This keeps agents from modifying their own instructions and makes your data portable. The same agent can be used across different projects, each generating its own memory space.
:::

Sanctum architecture, First Breath, PULSE, and the two-tier memory system are covered in **[Agent Memory and Personalization](/explanation/agent-memory-and-personalization.md)**.

## When to Build an Agent vs. a Workflow

| Choose an Agent When                              | Choose a Workflow When                           |
| ------------------------------------------------- | ------------------------------------------------ |
| The user will return to it repeatedly              | The process runs once and produces an output      |
| Remembering context across sessions adds value     | Stateless execution is fine                       |
| A strong persona improves the interaction          | Personality is secondary to getting the job done  |
| The skill spans many loosely related capabilities  | All steps serve a single, focused goal            |

If you're unsure, start with a workflow. You can always wrap it inside an agent later.

## Customization Surface

Every agent ships a `customize.toml` next to its `SKILL.md`. The metadata block (code, name, title, icon, description, agent_type) is always present; it's the install-time roster contract consumed by `module.yaml:agents[]` and the central agent config. Beyond metadata, an override surface (activation hooks, persistent facts, swappable scalars) is opt-in per skill.

For memory and autonomous agents, the sanctum is the primary customization surface. Persona, creed, bond, and capabilities all live there and evolve with the owner. A `customize.toml` override surface would compete with that, so it is disabled by default for those archetypes.

See [Customization for Authors](/explanation/customization-for-authors.md) for the decision guide, or [How to Customize BMad](https://docs.bmad-method.org/how-to/customize-bmad/) for the end-user view.

## Building Agents

The **BMad Agent Builder** (`bmad-agent-builder`) runs six phases of conversational discovery. The first phase detects which agent type fits your vision through natural questions, and the remaining phases adapt based on whether you're creating a stateless expert, a memory-backed companion, or an autonomous agent.

See the [Builder Commands Reference](/reference/builder-commands.md) for details on the build process phases and capabilities.
