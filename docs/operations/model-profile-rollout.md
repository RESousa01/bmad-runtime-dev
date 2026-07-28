# Model profile rollout

Date: 2026-07-28

A model profile states what the organization *permits* and how Forge uses it.
The deployments that physically exist are discovered at runtime; the profiles
that may be selected are published here, as versioned key-values in Azure App
Configuration. This runbook covers authoring, validating, dry-running,
publishing, verifying, and rolling back a profile set.

`packages/contracts/schemas/model-profile.schema.json`
(`sapphirus.model-profile.v1`) is the authority on profile shape.
`tools/foundry-profiles/profile.schema.json` mirrors it field-for-field for the
operator-authored document, and
`tools/foundry-profiles/validate-profiles.test.mjs` fails if the two ever
drift.

## What gets published

| Key | Value |
|---|---|
| `desktop-support:model-profile:<profileId>` | The RFC 8785 canonical JSON of one profile. |
| `desktop-support:model-profile-registry:version` | The registry record: `schemaVersion`, `registryVersion`, `registryHash`, `capabilityHash`, `profileIds`. |

Two invariants make a reader safe:

1. **The registry-version key is written last.** A reader that takes
   `profileIds` from the registry record never observes a half-published set.
2. **Profile keys are never deleted by this tool.** The registry record's
   `profileIds` is the manifest; a profile key absent from it is inert. Removing
   a key is a separate, deliberate act.

Each publish also writes an immutable copy of the whole set under the archive
label `<label>-v<registryVersion>`. That archive is what a rollback reads. No
reader watches the archive labels.

## Three hashes, and what each one means

| Hash | Covers | Use |
|---|---|---|
| Profile hash | One whole profile. | Detect that a single profile changed. |
| `registryHash` | The registry version plus every profile hash. | Detect that the published set changed at all. |
| `capabilityHash` | Only what the organization *permits*: deployment, expected model, allowlists, modalities, required capabilities, retention, regions, qualification status. | Tell a price or budget change apart from a change in what is allowed. |

A price edit moves `registryHash` and leaves `capabilityHash` alone. Adding a
purpose to an allowlist moves both. All three use the repository canonicalizer
at `packages/contracts/scripts/lib/canonical-json.mjs` and the
`sha256:<64 lowercase hex>` serialization used everywhere else in the
repository.

## 1. Author

Copy `infra/desktop-support/model-profiles.example.json` and edit it. The
example is entirely synthetic: its deployment names all end in
`do-not-deploy`, its publisher and model names begin with `Example`, its
currency is the ISO 4217 test code `XTS`, and its region is `exampleregion`.
Every one of those must be replaced.

Rules the validator enforces, so you do not have to remember them:

- `profileId` is stable and independent of the deployment name. Replacing a
  deployment must not invalidate a persisted user selection, so never rename a
  profile to match a deployment.
- `deploymentName` must exist in the deployment inventory, compared ordinally.
  A case-only difference is refused rather than silently matched.
- `expectedModel` must equal the publisher, name, and version that the
  inventory reports for that deployment. Two profiles that share a deployment
  must declare the same model identity.
- Two *visible* profiles may not share a display name. A hidden profile
  (`"visible": false`) may reuse one; that is how an evaluation-only profile
  coexists with the profile it replays.
- A variant may narrow the output budget; it may never widen it past
  `limits.maxOutputTokens`.
- `qualification.status` must be `qualified`, `qualifiedAt` must be set, and
  `evidenceHash` must be a real digest. A constant such as `sha256:000…0` is
  refused as a placeholder, not accepted as evidence.
- `registryVersion` must be strictly greater than the version currently
  published.

## 2. Validate

```
node tools/foundry-profiles/validate-profiles.mjs \
  --profiles infra/desktop-support/model-profiles.example.json \
  --deployments artifacts/foundry/prerequisites.json
```

The report is printed to stdout; a one-line summary goes to stderr. Exit `0`
means publishable, `2` means refused with every reason in `report.violations`,
`1` means the input could not be read at all.

The deployments file is the report written by
`tools/foundry-smoke/check-prerequisites.ps1` (see
[foundry-prerequisites.md](foundry-prerequisites.md)). **A report whose
`ready` is `false` is refused outright**, and the refusal names the inventory's
own blockers. There is no way to validate profiles against an inventory that
has not been proved. The canonical `sapphirus.foundry-deployment.v1` inventory
is also accepted; because it carries capability booleans rather than the
redacted key names, it additionally enforces that a deployment advertises every
capability a profile requires.

Useful flags:

- `--current <state.json>` produces a redacted diff against what is published
  today, and enforces the registry-version increase. The diff carries profile
  identifiers and digests only.
- `--out <report.json>` writes the report to a file as well.
- `--emit-plan <plan.json>` writes the canonical values to publish. A plan is
  emitted only for a document that validates.

