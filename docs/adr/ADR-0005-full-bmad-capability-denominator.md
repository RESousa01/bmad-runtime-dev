# ADR-0005: The full BMAD capability denominator

- Status: accepted (2026-07-20)
- Supersedes: only the executable-scope exclusions of ADR-0003. Every other
  ADR-0003 rule — above all the no-source-body rule — remains in force.

## Context

ADR-0003 locked the P5 breadth denominator by excluding Builder authoring
and several Paige/John targets from executable scope. The 100-percent
readiness program requires the opposite posture: every user-visible roster
menu target counts, and nothing is "complete" by being deferred or
excluded. The reviewed source (`bmad-runtime-lib/_source_review`,
BMAD-METHOD 6.10.0 `customize.toml` per agent) fixes the denominator at
**26 roster menu paths** across six agents, plus the **five Builder
authoring operations**:

- Mary (`bmad-agent-analyst`): BP, MR, DR, TR, CB, WB, DP
- Paige (`bmad-agent-tech-writer`): DP, WD, MG, VD, EC
- John (`bmad-agent-pm`): PRD, CE, IR, CC
- Sally (`bmad-agent-ux-designer`): CU
- Winston (`bmad-agent-architect`): CA, IR
- Amelia (`bmad-agent-dev`): DS, QD, QA, CR, SP, CS, ER
- Builder: `agent.analyze`, `agent.create_rebuild`, `agent.edit`,
  `workflow.analyze`, `workflow.build_edit`

DP (document project) and IR (implementation readiness) are shared
capabilities reachable from two agents each; the shared capability is one
record, but every agent/menu path is independently counted and tested.

## Decision

1. **The denominator is monotonic.** `capability-closure-ledger.json`
   records all 26 menu paths and 5 Builder operations. Records may move
   `planned → active`; they may never be deleted, re-excluded, or
   reclassified out of the denominator. The foundation test suite pins the
   exact path set.
2. **Three closed output archetypes**, each with one closed output schema:

   ```text
   document_artifact       -> inert structured document stored locally
                              (sapphirus.bmad-document-artifact.v1)
   governed_change_set     -> candidate D3 proposal requiring fresh review
                              (sapphirus.bmad-governed-change-set.v1)
   inactive_builder_draft  -> versioned draft that cannot install or activate
                              (sapphirus.bmad-inactive-builder-draft.v1)
   ```

   `bmad-dev-story`, `bmad-quick-dev`, and `bmad-qa-generate-e2e-tests`
   are governed change sets; the Builder five are inactive drafts; every
   other capability is a document artifact.
3. **Source intake.** Paige's five targets (DP, WD, MG, VD, EC) and John's
   PRD carry `first_party_semantic_rewrite`: their instruction projections
   must be authored first-party after license/provenance approval, since
   ADR-0003 excluded their source bodies. All other targets carry
   `semantic_rewrite_from_reviewed_source` under the existing intake rules.
   Source bodies are never copied into runtime artifacts.
4. **No shrinkage claim.** If any target cannot be legally or semantically
   rewritten, the program cannot claim 100% breadth. The record stays
   `planned` and the readiness scorecard stays incomplete — the denominator
   is never reduced to make a percentage true.
5. **Activation is evidence-gated.** A record becomes `active` only when
   its focused tests pass through the generic capability-run lifecycle
   (readiness Tasks 5-7) on the same revision.

## Consequences

- The foundation gate now fails on any drift between the ledger and the
  reviewed 26+5 set.
- P8 implementation work consumes this ledger as its work queue; the
  readiness scorecard's `full_bmad_breadth` capability counts these
  records and nothing else.
- Model output for every archetype remains inert data until it passes the
  archetype's authority path (local storage, D3 review, or inactive draft
  storage respectively).
