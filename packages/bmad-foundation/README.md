# Sapphirus BMAD Foundation

This private package contains the first-party semantic foundation used by later
Sapphirus BMAD packets. It is repository-owned, relocatable, and deliberately
inactive.

BMAD-00 provides the independently reviewed source and instruction foundation:

- independently authored Method and Builder instruction data;
- exact source, legal, adoption, and managed-byte evidence;
- separate package/module, explicitly undeclared source-format, and exact source
  Node compatibility facts;
- fail-closed verification using Node built-ins.

BMAD-04 adds generated-contract-shaped normalized Method data and a hash-sealed
runtime manifest. The Method records remain sealed read-only data. Builder
records remain inactive authoring data with no registration, activation, model,
lifecycle, or effect authority. Development scripts and tests are excluded from
the desktop resource manifest.

Run `node ./scripts/verify.mjs` from this directory to verify the reviewed
source facts, adoption policy, runtime allowlist, and every managed byte.
