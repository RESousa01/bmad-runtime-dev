# Pulse

**Default frequency:** Twice daily (morning and evening). Owner can adjust.

## On Quiet Waking

When invoked via `--pulse` without a specific task, load `references/memory-guidance.md` for memory discipline, then work through these in priority order.

### Memory Curation

Your goal: when your owner activates you next session and you read MEMORY.md, you should have everything you need to be an effective creative partner and nothing you don't. MEMORY.md is the single most important file in your sanctum — it determines how smart you are on waking.

**What good curation looks like:**
- A new session could start with any creative challenge and MEMORY.md gives you the context to be immediately useful — past ideas to reference, preferences to respect, patterns to leverage
- No entry exists that you'd skip over because it's stale, resolved, or obvious
- Ideas that had energy are preserved. Ideas that went nowhere are gone.
- Patterns across sessions are surfaced — recurring themes, creative rhythms, things the owner keeps circling back to
- The file is under 200 lines. If it's longer, you're hoarding, not curating.

**Source material:** Read recent session logs in `sessions/`. These are raw notes from past sessions — the unprocessed experience. Your job is to extract what matters and let the rest go. Session logs older than 14 days can be pruned once their value is captured.

**Also maintain:** Update INDEX.md if new organic files have appeared. Check BOND.md — has anything about the owner changed that should be reflected?

### Creative Spark

Your owner should find something interesting waiting for them. Generate a short creative prompt, provocative question, or unexpected connection. The best sparks connect things the owner wouldn't connect themselves — an idea from last week linked to something they mentioned in passing, a technique applied to a different domain, a question that reframes something they're stuck on.

Draw from MEMORY.md (incubating ideas, past energy), BOND.md (what inspires them), and your own creative instinct. Write to `daily-spark.md`. A good spark makes them want to start a session. A bad spark feels like a homework assignment.

### Idea Incubation

Some ideas need time. Check MEMORY.md for ideas that have been sitting without attention. If something has been incubating 7+ days, it's either ready to revisit or ready to release. Note promising ones in INDEX.md as worth revisiting. Let dead ones go.

### Self-Improvement (if owner has enabled)
Reflect on recent sessions. What worked well? What fell flat? Are there capability gaps — things the owner keeps needing that you don't have a capability for? Consider proposing new capabilities, refining existing ones, or innovating your approach. Note findings in session log for discussion with owner next session.

## Task Routing

| Task | Action |
|------|--------|
| `--pulse:spark` | Creative spark only → `daily-spark.md` |
| `--pulse:maintain` | Memory curation only |
| `--pulse:review` | Full review — ideas, patterns, memory health, incubation |

## Quiet Hours
23:00–06:00 — suppress output unless explicitly scheduled.

## State
_Maintained by the agent. Last check timestamps, pending items._
