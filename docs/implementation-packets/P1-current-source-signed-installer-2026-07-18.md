# Implementation packet: P1 — current-source signed Windows installer

## Authority and intent

- Owning authority: repository maintainer (RodrigoSousa0); organization release administrators own the protected signing environment, certificate, timestamp service, and release approval.
- User-visible outcome: an exact-current-source Windows installer whose executable and NSIS package are Authenticode-signed and timestamped, lifecycle-qualified on an isolated Windows account, traceable to one clean Git revision, and eligible for an explicitly governed update channel.
- Contracts read: `crates/desktop-app/tauri.conf.json`, `tools/build-signed-windows-installer.ps1`, `tools/qualify-windows-installer.ps1`, `.github/workflows/release-dry-run.yml`, `crates/desktop-app/src/update.rs`, and the P0 review ledger.
- Non-goals: D3 recovery closure (P2), Azure model-path productionization (P3), broader BMAD workflows (P5), public distribution, automatic update installation, or storing signing private keys in GitHub/repository files.
- Stop conditions: signing identity is not organization-controlled; certificate or timestamp verification is not valid; source revision is dirty or ambiguous; versions disagree; clean-account lifecycle or upgrade proof fails; release evidence cannot be bound to the exact artifact hashes.

## Delivery slices and exit gates

1. **P1-A — fail-closed signed qualification lane (started here).** Add a protected, manually dispatched, organization-runner lane that builds with the repository signing script, requires a same-publisher signed prior installer on a separate non-signing qualification runner, and proves signature, timestamp, install, upgrade, launch, uninstall, install-root/uninstall-registration removal, and exact bundled BMAD payload. Record hash-bound signed-build and lifecycle evidence as short-lived CI artifacts.
2. **P1-B — release metadata and provenance.** Replace duplicated version/path literals with one tested resolver; record exact source revision, toolchain versions, dependency locks, artifact hashes, publisher/timestamper identities, and generate an SBOM plus GitHub build attestation. Reject version drift and dirty sources.
3. **P1-C — independent clean-machine qualification.** Run the signed artifact on a disposable organization-managed Windows image/account, including first install, prior-version upgrade, launch smoke, uninstall, residue inspection, and negative tests for unsigned, wrong-publisher, and untimestamped artifacts.
4. **P1-D — governed updater artifacts.** Define the signed update manifest/key authority, immutable release channel, downgrade/rollback policy, and staged rollout. Only then enable `createUpdaterArtifacts` in a release-only overlay and expose non-status updater actions.
5. **P1-E — release rehearsal and handoff.** Independently review the full release delta, rehearse promotion and rollback, retain durable provenance/SBOM/lifecycle evidence, and document administrator/user recovery paths.

P1 is complete only when P1-A through P1-E are green on the same committed revision and exact signed artifacts. The workflow landing alone does not make the installer releasable: the protected environment, signing runner, certificate, prior artifact, clean-machine run, SBOM/attestation, updater trust root, and rollback rehearsal remain required evidence.

## Tests first

- Success fixture: the repository boundary check requires the signed workflow, organization signing environment/runner, signed build script, prior-version upgrade, valid-signature gate, and both evidence files.
- Negative/bypass fixture: missing workflow, mutable action reference, unauthorized ref, absent organization gates, unsigned or wrong-publisher lifecycle input, absent prior installer, dirty source tree, version mismatch, evidence hash mismatch, or non-timestamped signature fails closed.
- Failure/recovery fixture: signing or timestamp failure publishes no candidate; qualification cleans up a partial installation; release administrators can disable either organization lane variable without changing source.
- Compatibility or migration fixture: exact prior-version installer upgrades to the current version, retains the expected installed payload, and uninstalls without product residue.

## Change and rollback

- Files/surfaces allowed for P1-A: signed-release workflow, signing/lifecycle scripts and their tests/guards, release documentation, and evidence schemas. Product updater configuration remains unchanged and fail closed.
- Disable or rollback path: set `SAPPHIRUS_SIGNING_LANE_ENABLED` false to disable the signed lane immediately; revert the P1-A workflow/guard commit to remove it. No updater artifact or product behavior is enabled by this slice.
- Observability/evidence: boundary red/green proof; source verification; workflow run URL and commit; signed-build JSON; lifecycle JSON; artifact hashes; independent review ledger.

## Review ledger

- Implementer full-diff review: executed 2026-07-18 for the P1-A workflow, signing/lifecycle scripts, boundary guard, and release claims.
- Independent bug/security review: executed 2026-07-18. Initial blockers were dispatcher-to-PowerShell injection, caller-selected prior installer execution on the signing runner, mutable action tags, and insufficient ref restriction. Corrections moved lifecycle execution to a separate qualification runner, validate prior signature/timestamp/same publisher before process creation, pass input through the environment, restrict signing to exact `main` HEAD, and pin every action to a reviewed commit. Re-review found and closed one artifact evidence-path defect; final verdict: PASS with no P0/P1 blockers.
- Commands executed:
  - `pnpm verify:boundaries` — observed red on the missing signed workflow, then green after P1-A implementation and after review corrections.
  - PowerShell AST parsing for both release scripts — green; an invalid signing thumbprint fails before build or evidence creation.
  - `pnpm verify:source` with Node 24.18.0 / pnpm 11.12.0 — green, including renderer 296/296 and production build.
  - First-party secret scan — 3,324 files green; `git diff --check` — green.
- Checks skipped and reason: signed build and lifecycle workflow require the protected organization signing runner/certificate and a prior installer run; they cannot be honestly reproduced in this developer checkout.
- Remaining risks: organization runner/environment provisioning, external protected-environment branch policy, certificate custody/rotation, timestamp service policy, approved prior-run provenance, SBOM/attestation selection, structured workflow-mutation tests, updater trust-root design, full application-data residue inspection, rollback rehearsal, and uninstall/offboarding data-retention policy remain open P1 work.
