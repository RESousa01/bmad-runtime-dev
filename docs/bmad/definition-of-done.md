# BMAD binary definition of done

BMAD completion is binary. The implementation is complete only when every mandatory gate passes against the exact pinned source identities, one Git commit, one source tree, and fresh evidence. There is no weighted average and task counts, code presence, or unit tests alone never establish completion.

## Exact target

| Target | Version | Archive SHA-256 | Required denominator |
|---|---|---|---|
| BMAD Method | 6.10.0 | `2993a38a36a9586d16a62a4eacb3400db25dd1ea594bbfe4682789d7f95ac02b` | 597 source members, 48 primary skills |
| BMad Builder | 2.1.0 | `d3c70744a9875623b01856cc907cf558324bacc920f0d860c36ad2788a4d2852` | 299 source members, 5 production skills, 9 public actions, 1 internal Eval Runner action |

All aliases must appear in the generated compatibility catalog, resolve exactly once to a canonical capability before dispatch, and carry a migration policy. Alias resolution never grants authority.

## Mandatory evidence

The verifier requires fresh evidence for source identity, recursive closure, contracts, Method semantics, Builder conformance, Rust runtime, UI, managed support plane, real desktop E2E, security, performance, persistence and migrations, signed packaging and updates, deployed managed mode, documentation, pilot, traceability, and independent review.

Each gate record must:

- record `status: "pass"` and contain no skipped, unavailable, partial, or incomparable result;
- be bound to the current Git commit and source-tree hash;
- be inside the gate's freshness window;
- contain at least one required command, with every required command unskipped and exiting zero;
- include interactive, headless, error, resume, and cancel conformance where the capability declares those states.

Local evidence expires after seven days. Environment and release qualification expires after 24 hours. Independent review expires after four hours. Regeneration does not rescue evidence from another commit or source tree.

Production source accounting and mandatory resource closure must both be 100%. No primary Method or Builder row may remain deferred, rejected, missing, unprojected, normalized-only, implementation-pending, unreleased, or unreachable.

## Running the phase-zero verifier

Run:

```text
node tools/verify-bmad-100.mjs --explain
```

During implementation this command is expected to return a nonzero exit and list exact blocker codes, evidence paths, and remediation task IDs. That is a correct binary result, not a verifier failure. The focused verifier tests prove that a synthetic, internally consistent complete evidence set is the only zero-exit state:

```text
node --test tools/bmad100/tests/verify-bmad-100.test.mjs
```

The contract template at `packages/bmad-foundation/bmad-100-scorecard.json` intentionally contains `complete: false`. It defines required inputs and must never be copied or treated as the live final scorecard.

## Final claim

Only the final release verifier may write `artifacts/bmad100/final-scorecard.json`. A public 100% claim additionally requires the strict independent review and final approval to reference the exact scorecard hash. Source or unit-test success cannot substitute for Windows, WAM/Entra, signing/updater, deployed Azure, pilot, operational approval, or other required external evidence.
