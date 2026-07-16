---
title: "Living Knowledge Base Design"
status: approved-for-planning
approved_on: 2026-07-16
scope: bmad-runtime-lib
---

# Living Knowledge Base Design

## Goal

Convert `bmad-runtime-lib` from a mostly historical implementation-planning
vault into a living, evidence-backed knowledge base for the Sapphirus desktop
runtime. It must preserve source provenance and remain optional reference
context: product builds, tests, CI, runtime, packaging, and installers cannot
depend on the vault.

## Audience and success

The primary readers are maintainers, implementation agents, reviewers, and
architects. A reader must be able to identify what is implemented, planned,
deferred, historically preserved, or unknown without treating an old plan as
current product truth.

Success requires a current-authority layer that is anchored to a repository
commit and research date; evidence for every material claim; mechanically
checked toolchain and authority consistency; and a clean-snapshot validation
result with no errors or warnings.

## Truth and evidence model

Each material claim has a stable ID and records its text, classification,
implementation status, evidence references, confidence, supersession relation,
revalidation deadline, and limitations. Material claims affect architecture,
security, implemented capability, compatibility, dependencies, support
lifecycle, release readiness, or build order.

Allowed classifications are:

- `IMPLEMENTED_FACT`: committed implementation plus test or reproducible check.
- `VERIFIED_EXTERNAL_FACT`: dated primary external evidence.
- `ARCHITECTURE_DECISION`: deliberate Sapphirus choice with decision evidence.
- `PLANNED`: approved direction not yet implemented.
- `WORKTREE_CANDIDATE`: uncommitted work, never represented as implemented.
- `HISTORICAL`: retained context that is not current authority.
- `UNKNOWN`: unresolved; no permissive conclusion is inferred.

Evidence precedence is committed source and manifests, reproducible checks,
accepted architecture/implementation records, BigBrain and prior synthesis, and
then external documentation. Existing plans and BigBrain guide investigation;
they do not override newer repository evidence.

Repository claims require implementation and test/check evidence. External
claims require an official canonical source and a second official release,
changelog, registry, specification, or compatibility source when available.
A unique canonical source is recorded as an explicit single-source exception.
Architecture claims require decision evidence and a separate implementation
status check. Source-semantic claims require a locked source identity and
reviewed extraction evidence.

## Information architecture

New `current/` notes are the only current product authority:

1. Current product state.
2. Architecture and ownership.
3. Capability matrix.
4. Security and trust boundaries.
5. Contracts and persistence.
6. Toolchain and dependencies.
7. Verification and release readiness.
8. Risks, roadmap, and open decisions.

An evidence registry and a catalog classify every existing root note as current
authority, supporting reference, source evidence, planned, superseded,
historical, or preserved verbatim. Preserved source evidence remains byte-stable.
Older root notes may be superseded but remain reachable and attributable.

The implemented product is Windows-local. The web-managed architecture is a
deferred product option, not an implemented delivery. Azure supplies desktop
support-plane capabilities and never becomes the local lifecycle authority.

## Validation and maintenance

Validation is offline and deterministic. It checks claim/evidence completeness,
claim IDs, stale revalidation dates, pin consistency with repository manifests,
conflicting current claims, links, supersession chains, duplicate authority
ownership, historical notes presented as current, stable LF manifest generation,
and clean-snapshot integrity.

Live URL availability is not required during normal local validation. External
research occurs in a deliberate revalidation pass and records its retrieval date
and source quality. The library remains removable without changing the product's
`verify:source` path.

## Migration phases

1. Capture the commit, research cutoff, toolchain facts, and dirty-worktree
   boundary.
2. Add the claim schema, evidence registry, and complete root-note catalog.
3. Write the eight current-authority notes from repository and primary-source
   evidence.
4. Reclassify high-risk misleading authority and toolchain claims.
5. Add deterministic validators and their behavior tests.
6. Regenerate the manifest and provenance record using LF-stable generation.
7. Run clean-snapshot validation and independent architecture, security,
   full-stack, cloud, and research review.
8. Update BigBrain with the authority model, evidence cutoff, and maintenance
   procedure.

## Acceptance criteria

- Every root Markdown note has an explicit authority classification.
- Every material claim in `current/` has complete evidence.
- No uncommitted behavior is presented as implemented.
- Current notes agree with their declared repository commit.
- Documented pins mechanically agree with repository manifests.
- External facts are dated and double-checked or marked as exceptions.
- Preserved evidence remains recoverable and unchanged where required.
- Validation requires no network and passes from a clean snapshot.
- Product source verification and packaging remain independent of this library.
- No unresolved Critical review finding remains.

## Non-goals

- Implementing missing product capabilities such as D2, signing, or packaged
  pilot validation.
- Rewriting imported source snapshots.
- Claiming production or pilot readiness.
- Making the library a runtime/build/CI dependency.
- Automatically upgrading dependencies based only on newer releases.

## Protected scope

Only library-owned files are in scope. The existing `.obsidian/workspace.json`
change and all parent-repository D2/support-plane work are user-owned and must
not be modified, staged, or committed by this migration.
