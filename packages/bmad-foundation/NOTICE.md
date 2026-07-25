# Third-party notices

This package incorporates material from BMAD Method 6.10.0 and BMAD Builder
2.1.0 (module 1.0.0). Both are licensed under the MIT License, and the exact
license text is retained in `licenses/BMAD-METHOD-MIT.txt` and
`licenses/BMAD-BUILDER-MIT.txt`.

Material is carried under one of two recorded treatments, per member, in
`semantic-source-ledger.json`:

- **adopt** — the upstream text is used as licensed, with its copyright and
  permission notice retained.
- **adapt** — Forge authored a projection that diverges from upstream,
  because an upstream step relies on an authority this runtime withholds
  (`external_handoff`, `file_read`, `file_write`, `process`, `subagent`, `web`
  are blocked and host-replaced). Adapted text is never presented as upstream
  text.

Adopting an instruction body does not adopt its authority: blocked tool intents
remain blocked, Builder output remains inert, and everything reaching a model
passes consent and egress review.

BMAD names and marks remain the property of BMad Code, LLC and are **not**
licensed under the MIT License. They are used here only descriptively, to
identify the reviewed sources and to state compatibility. Forge does not
use a BMAD mark as a product, service, company, or domain name, and claims no
endorsement, approval, or certification by BMad Code, LLC.

The reviewed archives carry no immutable Git metadata. Their identity rests on
the repository maintainer's attestation that each was downloaded directly from
its official upstream repository, together with the release version declared
inside the archive and a content hash for every adopted member. That basis is
recorded as an owner-granted exception in `semantic-source-ledger.json` and
decided in `docs/adr/ADR-0008-verbatim-method-adoption.md`. What it does not
provide is reproducibility: the adopted bytes must be pinned to an immutably
identified upstream release before any public distribution.
