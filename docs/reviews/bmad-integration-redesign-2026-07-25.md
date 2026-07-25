# BMAD integration — source study and redesign proposals

**Date:** 2026-07-25
**Source studied:** `bmad-runtime-lib/_source_review/BMAD-METHOD-main/BMAD-METHOD-main`
(`src/bmm-skills`, `src/core-skills`, `src/scripts`, `src/bmm-skills/module-help.csv`)
**Product intent:** Forge is the BMAD runtime — practise the method without a
separate LLM runtime (Claude Code, Cursor, …). Chat is the surface BMAD runs
on. BMAD must never replace chat.
**Status:** **implemented.** This began as a proposal; all six steps of the §4
sequence shipped the same day as packets
[P10](../implementation-packets/P10-verbatim-method-adoption-2026-07-25.md)
through [P13](../implementation-packets/P13-core-skill-lane-2026-07-25.md),
under [ADR-0008](../adr/ADR-0008-verbatim-method-adoption.md). Read the
analysis below as the design rationale for that work, not as a plan. Statements
in the present tense describing what Forge *lacks* — "No chat lane exists",
`BmadConfigResolver` "wired to nothing", `module-help.csv` "not ingested",
"entirely absent: `core-skills/…`" — were true when written and are now false.
The four open questions in §5 are all answered by the packets.

---

## 1. What BMAD actually is

Reading the source rather than the capability list, BMAD is not a catalog of
functions. It is a **conversational method with five runtime primitives**:

### 1.1 The agent is a persona with a menu, and chat is always available

`bmad-agent-architect/SKILL.md` activation: resolve customization → run prepend
steps → adopt persona → load persistent facts → load config → greet → run
append steps → present menu **or dispatch directly from intent**.

Two lines matter enormously:

> "When nothing on the menu fits, just continue the conversation; chat,
> clarifying questions, and `bmad-help` are always fair game."

> "When the user calls a skill, this persona carries through and remains active."

The method's own source says chat is the base state and skills are dispatched
*out of* it. Forge inverted this exactly.

The menu is data, in `customize.toml`:

```toml
[[agent.menu]]
code = "CA"
description = "Produce the architecture spine: …"
skill = "bmad-architecture"     # or `prompt = "…"` for an inline instruction
```

### 1.2 The memlog is the method's central mechanic

`src/scripts/memlog.py` — an append-only, chronological, atomic working memory
per run (`.memlog.md`). Its three invariants: append-only (no edit, no delete,
by design), write-only/blind (never re-read mid-session), and **no lifecycle
status** (completion is an `event` entry, not frontmatter).

`bmad-spec` states the consequence outright:

> "Deriving the contract from a living log instead of editing the contract in
> place is what lets the steps around the spec run in any order and feed the
> same spec without merge drift: the log only accumulates, the artifact is
> re-rendered."

So: **artifacts are derived from the log on every run, never hand-patched.**
`SPEC.md`, the architecture spine, the PRD — all are renders of an accumulating
log. That is what makes the method resumable, re-orderable, and merge-free.

### 1.3 The workflow graph is machine-readable

`src/bmm-skills/module-help.csv` encodes the whole method:

```
module,skill,display-name,menu-code,description,action,args,phase,
preceded-by,followed-by,required,output-location,outputs
```

- `phase`: `anytime`, `1-analysis`, `2-planning`, `3-solutioning`, `4-implementation`
- `preceded-by` / `followed-by`: the DAG (soft ordering)
- `required`: the hard gates — PRD, architecture, epics-and-stories, IR,
  sprint-planning, create-story, dev-story
- `output-location` + `outputs`: **completion detection** — where an artifact
  lands and what it is called

`bmad-help` reads this CSV, resolves `output-location` against config, scans for
`outputs` patterns, fuzzy-matches files to rows, and infers where you are.

### 1.4 Config is a precise four-layer merge

`resolve_config.py`: `_bmad/config.toml` → `config.user.toml` →
`custom/config.toml` → `custom/config.user.toml`. `resolve_customization.py`:
skill base → team → user. Merge rules are exact: scalars override, tables deep
merge, **arrays of tables keyed by `code`/`id` replace matching and append new**,
all other arrays append.

### 1.5 Skills compose

