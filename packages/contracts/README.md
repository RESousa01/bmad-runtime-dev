# Sapphirus contracts

This package is the reviewed JSON Schema 2020-12 source and language-neutral
conformance suite for Sapphirus. It contains no lifecycle authority, policy
decision, filesystem access, executor dispatch, or persistence behavior.

## Commands

- `pnpm verify` and `pnpm verify:typescript` run the active TypeScript 7 lane:
  deterministic TypeScript/fixture regeneration checks, schema posture, TS7
  type checking, binding inventory/digest checks, and JavaScript conformance tests.
- `pnpm generate`, `pnpm check`, and `pnpm verify:cross-language` are frozen
  cross-language maintenance commands. Run them only after the native and .NET
  toolchain lanes are explicitly re-enabled.
- `pnpm test` exercises strict JSON parsing, RFC 8785 canonicalization,
  purpose-separated hashes, fixture validation, and delivery binding.

The active `--typescript-only` check is read-only and neither constructs nor
reads native binding outputs. Cross-language generation runs only after the
exact workspace install. It emits public
TypeScript shapes through `json-schema-to-typescript`, browser-safe Ajv 2020-12
standalone validators, and the cross-language bootstrap bindings recorded in
`schema-lock.json`. Every generation/check/test path invokes the native
`typescript@7.0.2` compiler through `tsc`; there is no alternate compiler lane.
Generated files are never edited by hand.

Import `@sapphirus/contracts/validation` to parse an untrusted serialized
contract. `parseAndValidateContract` rejects duplicate keys, malformed Unicode,
unsafe integers, oversized input, unknown contract families, and schema drift
before returning a value. Domain authority and side effects remain handwritten
host responsibilities after this structural boundary.

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

Call `parseAndValidateContract` first, then use the handwritten validators from
`@sapphirus/contracts/semantics`. Semantic validators assume their input has
already passed the matching closed schema; they do not grant authority or
perform lifecycle transitions.

Rust and C# generation for these four newly added families is intentionally
deferred. Their existing bootstrap bindings remain unchanged; `schema-lock.json`
records this partial language coverage so no consumer can mistake it for
three-language conformance.

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
