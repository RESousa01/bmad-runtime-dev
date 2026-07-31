# External source-library boundary

This directory is reserved for developer-supplied, ignored source checkouts used for bounded
comparison and research. External repositories are not Forge runtime dependencies, build inputs,
or current product authority.

## OpenCode

The dated Forge v2 audit reviewed OpenCode commit
`0a601cf334b9a83cc2854108a2b860f25e6e7e8e`. That checkout is **not present in the current
repository**, and there is no approved immutable source lock or storage locator for it.

Do not assume `library/opencode` exists, fetch a moving branch as a substitute, or cite paths from
an unverified checkout. See the maintained
[OpenCode source analysis](../docs/reference/source-analysis/opencode.md) for the accepted concepts,
rejected integration choices, and current reproducibility blocker.

## Rules

- Never package, execute, or import an external checkout into Forge production code.
- Record repository URL, immutable revision, license, acquisition evidence, and archive hash before
  relying on path-level analysis.
- Keep Forge-owned adaptations in maintained first-party source with independent tests.
- Re-derive useful interaction patterns within Forge’s Rust-host authority boundary.
- Treat missing or mutable source identity as blocked evidence, not as permission to guess.

The externalization and deletion gate is defined in the
[source-intake policy](../docs/reference/source-intake-policy.md).
