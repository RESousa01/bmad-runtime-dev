# Upstream source libraries

## OpenCode

Forge's renderer migration is based on the complete OpenCode source checkout at
`library/opencode`.

- Repository: `https://github.com/anomalyco/opencode`
- Pinned revision: `0a601cf334b9a83cc2854108a2b860f25e6e7e8e`
- License: MIT; see `THIRD_PARTY_NOTICES.md`

The checkout is intentionally ignored by the parent repository so its own Git
history remains intact. Forge-owned integration code and source adaptations are
tracked in this repository. The migration keeps OpenCode's renderer structures
for sessions, filesystem navigation, Git review, context metrics, and raw
messages, while Forge's existing Rust commands remain authoritative for BMAD,
workspace access, review consent, and governed edits.
