# Sapphirus contracts

This package is the reviewed JSON Schema 2020-12 source and language-neutral
conformance suite for Sapphirus. It contains no lifecycle authority, policy
decision, filesystem access, executor dispatch, or persistence behavior.

## Commands

- `pnpm verify` and `pnpm verify:typescript` run the active TypeScript 7 lane:
  deterministic TypeScript/fixture regeneration checks, schema posture, TS7
  type checking, binding inventory/digest checks, and JavaScript conformance tests.
- `pnpm generate`, `pnpm check`, and `pnpm verify:cross-language` run the locked
  three-language lane. They require the repository-local `cargo-typify@0.6.1`
  binary, the manifest-scoped `Corvus.Json.Cli@5.1.0` tool, and the exact SDKs
  recorded in `tools/contract-codegen/tool-lock.json`.
- `pnpm test` exercises strict JSON parsing, RFC 8785 canonicalization,
  purpose-separated hashes, fixture validation, and delivery binding.
- BMAD native conformance additionally runs through
  `cargo test --manifest-path tests/conformance/rust/Cargo.toml --locked` and the
  locked `tests/conformance/dotnet/Sapphirus.Contracts.Conformance.Tests.csproj`.
  Both replay the same cataloged BMAD fixtures and eight golden hash vectors used
  by the TypeScript lane.

The active `--typescript-only` check is read-only and neither constructs nor
reads native binding outputs. Cross-language generation fails closed when an
exact generator, reviewed lock, declared schema source, or checksum differs. It
emits public TypeScript shapes through `json-schema-to-typescript`, browser-safe Ajv 2020-12
standalone validators, a monolithic Typify Rust module, and Corvus's natural
multi-file C# tree. Every native generator runs twice from clean ignored roots;
only byte-identical normalized inventories can be synchronized. Every
generation/check/test path invokes the native
`typescript@7.0.2` compiler through `tsc`; there is no alternate compiler lane.
Generated files are never edited by hand.

Import `@sapphirus/contracts/validation` to parse an untrusted serialized
contract. `parseAndValidateContract` rejects duplicate keys, malformed Unicode,
unsafe integers, oversized input, unknown contract families, and schema drift
before returning a value. For BMAD contracts it also verifies purpose-separated
self hashes and repository-owned semantic invariants. Catalog parsing requires
the matching descriptor in `semanticContext`; Method-session parsing requires
the matching catalog. Domain authority and side effects remain handwritten host
responsibilities after this validation boundary.

The TypeScript 7 lane also generates structural types and standalone validators
for filesystem capability snapshots, safe contract errors, signed package
compatibility, and explicit remote-job handoffs. Package compatibility is inert
metadata: this package does not activate packages. Remote-job handoffs are inert
records: this package does not upload content, start remote work, import results,
or apply local changes. Semantic validation rejects noncanonical or overlapping
capability sets and broken immutable handoff chains.

Contract-error semantic validation also enforces renderer-safe message text. It
rejects Unicode `\p{C}` code points, including controls and invisible bidi
formatting, plus path-shaped disclosures such as Windows drive roots,
backslashes, UNC/device forms, `file://` references, and rooted POSIX paths.
Ordinary Unicode prose, ratios, and HTTPS help links remain valid. This is a
narrow message-safety boundary, not a substitute for upstream secret
classification or diagnostic redaction.

When `detailsRef` is present, the same semantic boundary rejects controls,
absolute local paths, UNC/device forms, and `file://` references. Intended
opaque references such as `cas://`, `azure-blob://`, and `https://` remain
valid; the reference is still non-authoritative and cannot expose filesystem
access to the renderer.

Call `parseAndValidateContract` first. BMAD semantics run automatically and fail
closed; the exported validators from `@sapphirus/contracts/semantics` remain
available for explicit diagnostics and for the non-BMAD semantic families.
Semantic validators assume their input has already passed the matching closed
schema; they do not grant authority or perform lifecycle transitions.

All nineteen public schema families and the shared `common` dependency are
reachable from the same deterministic internal-`$defs` bundle in Rust and C#.
The bundle rewrites only declared references, rejects source-set drift and
definition collisions, and is staging-only: it is not a public wire contract.
The domain-neutral qualification bundle and fixtures exercise closed objects,
unions, null versus absent, local references, numeric bounds, Unicode,
recursion, duplicate members, canonicalization, and purpose-separated hashes.

The five early BMAD families cover inert package descriptors, separate
installed-skill/help/agent-roster catalogs, Method session/checkpoint records,
inactive Builder drafts/revisions/analysis evidence, and validation reports.
Three additional sealed Help roots expose the untrusted proposal, canonical
host-owned recommendation, and canonical advance result without granting model
or lifecycle authority. Their fragment-aware transitive schema closures are
locked in `schema-lock.json` and emitted as generated TypeScript, Rust, and C#
constants. Recommendation and advance-result hashes use distinct reviewed
domains, while all three semantic entry points share the same control/bidi-safe
text predicate and strict UTC-instant checks for host records.
They deliberately contain no runner, generic workflow AST, memory/autonomy,
registration, evaluation, publication, promotion, activation, rollback, or
workspace-effect authority. Their cross-record ordering, source closure,
single-use context-decision, and source-hash rules remain handwritten semantic
checks after structural validation.

## Hashing

After schema and semantic validation, remove only the hash or signature fields
explicitly excluded by that schema and compute:

```text
sha256(UTF8("sapphirus:" + purpose + ":" + schemaMajor + "\n" + JCS(value)))
```

The line feed is part of the preimage. Serialized hashes are lowercase and use
the `sha256:` prefix.

## Compatibility

Contract epoch 1 writes major `v1` objects (and evidence event `v2`). Readers
must reject unknown schema majors and discriminators. Future retained-major
upcasters must be pure and cannot rewrite immutable source objects or hashes.
