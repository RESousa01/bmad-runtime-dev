# BMAD fixtures

This package freezes a small S0/S1 conformance surface without treating
upstream prose or scripts as trusted executable code. BMAD Method is the
application's foundational method, and BMAD Builder is the essential authoring
add-on for skills, workflows, and agents. These fixtures prove only a bounded
set of descriptor and draft invariants; runtime, catalog, model, and product
integration are separate implementation work.

- `sealed-method-bmad-help.json` records immutable provenance for the reviewed
  BMAD Method help skill and keeps the conformance fixture sealed and read-only.
- `inactive-stateless-agent.json` and `inactive-simple-workflow.json` record
  immutable provenance for reviewed Builder entrypoints and bind small,
  Sapphirus-owned candidate payloads. Their Build, Edit, and Analyze actions are
  intentionally inactive.

No setup, eval, scaffold, cleanup, hook, wake, dependency-install, or
candidate-provided script is run by this package. `pnpm verify` hashes files and
checks fixture invariants only. Descriptor parsing rejects duplicate JSON keys,
unknown fields and discriminators, malformed values, path escapes, and nested
authority-bearing keys.

Upstream archives and source files are not packaged, opened, or required by the
verifier. Each source record stores the project, version, license, reviewed
archive SHA-256, normalized archive-member path, source byte length, and source
SHA-256 as immutable provenance metadata. Only repository-owned draft payload
bytes are opened and hashed. Provenance and payload identities are also pinned
independently in reviewed policy constants, so changing a descriptor claim alone
fails; updating a lock constant is an explicit trust change that requires
review.

`distributionProfile`, `validationProfile`, and `executionProfile` classify the
reviewed upstream Method/Builder shapes and semantics; they do not imply that an
upstream source tree is packaged, and they grant no runtime authority. Method
remains sealed and read-only. Builder payloads remain inactive drafts with only
Build, Edit, and Analyze authoring actions. Convert, evaluation, activation,
script, process, and network capabilities are absent; the explicit
script/network fields are deny-only assertions.

`Build` is the desktop's normalized inactive-authoring label for this proof
surface. It does not replace the upstream Agent Builder's raw Create/rebuild
intent in a future source catalog. The simple workflow uses note 100's `inline`
execution classification; that classification is descriptive, not executable.

The test suite copies this package into a minimal checkout and verifies it there,
proving that conformance validation has no dependency on an external context
library or reviewed source tree.
