# Sapphirus BMAD Foundation

This private package contains the first-party semantic foundation used by later
Sapphirus BMAD packets. It is repository-owned, relocatable, and deliberately
inactive.

BMAD-00 provides only:

- independently authored Method and Builder instruction data;
- exact source, legal, adoption, and managed-byte evidence;
- separate package/module, explicitly undeclared source-format, and exact source
  Node compatibility facts;
- fail-closed verification using Node built-ins.

It does not provide contracts, normalized descriptors, a runtime manifest,
package registration, model access, lifecycle transitions, or effect authority.
The Method records are sealed read-only data. Builder records describe inactive
authoring drafts only.

Run `node ./scripts/verify.mjs` from this directory to verify the reviewed
source facts, adoption policy, runtime allowlist, and every managed byte.
