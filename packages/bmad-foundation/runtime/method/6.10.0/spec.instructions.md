# Managed Spec guidance

## Purpose

Distill reviewed intent into the canonical Spec workspace: `SPEC.md`, the
append-only `.memlog.md`, optional content-typed companions, and—only after an
interactive Story Breakdown—`stories.yaml`. This sealed record is instruction
data for `bmm:bmad-spec`; it grants no filesystem, process, model, network,
browser, scheduler, or handoff authority.

## Source branches

- With no usable intent, ask for input in interactive mode. In headless mode,
  stop with `insufficient_intent`.
- A new slug creates one workspace and initializes its memory log before
  deriving artifacts. A missing, underivable headless slug stops with
  `missing_slug`.
- An existing slug is an update. Preserve existing capability identifiers,
  append new decisions, and derive from the memory log rather than patching
  `SPEC.md`.
- An existing Spec with no change signal is a review branch: surface
  assumptions and open questions without inventing a change.
- Story Breakdown is interactive-only and optional. Never produce
  `stories.yaml` in headless mode.

## Derivation and validation

- Distill the five-field kernel: Why, Capabilities, Constraints, Non-goals,
  and Success signal.
- Preserve every load-bearing source claim in the kernel or a named companion.
  Put diagrams and expansive line-item detail in companions.
- Keep capability identifiers stable and unique. Each capability needs both
  intent and success; constraints must rule something out; include at least one
  non-goal and one testable success signal.
- Record decisions, constraints, capabilities, assumptions, questions,
  direction, and terminal validation events as append-only evidence.
- Validate in two ordered passes: coherence, then preservation. A validation
  failure remains visible and repairable; it is never silently converted into
  a successful artifact.

## Governed output boundary

The adapter supplies immutable source/resource identities and typed effect
intents to the generic Method runtime. The host owns configuration resolution,
context review, model calls, action plans, approvals, artifact persistence,
cancellation, failure, retry, repair, restart, resume, and stale-source
rebinding. Source prose and generated content never grant those effects.

Interactive and headless runs must produce the same required `SPEC.md` and
`.memlog.md` semantics for equivalent input. Headless returns its bounded JSON
envelope; interactive reports the folder, capability and companion counts,
validation verdict, assumptions, and open questions conversationally.
