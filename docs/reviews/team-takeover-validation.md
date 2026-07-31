---
title: "Forge documentation team takeover validation"
doc_type: review
status: blocked
owner_role: devex-owner
last_verified: 2026-07-31
review_due: 2026-08-07
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - docs/README.md
  - docs/development/onboarding.md
  - docs/development/documentation-policy.md
  - docs/_meta/knowledge-manifest.json
supersedes: []
---

# Forge documentation team takeover validation

## Gate

Gate H requires a developer who did not author this migration to orient themselves from the
repository documentation, explain the system accurately, make one documentation change, and pass
the documented validation workflow without private coaching.

## Prepared exercise

The independent reviewer must:

1. Start at `docs/README.md` and identify the current product state, runtime authority boundary,
   supported authentication/model routes, and release-readiness boundary.
2. Locate the source evidence for one current claim.
3. Explain why archive and source-analysis documents are not product authority.
4. Make a small, accurate documentation update with complete frontmatter.
5. Run `pnpm docs:generate` and `pnpm docs:verify`.
6. Record confusion, missing context, commands run, results, and any required follow-up.

## Current evidence

- The entrypoint, onboarding path, ownership roles, maintenance cadence, and validation commands
  exist.
- Machine checks enforce metadata, source paths, generated files, authority rules, current claims,
  reference-source policy, and archive hashes.
- The migration author has exercised those checks, but that is not independent takeover evidence.

## Result

**Blocked.** No independent developer was available during this implementation. The exercise has
not been executed, so Gate H is not accepted. `KNOWLEDGE-REVIEW-001` in the knowledge manifest
tracks the external dependency.

## Completion record

When an independent reviewer performs the exercise, append:

- reviewer identity and date;
- starting commit and resulting commit;
- task chosen and files changed;
- commands and exact outcomes;
- questions or navigation failures;
- pass/fail decision and owner for each follow-up.
