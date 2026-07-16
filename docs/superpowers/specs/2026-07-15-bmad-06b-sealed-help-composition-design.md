# BMAD-06B Sealed Help Composition Design

**Status:** Approved continuation of the BMAD-06 milestone defined in
`docs/implementation-packets/BMAD-00-06-08-09-foundation-2026-07-13.md`.

**Goal:** Build the complete local authority boundary for the first sealed
`bmad-help` capability without making the capability runnable before the D2
consent/model lane is integrated and verified.

## Context and constraints

BMAD-06A already prevents raw renderer/model JSON from advancing a Method
session. It records exact decision, request, session-authority, D2 invocation,
bridge, raw-response, accepted-transition, receipt-evidence, and checkpoint
lineage. BMAD-06B must now bind that authority to the installed Method 6.10.0
Help source and define how an eventual verified model response becomes canonical
local evidence.

The reviewed BMAD source skill asks for catalog, config, artifacts, project
knowledge, local file reads, remote module documentation, and an offer to invoke
the next skill. The sealed Sapphirus projection intentionally narrows that
behavior: it receives only bounded host facts, cannot open paths or fetch URLs,
cannot invoke a capability, and returns advisory data only.

The following constraints are absolute:

- Rust owns the Method session, transition table, checkpoint, evidence, and
  local content registration.
- D2 may perform only a transient, consent-bound model call. It cannot create
  local authority, artifact references, lifecycle state, or effect claims.
- The renderer, package data, workspace content, and model output cannot select
  provider endpoints, tools, commands, state transitions, storage references,
  or completion evidence classes.
- `bmad-help` remains `created_unbound`, `runnable: false`, and
  `completion_claimed: false` until the D2 integration gate is explicitly
  completed.
- No runtime code reads `bmad-runtime-lib`; that directory remains review
  context. Production uses the independently reviewed, manifest-bound managed
  projection under `packages/bmad-foundation`.

## Options considered

### 1. Let the model return `MethodHelpRecommendation`

This is rejected. The canonical recommendation contains host-owned IDs,
timestamps, `ArtifactRef` values, evidence classification, and a cryptographic
self-hash. A model cannot safely mint those facts or reliably calculate the
self-hash. Allowlisting after the fact would still give model JSON selection
authority over evidence and would blur raw response bytes with trusted local
evidence.

### 2. Add a private Rust-only Help response shape

This is rejected. D2, the desktop host, and persisted replay need one qualified
cross-language response contract and one exact schema-closure hash. A Rust-only
interpretation would recreate the contract drift BMAD-01 was designed to
prevent.

### 3. Use an untrusted proposal followed by host materialization

This is selected. The model returns a small closed `MethodHelpProposal` that
contains only a proposed catalog capability, opaque host-issued evidence token
IDs, and bounded rationale, or one closed no-recommendation reason. The trusted
host validates the exact bytes and semantic facts, resolves tokens, derives the
evidence class, and creates the canonical recommendation and advance result.

## Contract boundary

BMAD-06B adds three standalone v1 schema roots:

1. `bmad-method-help-proposal.schema.json` is the D2/model output. It has no
   self-hash and no IDs, timestamps, content/artifact references, paths,
   authority, tools, effects, confidence field, or lifecycle fields.
2. `bmad-method-help-recommendation.schema.json` exposes the existing closed
   `MethodHelpRecommendation` union as a standalone host-canonical record.
3. `bmad-method-advance-result.schema.json` exposes the existing closed
   `MethodAdvanceResult` union as a standalone host-canonical record.

The recommended proposal branch is exactly:

```text
proposalKind       = recommended_capability
capabilityKey      = complete BmadCapabilityKey proposed from the supplied catalog
evidenceTokenIds   = 1..64 unique host-issued ContractId values
rationaleSummary   = 1..4096 characters, excluding C0, DEL, and bidi controls
```

The no-recommendation branch is exactly:

```text
proposalKind = no_recommendation
reasonCode   = catalog_evidence_absent |
               completion_evidence_ambiguous |
               dependency_unavailable
```

Every standalone root receives a locked transitive schema-closure hash. The
closure manifest is canonical JSON with the root schema ID and the unique,
strictly schema-ID-sorted set of the root plus every transitively referenced
production schema, represented by `{schemaId, canonicalSha256}`. The closure
hash is raw SHA-256 over those canonical manifest bytes. Runtime code consumes
generated constants for these values; no caller supplies an accepted schema
hash.

Two new self-hash domains are qualified across TypeScript, Rust, and C#:

- `bmad-method-help-recommendation/v1`, excluding `recommendationHash`.
- `bmad-method-canonical-advance-result/v1`, excluding `resultHash`.

The existing private Rust transition remains
`bmad-method-advance-result/v1`; the distinct canonical domain is intentional.
The untrusted proposal is bound only by the exact raw-byte SHA-256 already
retained as `model_response_payload_hash`.

## Sealed source ownership

`BmadPackageLoader` is the only place that simultaneously holds the generated
typed descriptor, the already verified descriptor JSON, and the observed
managed instruction bytes. It therefore creates one package-owned opaque Help
source value after all existing package gates pass.

The sealed value retains:

- exact package, descriptor, source-snapshot, and observed-inventory identity;
- `core/bmad-help`, the full direct execution profile, and profile hash;
- source entrypoint, projection, resource-set, skill-descriptor, managed
  instruction, central graph/resolution, module metadata, and ledger identity;
- the exact projection source closure, blocked `file_read`/`web` intents, and
  typed host replacements;
- exact managed instruction bytes behind a redacted, non-serializable,
  non-deserializable wrapper; and
