# BMAD-00 implementer review

Date: 2026-07-14
Verdict: **PASS for BMAD-00 only**

## Scope

I reviewed the complete BMAD-00 diff against
`docs/implementation-packets/BMAD-00-06-08-09-foundation-2026-07-13.md`
lines 294–391, the BMAD Method/Builder audit note 100, and the exact Method and
Builder source members in `bmad-runtime-lib/_source_review`.

The review covered:

- `packages/bmad-foundation`, including every ledger, managed instruction,
  license, verifier branch, and test;
- the BMAD fixture action correction;
- root package, lockfile, README, CI, boundary, and secret-scan integration;
- source, roster, prompt-reference, runtime-projection, legal, trademark, and
  provenance closure;
- absence of premature normalized descriptors, runtime manifests, runtime
  authority, and product dependency on the context library.

Pre-existing Desktop Support API/D2 changes,
`bmad-runtime-lib/.obsidian/workspace.json`, and `dist-runtime/` were preserved
and excluded from this review.

## Findings resolved during implementation

1. Separated package, module, undeclared source-format, and Node compatibility
   facts instead of collapsing them.
2. Expanded all 76 source-member decisions to claim-specific
   adopt/adapt/defer/reject treatment sets. In particular, Builder edit guidance
   now distinguishes stateless semantics, bounded immutable revisions, deferred
   memory/autonomous behavior, and rejected live-file/lint execution.
3. Closed the exact source graph for all eight managed projections, all six
   roster entries, and Paige's four unavailable prompt references.
4. Preserved source-correct action identities: Agent `create_rebuild`, `edit`,
   `analyze`; Workflow `build`, `edit`, `analyze`.
5. Corrected Method help confidence semantics to preserve `authoritative`,
   `user-asserted`, `heuristic`, `contextual`, and `unknown` without promoting
   inference to completion.
6. Made recovery precedence fail closed for dependency, reference, legal,
   identity, authority, and ordinary hash failures.
7. Added strict, bounded JSON parsing with decoded duplicate-key rejection,
   prototype-free records, valid UTF-8/Unicode, safe integers, and size/depth
   limits.
8. Added stable recovery codes for malformed identity, source-member, legal,
   license, managed-output, and runtime-projection records.
9. Enforced package/file allowlists, physical link containment, Windows reserved
   paths, alternate-data-stream syntax rejection, and exact managed bytes.
10. Removed the context-vault audit from product verification and CI. Boundary
    guards reject both `vault:check` aliases and direct invocation of
    `tools/verify-reference-vault.mjs`; the audit remains an optional development
    command only.

## Evidence reviewed

- Recomputed 76 source-member, 2 identity, and 4 legal hashes against the
  reference source trees: 82/82 matched.
- Foundation verification: 50/50 tests passed; 76 source members and 12 managed
  outputs; semantic digest
  `574ab4d79a8f954c9743741cf9912f5283a255b88a80b07550ed379865d8cc4f`.
- Fixture verification: 76/76 tests passed.
- Root source lane passed: TypeScript contracts 26/26, desktop UI 62/62,
  typecheck, lint, secret scan, boundary inspection, and production web build.
- Boundary syntax/self-regressions and `git diff --check` passed after the final
  command-alias hardening.

## Residual obligations

- Immutable upstream commit/tag and release-signature evidence is absent.
  `promotionEligibility` must remain `blocked_provenance` until that evidence or
  a human-owned expiring exception is recorded.
- BMAD-G0 and BMAD-01 through BMAD-09 are not implemented by this packet.
- Normalized descriptors, the runtime manifest, and the Tauri resource inventory
  remain deferred to BMAD-04.
- Native/cross-language verification and the unrelated D2 work were outside this
  schema-independent packet's applicable source lane.