Skills invoke skills: `bmad-architecture` calls `bmad-advanced-elicitation`,
the reviewer-gate lenses (`bmad-review-adversarial-general`,
`bmad-review-edge-case-hunter`, `bmad-review-verification-gap`),
`bmad-party-mode`, and offers `bmad-spec` as a follow-on. Composition is the
norm, not an edge case.

---

## 2. Where Forge's model diverges

Forge modelled BMAD as **29 capabilities, each one stateless model call
returning one sealed archetype**. That is a faithful model of the *outputs* and
no model at all of the *mechanics*.

| BMAD primitive | Forge today |
|---|---|
| Persona you converse with | Persona is *viewable* (`bmad.persona.view`); never bound to a conversation |
| Chat always available | **No chat lane exists**; `session.create`/`task.submit` are stubbed |
| Append-only memlog per run | Nothing. Each capability call is stateless |
| Artifacts derived from the log | Artifacts are the direct model output of one call |
| Workflow graph with gates + completion detection | Not ingested. `bmad-help-action-graph.json` carries 2 skills / 2 actions |
| Four-layer config merge | `BmadConfigResolver` **is implemented (388 LOC, tested) and wired to nothing** |
| Skills compose | Each capability is terminal |
| `_bmad/` workspace state | `bmad.scan` classifies asset *presence* only — no config read, no artifact detection |

### 2.1 Coverage is also about half the method

Forge's ADR-0005 denominator covers `bmm-skills` only. Entirely absent:

- **`core-skills/bmad-spec`** — the spec kernel, the machine contract downstream
  skills consume
- **`core-skills/bmad-advanced-elicitation`** — the elicitation engine
- **the three reviewer lenses** — adversarial, edge-case-hunter, verification-gap
- `bmad-party-mode`, `bmad-shard-doc`, `bmad-index-docs`, `bmad-forge-idea`,
  `bmad-customize`, the two editorial-review skills
- `bmad-generate-project-context` (GPC), `bmad-sprint-status` (SS),
  `bmad-checkpoint-preview` (CK), `bmad-dev-auto`
- `bmad-create-story:validate` (VS) — Forge collapses `create-story`'s two
  actions into one
- `bmad-create-prd` / `bmad-edit-prd` / `bmad-validate-prd` behind `bmad-prd`

### 2.2 Honest self-assessment of the architecture state machine

The ordered architecture progression shipped earlier today
([P9](../implementation-packets/P9-architecture-state-machine-2026-07-25.md))
works and is well tested, but it is a **special case of a mechanic that should
be general**. It hard-codes eight steps for one capability, keeps its session
in memory, and invents its own decision-log format. A memlog primitive
(§3.1) subsumes it: the architecture run becomes "a run whose memlog
accumulates decisions and whose spine is derived from it", with no
capability-specific state machine at all. If the memlog lands, P9 should be
refactored onto it rather than kept alongside.

---

## 3. Proposals, ranked by leverage

### 3.1 ⭐⭐⭐ Make the memlog a host-owned primitive

**The single highest-leverage change.** Every skill run gets a run folder with
an append-only `.memlog.md`; artifacts are *derived* from it.

Why Forge is unusually well placed: upstream's memlog is a Python script doing
atomic temp-file-plus-rename. Forge already has governed D3 writes, an
append-only effect journal, checkpoints, and rollback. The memlog maps onto
machinery that already exists and is stronger than the upstream original.

What it unlocks, all at once:

- **Resumability** — reload the memlog, continue a run across sessions. This is
  the property the current stateless lane cannot express at all.
- **Re-derivation** — re-render `SPEC.md` / the spine / the PRD from the log;
  no merge drift, no hand-patching.
- **Multi-turn workflows without bespoke state machines** — coaching loops,
  elicitation, reviewer gates all just append.
- **Real fidelity** — this is the mechanic that makes the adapted one-shots into
  actual BMAD runs, moving the matrix off "faithful: 0".

Host owns append (the log is authority-controlled); derived artifacts enter D3
as ordinary proposals, so nothing bypasses review.

### 3.2 ⭐⭐⭐ Ingest the workflow graph and compute state in Rust

Ship `module-help.csv` as sealed foundation data. Then, natively:

1. resolve `output-location` variables from the project's config,
2. scan for `outputs` patterns through governed reads,
3. compute phase, completed steps, next *required* step, available steps.

