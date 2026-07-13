# Implementation packet: S0/S1 TypeScript 7 contracts and sealed BMAD fixtures

## Authority and intent

- Owning authority: JSON Schema 2020-12 is the wire-shape source; handwritten semantic validators
  own compatibility, lifecycle, and safe-error rules. Note 99 owns canonical hashing and note 100
  owns BMAD Method/Builder semantics.
- User-visible outcome: the repository provides closed TypeScript 7 boundary shapes and a safe
  fixture proof for one sealed Method capability plus inactive Builder workflow/agent drafts.
  These foundation artifacts are not yet integrated as an executable desktop workflow.
- Contracts read: authority, candidate, approved spec, consumption, result, evidence, durable
  envelope, filesystem capability, stable contract error, package compatibility, and remote-job
  handoff families.
- Non-goals: connected model access, Builder evaluation or activation, candidate scripts,
  package installation, remote-job submission, local file mutation, and any command/process
  capability.
- Stop conditions: duplicate keys, invalid Unicode, unsafe integers, unknown fields or
  discriminators, canonical-hash drift, authority transplantation, one-time-spec replay, local
  path disclosure in renderer-safe errors, or a BMAD fixture that can claim execution, network,
  evaluation, promotion, or activation authority.

## Tests first

- Success fixture: twelve closed schemas, note 99 JCS/hash vectors, package/handoff semantic hash
  rules, one exact Method help skill, one inactive stateless-agent draft, and one inactive simple
  workflow draft.
- Negative/bypass fixture: duplicate decoded members, unpaired surrogates, non-I-JSON numbers,
  unknown majors/discriminators/properties, cross-authority transplantation, spec replay, handoff
  chain breaks/direct-apply claims, overlapping capability sets, recursive authority-bearing BMAD
  keys, path escapes, content tampering, Convert, and active/script/network/evaluation claims.
- Failure/recovery fixture: schema and semantic validators return stable reason categories without
  accepting malformed input or turning fixture metadata into authority.
- Compatibility or migration fixture: package contract epochs and immutable remote-handoff chains
  are covered for the current schema family. General M-1/M-2 upcasters are not yet implemented.

## Change and rollback

- Files/surfaces allowed: `packages/contracts`, `packages/bmad-fixtures`, boundary/source checks,
  and this packet.
- Disable or rollback path: omit the new contract family from callers and keep every BMAD artifact
  sealed or `not_active`; no runtime effect depends on these extension families yet.
- Observability/evidence: `schema-lock.json` records schema hashes, generator configuration,
  language coverage, and generated/fixture tree digests. BMAD source and payload identities are
  independently pinned by path, byte length, and SHA-256.

## Review ledger

- Implementer full-diff review: completed 2026-07-13 for closed schemas, generated exports,
  semantic rules, fixture identity, naming, deterministic regeneration, and unintended authority.
- Independent bug/security review: completed 2026-07-13. It added renderer-safe `ContractError`
  semantics, control/bidirectional-character rejection, rooted UNC/device/drive/POSIX checks,
  details-reference checks, and a follow-up regression for POSIX paths beginning with non-Latin
  letters.
- Commands executed: two consecutive `@sapphirus/contracts` TypeScript 7 verification runs after
  the final change (26/26 tests on each run, 58 controlled TypeScript/fixture outputs with zero
  drift); the generated TypeScript inventory contains 21 reviewed files with tree digest
  `sha256:22c7e5a80257d2ba486433c4ff52337b6ce527b488434038d89ee04ddc120303`;
  one independent BMAD fixture verification run (3 descriptors and 64/64 tests); TypeScript
  compiler version check reporting exactly `7.0.2`.
- Checks skipped and reason: generated Rust compilation/conformance and all C#/.NET generation or
  conformance remain deliberately deferred under the user's toolchain freeze. No native build,
  package, or installer check belongs to this packet.
- Remaining risks: Rust bindings do not yet cover the four new schema-first extension families;
  M-1/M-2 upcasters and N+1 rejection need broader family coverage; the repository-local BMAD lock
  constants are a trust root whose modification requires explicit source/provenance review. Legacy
  `web_managed`/`webDotnet` compatibility shapes remain inert and are not imported by the renderer,
  but they must be pruned or explicitly segregated before the desktop-only contract boundary is
  considered complete.
