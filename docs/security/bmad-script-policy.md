# BMAD and Builder script policy

## Security boundary

Pinned BMAD Method and BMad Builder archives are retained as untrusted source
evidence. A script, executable bit, shebang, template, Markdown instruction,
manifest hook, or generated file never grants runtime authority.

Product side effects must cross the established boundary:

```text
typed plan
→ Airlock decision
→ exact approval when required
→ deterministic native execution
→ post-validation
→ durable evidence
```

The React renderer does not execute source scripts. Generated Builder output is
inert until validation, analysis, evaluation, review, approval, staging,
installation, atomic catalog activation, live conformance, and receipt all
succeed.

## Inventory and discovery

`packages/bmad-foundation/script-inventory.json` is the reviewed source-risk
inventory. Its rows are derived from the pinned source manifest and cover files
identified by:

- a script/code extension;
- an archive executable bit;
- a shebang;
- the manifest's `script_source` class;
- Dockerfile or Compose naming;
- a script or hook path; or
- a code-emitting template path.

Each row binds the source identity, version, path, SHA-256, owner, language,
imports, inputs and outputs, filesystem/network/process needs, environment and
secret access, determinism, platform assumptions, upstream tests, indirect
consumers, review flags, treatment, owner, and rationale.

Discovery is intentionally broader than the manifest's `script_source` class.
This catches sample scripts and executable templates whose retained-source class
is `test_or_sample_fixture`, `template_or_asset`, distribution metadata, or a
plain source resource.

## Treatments

Every discovered member has exactly one of these treatments:

- `port`: independently implement the reviewed semantics behind typed
  Sapphirus runtime/native ports. The retained script never executes.
- `sandbox`: behavior cannot reasonably be reduced to a pure port and may only
  be implemented through a future governed sandbox plan with Airlock,
  deterministic limits, validation, and evidence. Classification is not
  permission to execute it.
- `fixture_only`: retain only for tests, characterization, or sample evidence.
  It is excluded from installed/runtime packages.
- `distribution_only`: retain only for provenance, documentation, repository
  automation comparison, or upstream distribution analysis.
- `reject`: the behavior is unsafe, unnecessary, or violates Sapphirus
  ownership. Runtime and generated packages must not reference it.

There is no default treatment. Adding a newly discovered path that does not
match one and only one reviewed decision rule fails regeneration.

## Required explicit review

Inventory rows expose review flags for filesystem access, shell or subprocess
execution, dynamic code loading/evaluation, network access, package
installation, archive extraction, path traversal/canonicalization, process
trees, schedule creation, secret-like data, and environment access.

These flags are evidence for review; they are never authority. A future port or
sandbox implementation must independently validate inputs, paths, providers,
processes, network destinations, secrets, approvals, cancellation, recovery,
and receipts under the relevant runtime contract.

## Verification and drift control

Run:

```powershell
node packages/bmad-foundation/scripts/classify-scripts.mjs --strict --check
```

Strict checking:

1. rediscovers candidates from `source-manifest.json` and the retained source;
2. requires one unique, reviewed inventory row per candidate;
3. requires the inventory SHA-256 to match the pinned manifest member;
4. compares every derived field and treatment decision with the canonical
   inventory;
5. scans runtime/package surfaces for direct references to rejected source
   scripts; and
6. exits nonzero on missing, duplicate, invalid, stale, or prohibited rows.

After an intentional pinned-source review, regenerate for human diff review:

```powershell
node packages/bmad-foundation/scripts/classify-scripts.mjs --write
node packages/bmad-foundation/scripts/classify-scripts.mjs --strict --check
```

Regeneration is not approval. Reviewers must inspect new candidates, decision
rules, risk flags, consumers, and rationales before accepting the inventory.

