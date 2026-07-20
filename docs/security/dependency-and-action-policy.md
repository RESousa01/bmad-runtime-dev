# Dependency and GitHub Action policy

Readiness-program Task 3. Every GitHub Action reference in every workflow
must be an immutable 40-character commit SHA that was resolved from an
official upstream tag and reviewed. `tools/check-boundaries.mjs` scans all
workflow files and fails on any mutable reference; the red fixtures live in
`tools/check-boundaries.test.mjs`.

## Reviewed action pins (2026-07-20)

Resolution method: `gh api repos/<owner>/<repo>/commits/<tag>` (peeled
commit), then confirmed the commit is reachable from the official tag list
of the upstream repository.

| Action | Reviewed tag | Pinned commit |
|---|---|---|
| actions/checkout | v4.3.1 | `34e114876b0b11c390a56381ad16ebd13914f8d5` |
| pnpm/action-setup | v4.3.0 (peel of `v4`) | `b906affcce14559ad1aafd4ab0e942779e9f58b1` |
| actions/setup-node | v4.4.0 | `49933ea5288caeca8642d1e84afbd3f7d6820020` |
| actions/setup-dotnet | v4.3.1 | `67a3573c9a986a3f9c594539f4ab511d57bb3ce9` |
| dtolnay/rust-toolchain | reviewed master commit, 2026-07-16 | `2c7215f132e9ebf062739d9130488b56d53c060c` |
| actions/upload-artifact | v4.6.2 | `ea165f8d65b6e75b540449e92b4886f43607fa02` |
| actions/download-artifact | v4.x (release lane, previously reviewed) | `d3f86a106a0bac45b974a628896c90dbdf5c8093` |
| actions/attest-build-provenance | previously reviewed (release lane) | `e8998f949152b193b063cb0ec769d69d929409be` |
| actions/attest | previously reviewed (release lane) | `a1948c3f048ba23858d222213b7c278aabede763` |
| anchore/sbom-action | v0.24.0 | `e22c389904149dbc22b58101806040fa8d37a610` |
| anchore/scan-action | v6.5.1 | `1638637db639e0ade3258b51db49a9a137574c3e` |
| azure/login | v2.3.0 | `a457da9ea143d694b1b9c7c869ebb04ebe844ef5` |

### Corrected finding

The release workflows previously pinned
`pnpm/action-setup@f40ffcd9367d9f12939873eb1018b921a783ffaa`. That SHA does
**not exist** in the upstream repository (`gh api …/commits/<sha>` returns
404) — any release run would have failed at action resolution. It was
replaced everywhere with the verified `v4.3.0` peel above. Lesson enforced:
a pin is reviewed only when the commit is fetched from upstream at review
time, never transcribed from another workflow.

## Base container images

`services/desktop-support-api/Dockerfile` pins both stages by digest,
resolved from the Microsoft Container Registry manifest API (2026-07-20):

| Image | Tag | Digest |
|---|---|---|
| mcr.microsoft.com/dotnet/sdk | 10.0.100-alpine3.22 | `sha256:7d98d5883675c6bca25b1db91f393b24b85125b5b00b405e55404fd6b8d2aead` |
| mcr.microsoft.com/dotnet/aspnet | 10.0.0-alpine3.22 | `sha256:049f2d7d7acfcbf09e1d15eb4faccec6453b0a98f0cb54d53bcbdc3ed91e96c8` |

The `support-container` CI job fails hard if a digest placeholder remains,
then builds, SBOMs (Anchore), and scans (Grype, fail on High/Critical).
Digest updates require re-resolving from MCR and re-running the container
scan; never copy a digest from a forum, blog, or another repository.

## Advisory automation

`security-nightly.yml` runs nightly (02:30 UTC) and on dispatch:

- Hosted `advisories` job (ubuntu): boundary scan, secrets check, and
  `cargo deny check advisories bans licenses sources` with cargo-deny
  pinned at exactly `0.19.4`. Failure policy: Critical/High advisories,
  banned licenses, unapproved git sources, and prohibited duplicate
  cryptographic/runtime packages fail the run.
- The Windows-native replica stays gated on
  `SAPPHIRUS_NATIVE_LANE_ENABLED` until the organization runner lane is
  approved.

`pnpm audit --prod` is intentionally **not** enabled: it transmits the
dependency graph to the configured registry. It may be added only after the
repository owner approves that egress, or an organization-mirrored advisory
database is configured instead.

## Update procedure

1. Resolve the new tag upstream and read the action's `action.yml` diff.
2. Update the pin (SHA + tag comment) in every workflow that uses it.
3. Update this table in the same commit.
4. `node --test tools/check-boundaries.test.mjs && pnpm verify:boundaries`
   must pass; CI re-runs the same guard.
