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

The reviewed archives carry no immutable Git metadata. Their byte identities,
declared package versions, member counts, runtime floors, licenses, trademarks,
and acquisition evidence are pinned in `source-lock.json`. Changed bytes are
rejected even when a folder or version label is unchanged. The earlier Method
snapshot identity remains recorded there only as superseded audit history.
The owner-attestation basis and the remaining tagged-release requirement for
public distribution remain governed by
`docs/adr/ADR-0008-verbatim-method-adoption.md`.
