# BMAD fixtures

This package freezes a small S0/S1 proof surface without treating upstream
prose or scripts as trusted executable code.

- `sealed-method-bmad-help.json` binds the real BMAD Method help skill by exact
  snapshot path, byte length, and SHA-256. It is a read-only direct-skill
  inspection fixture.
- `inactive-stateless-agent.json` and `inactive-simple-workflow.json` bind the
  real Builder entrypoints and small Sapphirus-owned candidate payloads. Their
  Build, Edit, and Analyze actions are intentionally inactive.

No setup, eval, scaffold, cleanup, hook, wake, dependency-install, or
candidate-provided script is run by this package. `pnpm verify` hashes files and
checks fixture invariants only. Descriptor parsing rejects duplicate JSON keys,
unknown fields and discriminators, malformed values, path escapes, and nested
authority-bearing keys. Source and payload identities are pinned independently
of the descriptor in separately reviewed policy constants. Changing content and
its descriptor claim alone therefore fails; updating a lock constant is an
explicit trust change that requires review.

`executionProfile` classifies upstream Method/Builder semantics; it grants no
runtime authority. Method remains sealed and read-only. Builder payloads remain
inactive drafts with only Build, Edit, and Analyze authoring actions. Convert,
evaluation, activation, script, process, and network capabilities are absent;
the explicit script/network fields are deny-only assertions.

`Build` is the desktop's normalized inactive-authoring label for this proof
surface. It does not replace the upstream Agent Builder's raw Create/rebuild
intent in a future source catalog. The simple workflow uses note 100's `inline`
execution classification; that classification is descriptive, not executable.