## 3. Dry run

```
pwsh -NoProfile -File tools/foundry-profiles/publish-profiles.ps1 -WhatIf
```

`-WhatIf` is the default way to inspect a change and performs **no writes at
all**. It reads the current state, validates the document against it, and
prints the exact ordered list of writes it would make. Point it at your own
files with `-ProfilePath`, `-DeploymentPath`, and `-Label`; the report lands at
`-OutputPath`, which defaults to `artifacts/foundry/infra/profile-dry-run.json`.

Reading the store requires `FORGE_APP_CONFIGURATION_ENDPOINT` and an Azure CLI
session with App Configuration data access. Without it the dry run still
validates the document and reports `store_endpoint_missing`, because it could
not prove what is published today.

Read the `plannedOperations` list before going further. It should end with a
single `set_registry` at the active label, and every `set_profile` should carry
`condition: if-match` for a key that already exists.

## 4. Publish

```
$env:FORGE_MODEL_PROFILE_PUBLISH_APPROVAL = 'approved'
pwsh -NoProfile -File tools/foundry-profiles/publish-profiles.ps1 `
  -ProfilePath <your-profiles.json> `
  -DeploymentPath <your-prerequisites.json> `
  -Label production
```

A write requires approval on top of dropping `-WhatIf`: either the `-Approve`
switch, or `FORGE_MODEL_PROFILE_PUBLISH_APPROVAL` set to exactly `approved` for
CI. Without one, the run stops with `publish_approval_missing` and writes
nothing.

Every write is conditional. A key that exists is written with `If-Match` on the
ETag observed while planning; a new key is written with `If-None-Match: *`. If
another operator changed anything after this run planned its writes, the
publish stops with `concurrent_update`, and — because the registry key is last
— the previously published set is still the one readers see. Re-run the dry run
to see what changed, then publish again.

Exit `0` means the set was applied. Exit `2` means it was refused or blocked;
`report.blockers` says which.

## 5. Verify

After a publish, confirm three things in the report:

1. `appliedOperations` has the same length as `plannedOperations`, and its last
   entry is `kind: set_registry` at the active label.
2. `registryVersion` is the version you authored, and
   `currentRegistryVersion` was strictly lower.
3. `capabilityHash` changed only if you intended to change what is permitted.

Then re-run the dry run. A clean second dry run reports every profile as
`unchanged` in the diff and refuses to republish the same `registryVersion`.

## 6. Roll back

```
pwsh -NoProfile -File tools/foundry-profiles/publish-profiles.ps1 `
  -RollbackToRegistryVersion <prior version> `
  -Label production `
  -Approve
```

Rollback reads the archive at `<label>-v<prior version>`, restores those exact
values to the active label, and publishes them under a **new, higher** registry
version. The content goes back; the version number never does. That keeps
downgrade detection meaningful for every reader — a reader that has seen
version *n* never has to accept something numbered lower.

The target must be strictly lower than what is published now
(`rollback_target_not_prior`), and its archive must be complete
(`rollback_target_missing`, `rollback_profile_missing`).

## What this tool cannot do

By construction, not by convention:

- It cannot create, update, or delete a model deployment.
- It cannot create or modify a role assignment.
- It cannot delete a key.
- It cannot write unconditionally.
- It cannot touch a key outside `desktop-support:model-profile:<profileId>` and
  `desktop-support:model-profile-registry:version`.

Every store interaction passes `Assert-ProfileStoreOperation`, whose allowlist
is closed over `list`, `get`, and `set`, and the only Azure CLI commands
reachable are `account show` and `account get-access-token`. The tests in
`publish-profiles.Tests.ps1` assert each of these refusals.

## Report safety

Neither report ever contains a token, connection string, endpoint, profile
value, or ETag in clear text. Endpoints and ETags appear only as SHA-256
labels, and the diff carries profile identifiers and digests. Do not add
anything to these reports that is not a boolean, a stable code, an identifier,
or a digest.

## Running the tests

```
node --test tools/foundry-profiles/validate-profiles.test.mjs
pwsh -NoProfile -File tools/foundry-profiles/publish-profiles.Tests.ps1
```

Both run fully offline. The publish tests drive an in-memory App Configuration
double injected through `-StoreInvoker`, which enforces real `If-Match` and
`If-None-Match` semantics. The App Configuration REST boundary in
`Invoke-ProfileStoreRequest` is therefore **not** exercised by these tests and
has never been run against a live store; the first real publish should be a
dry run against a non-production label.

## Current status

No Foundry deployment has been proved. `artifacts/foundry/prerequisites.json`
reports `"ready": false` with six blockers, so
`artifacts/foundry/infra/profile-dry-run.json` records a refusal rather than a
plan. Publishing is blocked until a fresh prerequisite report says
`"ready": true` and real profiles replace the synthetic example.
