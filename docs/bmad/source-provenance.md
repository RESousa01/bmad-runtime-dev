# BMAD source provenance

Forge binds BMAD compatibility to archive bytes, not directory names, version
strings, normalized packages, or generated projections. The machine-readable
authority is
[`packages/bmad-foundation/source-lock.json`](../../packages/bmad-foundation/source-lock.json).

## Pinned sources

| Source | Version | Canonical archive | SHA-256 | Files | Skills | Runtime floor |
| --- | --- | --- | --- | ---: | ---: | --- |
| BMAD Method | 6.10.0 | `BMAD-METHOD-main(1).zip` | `2993a38a36a9586d16a62a4eacb3400db25dd1ea594bbfe4682789d7f95ac02b` | 597 | 57 `SKILL.md`; 48 primary | Node >=20.12.0 |
| BMad Builder | 2.1.0 | `bmad-builder-main(1).zip` | `d3c70744a9875623b01856cc907cf558324bacc920f0d860c36ad2788a4d2852` | 299 | 13 `SKILL.md`; 5 production | Node >=22.0.0 |

The supplied local files omit the download suffix `(1)` but match the canonical
hashes byte for byte. The hash is authoritative; both filenames are recorded
so a rename cannot be mistaken for a different source.

The previous Method hash
`a7c049038099b99081fbd03d22c6a5180edd88dee656bb37c4276b1cc31b4a32`
is superseded. It remains in the lock as historical migration evidence and is
not an accepted target identity.

## Verification

Set `BMAD_METHOD_ARCHIVE` and `BMAD_BUILDER_ARCHIVE` to the supplied archive
paths, then run:

```text
node packages/bmad-foundation/scripts/verify-source-identities.mjs --check
```

The verifier checks the archive SHA-256, byte length, regular-file count,
`SKILL.md` count, package name/version/runtime floor, license, trademark, the
semantic ledger, and this provenance record. Missing archives fail the full
check; `--metadata-only` exists only for deterministic repository unit tests.

## Upgrade intake

Changed bytes are rejected until a new source identity is reviewed. Every
upgrade record must include the source diff, semantic and compatibility impact,
migration, rollback, and approval. Imported source remains untrusted input and
does not acquire filesystem, process, network, model, memory, installation, or
publication authority.