- the exact descriptor facts needed to cross-bind the separately assembled,
  package-bound native catalog used by the Help compiler.

The loader independently recomputes all nested descriptor hashes before it
retains the value. It rehashes the managed instruction from observed bytes. It
cross-binds source-tree references against descriptor inventory and the
manifest-verified adoption/semantic ledgers; it does not claim to observe
upstream source-tree bytes that are deliberately absent from the product
snapshot.

`BmadLoadedSkill` remains unchanged because it is a display/catalog projection
with existing public test fixtures. A package-owned wrapper exposes only shared
references to the display package and sealed Help source. The app stores that
wrapper and does not parse or duplicate instruction bytes.

## Exact non-runnable binding

The Help compiler consumes only the sealed source, the bound catalog, and an
opaque trusted model/profile value. It does not accept free descriptor,
instruction, schema, customization, validation, egress, or model-binding
digests.

The compiler derives an explicit no-agent binding and the exact direct Help
step table. It derives a domain-separated empty Help customization commitment
from the exact capability identity and empty customization-layer set; it keeps
the core module metadata hash under its honest name. It derives a fixed Help
validation-profile commitment from a reviewed local descriptor of the semantic
rules. These commitments are internal host facts and have golden literal tests;
they are never aliases for unrelated descriptor hashes.

The result remains a non-runnable invocation plan. It owns the exact instruction
bytes, catalog candidates, Method binding, request/proposal/recommendation/result
schema identities, and validation facts. Its debug output redacts content and it
has no serialization or arbitrary-byte constructor.

## Proposal adaptation and two-record persistence

The eventual D2 adapter supplies an opaque verified output containing exact raw
proposal bytes and a production-verified receipt. BMAD-06B validates in this
order:

1. enforce the byte limit and strict-parse with duplicate-member rejection;
2. validate the standalone proposal schema and semantic safety rules;
3. require the proposed capability to be an exact non-`_meta` catalog member;
4. resolve every evidence token from the compiled host allowlist;
5. derive evidence class from the resolved host facts without upgrades;
6. prove a no-recommendation reason from the compiled catalog/dependency/evidence
   facts; and
7. retain the exact original bytes and raw-byte hash.

The host then creates two separate canonical records:

- a `MethodHelpRecommendation` with host ID/session/time, catalog-derived
  `guidanceRequired`, resolved `ArtifactRef` values, derived evidence class, and
  host-computed `recommendationHash`;
- a `MethodAdvanceResult::completion_candidate` with host ID/time, exact
  request/invocation/schema lineage, a local `ContentRef` to the canonical
  recommendation bytes, no produced artifacts, zero unresolved items, and a
  host-computed `resultHash`.

`responseContentRef` never points to the raw model proposal. The proposal and
canonical recommendation are distinct records: `model_response_payload_hash`
binds the former, while the canonical recommendation content reference and
canonical advance-result hash bind the latter.

Only a store-owned operation can prove local content registration. A shape-
checked Rust receipt is not described as persistence proof. Finalization reuses
the Method aggregate's existing non-mutating verified-result validator (or
accepts on an exact clone) before the store atomically links canonical content,
aggregate projection, checkpoint, evidence, and outbox.

BMAD-06A lineage is extended with `canonical_advance_result_hash` and enough
canonical data to recompute it after restart. The fixed internal transition is:

```text
Completed / currentStep=recommend / nextStep=None / workingArtifacts=[]
```

No proposal field chooses that transition.

## Failure behavior

- Any structural, Unicode, semantic, token, catalog, dependency, hash, receipt,
  or lineage mismatch fails before Method mutation.
- A confidence upgrade, invented catalog/dependency absence, `_meta` target,
  unknown evidence token, coordinated schema substitution, or canonical-record
  drift is rejected.
- Invalid/refused/incomplete external outcomes do not fabricate a successful
  recommendation or checkpoint.
- Failed finalization leaves the original aggregate, registered payload links,
  checkpoint index, evidence, and outbox unchanged; append-only unreferenced CAS
  staging may remain non-authoritative under the existing store model.
- Existing Created/unbound v1 sessions continue to restore without migration.

## Verification strategy

Each behavior slice follows red/green/refactor and receives a frozen commit plus
independent spec/quality review.

- Contract tests prove structural and semantic agreement across TypeScript,
  Rust, and C#, closure-hash drift detection, self-hash vectors, duplicate/
  unknown member rejection, bounds, timestamps, Unicode/control safety, and
  domain collision resistance.
- Loader tests prove exact extraction, nested coordinated-reseal rejection,
  ledger/source closure, managed-byte binding, opaque/redacted traits, and app
  manifest tamper rejection.
- Compiler tests prove no-agent/direct binding, exact catalog retention, honest
  no-customization/validation commitments, coordinated-substitution rejection,
  and continued non-runnability in runtime, IPC, and app projections.
- Adapter/store tests prove proposal-to-canonical materialization, evidence and
  no-recommendation truth, two-record hashes, aggregate non-mutation, atomic
  persistence, checkpoint replay, and restart recomputation.
- Final gates include format, strict Clippy, the full locked Rust workspace,
  pinned Node/pnpm contract and UI suites, production UI build, boundary checks,
  path-scoped diff checks, and an independent whole-slice review.

## Deferred activation gate

This design does not enable a model call. Activation requires the separate D2
worktree to provide opaque verified output and receipt types, real desktop
consent, exact model/request/egress bindings, privacy-canary proof, and a reviewed
composition commit. Only that later gate may replace the inert Help coordinator
or advertise the capability as runnable.