**This is strictly better than upstream.** Upstream must ask an LLM to
fuzzy-match filenames because it has no code. Forge has Rust: it can compute the
answer deterministically, with no model call, no receipt, no hallucination.

It also dissolves the standing "Help sees only a fraction of the product" gap —
not by widening the sealed Help binding, but by replacing *guessed* navigation
with *computed* navigation. The model is then only needed for the parts that
genuinely need judgement.

Deliverable: a "where you are in the method" panel that is always correct.

### 3.3 ⭐⭐⭐ Agent = persona bound to the conversation, menu dispatched from chat

Implements §1.1 directly, and is what the product intent asks for:

- Selecting an agent binds its sealed persona projection to the conversation and
  prefixes turns with `{agent.icon}`.
- The persona **carries through** a skill run and is still active afterwards.
- Typing an intent that matches a menu item dispatches it; anything else is
  ordinary conversation.
- `None` stays first-class — plain chat with no BMAD concept involved.

`BmadMenuTargetKind` already models `SkillTarget` *and* `PromptReference`, so
the menu model is already faithful; it is the *dispatch from conversation* that
is missing.

### 3.4 ⭐⭐ Wire the config resolver that already exists

`BmadConfigResolver` implements upstream's four-layer merge, keyed-array rules
included, and is used by nothing outside its own tests. Wiring it to the
workspace's real `_bmad/` config would give: `user_name`,
`communication_language`, `document_output_language`, `planning_artifacts`,
`output_folder`, `project_knowledge`, and per-skill `customize.toml` overrides —
which §3.2 needs anyway to resolve output locations.

This is the cheapest big win in the list: the hard part is already built.

### 3.5 ⭐⭐ Reframe "capability" as "skill run"

Today: capability = one archetype-producing call. Proposed: a **run** is a
resumable session with a memlog that may emit document artifacts and governed
change sets *at any point*. Archetypes stop being the unit of work and become
outputs of one.

This removes the awkwardness where a workflow that naturally produces a document
*and* a change set has to be modelled as one or the other.

### 3.6 ⭐⭐ Adopt the missing core skills, starting with `bmad-spec`

`bmad-spec` is the machine contract downstream skills consume; without it, the
planning → solutioning → implementation chain has no shared substrate. The three
reviewer lenses are the method's quality gate and map cleanly onto separate
reviewed model calls over an artifact Forge already holds.

### 3.7 ⭐ Elicitation as a chat affordance

`bmad-advanced-elicitation` operates on *the output just produced*. In a chat
surface that is natural — "push harder on that", "pre-mortem this". Nothing
about it needs a new lane once §3.1 and §3.3 exist.

---

## 4. Recommended sequence

| # | Step | Why first |
|---|---|---|
| 1 | **Chat lane + persona binding** (§3.3) | Nothing else is reachable without a conversation to reach it from. Also the stated product intent. |
| 2 | **Config resolver wiring** (§3.4) | Cheap, already built, and a prerequisite for §3.2. |
| 3 | **Workflow graph + deterministic state** (§3.2) | Highest user-visible value per unit of work; no model dependency; fixes navigation honestly. |
| 4 | **Memlog primitive** (§3.1) | The deep change. Do it once the surface exists to exercise it. |
| 5 | **Refactor P9 onto the memlog** (§2.2) | Removes the bespoke state machine. |
| 6 | **Skill-run reframing + core skills** (§3.5, §3.6) | Follows naturally once runs are stateful. |

## 5. Open questions for the owner

1. **Provenance for the new source.** The core skills and `module-help.csv` are
   not in the current ADR-0005 denominator. Ingesting them means an intake pass
   under ADR-0003 (source review, treatment decisions, sealed projections) —
   the same process P8 used. Is that in scope, or should the graph be
   product-owned data rather than sealed BMAD source?
2. **Does the memlog live in the user's workspace or in Forge's store?**
   Upstream writes `.memlog.md` into the project. Workspace-visible is more
   faithful and lets other tools read it; store-side is safer and needs no D3
   proposal per append. My inclination: workspace, through D3, because
   portability is the point of the method.
3. **Should Forge write `_bmad/` at all** (config, run folders), or only read
   what the user's existing BMAD install put there?
4. **How much does the chat lane persist?** In-memory per session (cheap, matches
   the existing coordinators) versus a store migration for durable transcripts.
