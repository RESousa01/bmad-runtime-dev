# D2-D BMAD Help Desktop Composition Implementation Plan

> **Status:** Approved implementation direction
> **Date:** 2026-07-16
> **Base:** `codex/d2-ai-request` at `3108bf37`
> **Execution rule:** Test first, keep production fail-closed, and commit after every numbered checkpoint.
> **Paused checkpoint:** Work stopped on 2026-07-16 before D3 integration, staging, commit, or
> deployment. See `docs/implementation-packets/D2-D-desktop-composition-2026-07-16.md` for the
> exact verified D2-D gates, uncommitted-work warning, and required resume order.

## Goal

Turn the durable but deliberately inert BMAD Help run into one honest desktop vertical:

1. create the local Method run;
2. compile the exact sealed Help invocation;
3. show the exact outbound, redacted context;
4. approve or cancel one short-lived decision;
5. consume the approval exactly once;
6. dispatch through an explicitly selected model composition;
7. verify exact response bytes and the complete receipt;
8. atomically materialize and persist the canonical BMAD recommendation;
9. recover the sanitized completed projection after restart.

The default/production build remains offline until D2-E supplies real tenant policy, entitlement,
support-plane, and signed receipt verification. A deterministic development model is available only
through an explicit Cargo feature and is visibly labelled as non-production.

## Locked public command contract

All commands continue through the single validated `host_dispatch` Tauri boundary.

- `model.auth.status`
- `model.auth.sign_in`
- `model.auth.sign_out`
- `bmad.help.prepare`
- `bmad.help.approve`
- `bmad.help.cancel`
- `bmad.help.submit`
- existing `bmad.help.latest`

BMAD mutation payloads carry only the current `workspaceId`, `workspaceGrantEpoch`, and the exact
displayed `manifestHash`/opaque `decisionId` where relevant. The renderer cannot select provider,
model, deployment, region, schema, package, capability, receipt, result, or authority fields.

The existing D1 `context.preview` contract remains local-only and unchanged.

## Locked bridge mapping

The host-only D2-to-Method bridge validates the pending decision, manifest, invocation binding,
session authority, and compiled Method binding before producing `MethodContextDecision`:

- `decision_id` = exact D2 decision ID;
- `manifest_hash` = exact `ContextEgressManifest.manifest_hash`;
- `consent_hash` = exact D2 `consent_disclosure_hash` reviewed by the user;
- `context_digest` = `canonical_hash("bmad-help-reviewed-context", 1, ordered outbound item IDs,
  content hashes, byte counts, manifest hash)`;
- `binding_hash` = compiled `MethodExactBinding.binding_hash()` (not the D2 binding hash);
- `reviewed_at` = D2 decision issuance time.

The D2 binding hash is retained separately and enters `MethodAdvanceRequest` and the Method/model
bridge hash. Golden tests must reject substitution of any bridge input.

## State and retention rules

- At most one active Help review exists for the single desktop renderer/workspace.
- Preparing a replacement cancels/drops the prior authority; it never silently reuses approval.
- Sign-out, renderer rebind, workspace/grant change, run change, manifest change, expiry, recovery,
  and process restart invalidate in-memory approval.
- Consent is consumed before dispatch and never resurrected after offline, cancellation, transport,
  output, receipt, or persistence failure.
- Context and raw model bytes never enter the general 2,048-entry reply cache.
- The exact verified payload stays opaque in native memory. Only a closed canonical recommendation
  and metadata-only receipt summary cross to the renderer.
- Completed renderer projection bytes are retained in the encrypted store in the same transaction
  as the raw proposal, canonical recommendation, completed aggregate, checkpoint, evidence, and
  outbox rows.

## Checkpoint 1: Restore a buildable desktop package

**Files**

- Create `crates/desktop-app/icons/app-icon.svg`
- Generate `crates/desktop-app/icons/icon.ico` and the Tauri-required PNG sizes
- Modify `crates/desktop-app/tauri.conf.json` only if the generated icon inventory must be explicit

**RED already observed**

```powershell
cargo test -p desktop-app --lib
```

Fails because `crates/desktop-app/icons/icon.ico` is absent.

**Implementation**

- Add an original Sapphirus geometric mark as the tracked SVG source.
- Use the pinned Tauri icon generator to create deterministic bundle assets.
- Do not copy the untracked icon from the dirty primary checkout.

**Proof**

```powershell
cargo test -p desktop-app --lib
cargo check -p desktop-app --all-features --locked
```

**Commit:** `build(desktop): track qualified application icons`

## Checkpoint 2: Preserve exact verified model bytes

**Files**

- Modify `crates/desktop-cloud/src/model.rs`
- Modify `crates/desktop-cloud/src/composition.rs`
- Modify `crates/desktop-cloud/tests/model_verification.rs`
- Modify `crates/desktop-cloud/tests/composition_modes.rs`

**RED tests**

- Verified output returns the exact original UTF-8 payload bytes after schema and receipt checks.
- Debug and Serde cannot reveal/serialize the opaque payload or receipt proof.
- JSON reformatting, key reordering, or Unicode escape substitution cannot replace retained bytes.
- The deterministic transport accepts a caller-supplied bounded fixture only behind
  `deterministic-fake`; the default transport remains absent/offline.

**Implementation**

- Refactor `VerifiedModelOutput` into an opaque trusted-host type with getters, a redacted `Debug`,
  no `Serialize`, parsed typed value, exact `Arc<[u8]>`, and complete verified receipt.
- Add a feature-gated deterministic `send_fixture` helper that still creates a fully bound raw
  response and must pass normal response verification.

**Proof**

```powershell
cargo test -p desktop-cloud --all-features
cargo test -p desktop-cloud
cargo clippy -p desktop-cloud --all-targets --all-features --locked -- -D warnings
```

**Commit:** `refactor(d2): retain opaque verified response bytes`

## Checkpoint 3: Bound consent memory and expose sealed bridge evidence

**Files**

- Modify `crates/desktop-egress/src/consent.rs`
- Modify `crates/desktop-egress/tests/consent.rs`
- Add `crates/desktop-app/src/bmad_model/bridge.rs`
- Add `crates/desktop-app/src/bmad_model/bridge_tests.rs` or colocated tests

**RED tests**

- Decision memory is bounded and prunes expired terminal entries without permitting ID replay.
- A live pending decision exposes only an opaque host evidence view needed by the bridge.
- The bridge produces the locked mapping above.
- Any manifest, item order/content, disclosure, D2 binding, session authority, Method binding, ID,
  or timestamp substitution fails closed.

**Implementation**

- Add a fixed-capacity, time-pruned ledger policy; never use unbounded terminal-state growth.
- Add a non-serializable `ContextDecisionEvidence` view with private fields and narrow getters.
- Implement the host-only bridge in `desktop-app`; do not add a runtime-to-egress dependency cycle.

**Proof**

```powershell
cargo test -p desktop-egress --test consent
cargo test -p desktop-app --lib bmad_model::bridge
cargo clippy -p desktop-egress -p desktop-app --all-targets --all-features --locked -- -D warnings
```

**Commit:** `feat(d2): seal the Method consent bridge`

## Checkpoint 4: Make completed Help projection durable

**Files**

- Modify `crates/desktop-store/src/bmad_method.rs`
- Modify `crates/desktop-store/src/lib.rs` exports as needed
- Modify `crates/desktop-store/tests/bmad_method_store.rs`
- Modify `crates/desktop-ipc/src/bmad.rs`
- Modify `crates/desktop-ipc/src/lib.rs`
- Add/modify focused BMAD projection tests

**RED tests**

- Finalization atomically retains a bounded renderer-safe completion projection.
- A failed finalization leaves no projection, result, checkpoint, evidence, or outbox partial state.
- Restart recovers the exact authenticated completed projection, not the original
  `created_unbound` projection.
- Tampering, scope/run/session substitution, oversized bytes, legacy rows, and projection gaps fail
  closed without exposing canonical authority records.

**Implementation**

- Extend the authenticated internal Help-run receipt with the exact Method scope needed to reload
  the session; it is never serialized to the renderer.
- Add a completion projection payload kind and binding hash.
- Store it inside `finalize_bmad_help`'s existing immediate transaction.
- Extend `BmadHelpRunLatest` to distinguish created, interrupted/non-resumable, and completed rows.
- Add a strict closed completed projection containing only recommendation display fields and a
  metadata-only receipt summary (never proof, raw response, consent authority, or token data).

**Proof**

```powershell
cargo test -p desktop-store --test bmad_method_store
cargo test -p desktop-ipc
cargo test -p desktop-store
cargo clippy -p desktop-store -p desktop-ipc --all-targets --all-features --locked -- -D warnings
```

**Commit:** `feat(bmad): retain completed Help projections atomically`

## Checkpoint 5: Implement the one-shot BMAD model coordinator

**Files**

- Add `crates/desktop-app/src/bmad_model.rs`
- Add focused submodules under `crates/desktop-app/src/bmad_model/`
- Modify `crates/desktop-app/src/state.rs`
- Modify `crates/desktop-app/src/lib.rs`
- Modify `crates/desktop-app/Cargo.toml`

**RED tests**

- Full deterministic state sequence:
  `Created -> Bound -> ContextReviewRequired -> Ready -> Advancing -> Completed`.
- Exact sealed Help instructions, current intent, catalog candidate, and evidence token facts appear
  in the reviewed outbound context and no other bytes do.
- Approval alone performs no transport.
- Submit consumes once, writes Advancing before transport, dispatches once, verifies exact bytes and
  receipt, materializes, and atomically finalizes.
- Duplicate submit, stale manifest/decision, expiry, sign-out, renderer/workspace/run drift,
  restart, transport failure, invalid schema, invalid receipt, and store failure all fail closed.
- Default composition uses `OfflineModelTransport`; no deterministic fallback exists.

**Implementation**

- Add `deterministic-help` Cargo feature depending on `desktop-cloud/deterministic-fake`.
- Compile `BmadCompiledHelpInvocation` from sealed foundation and host model configuration.
- Derive a `UserAsserted` architecture evidence token only when the intent contains a bounded,
  explicit architecture/readiness signal; otherwise the only valid deterministic result is
  `catalog_evidence_absent`.
- Prepare exact D2 items for sealed instruction, current intent, and closed catalog/evidence facts.
- Own one ledger, pending manifest/binding/decision, compiled invocation, and sanitized outcome.
- Use a feature-gated deterministic Help fixture and proof verifier only in explicit dev/test builds.
- Produce stable terminal reason codes; never project provider or validation internals.

**Proof**

```powershell
cargo test -p desktop-app --lib bmad_model --all-features
cargo test -p desktop-app --lib bmad_model
cargo clippy -p desktop-app --all-targets --all-features --locked -- -D warnings
```

**Commit:** `feat(bmad): compose one-shot verified Help requests`

## Checkpoint 6: Expose the capability through validated IPC

**Files**

- Modify `crates/desktop-runtime/src/command.rs`
- Modify `crates/desktop-runtime/src/error.rs`
- Modify `crates/desktop-ipc/src/envelope.rs`
- Add `crates/desktop-ipc/tests/bmad_help_model_access.rs`
- Modify `crates/desktop-app/src/commands.rs`
- Modify `crates/desktop-app/src/wire.rs`
- Modify `tools/check-boundaries.mjs`

**RED tests**

- Exact eight command envelopes, payload bounds, unknown-field rejection, capability gating, and
  mutability classification.
- Renderer cannot submit model/profile/schema/destination/context/result/receipt fields.
- Review responses show exact outbound bytes and disclosure; completed responses omit proof, token,
  absolute path, raw proposal, and Method/D2 authority hashes.
- Large review/results are excluded from the general reply cache; ambiguous mutation replay fails.
- Existing `context.preview` still rejects a model target.

**Implementation**

- Add the D2 stable error codes from the approved design to `LocalErrorCode`.
- Keep all commands behind `host_dispatch`; add no Tauri commands or permissions.
- Snapshot/revalidate renderer and workspace bindings at every coordinator transition.
- `bmad.help.latest` returns the current in-memory safe lifecycle or the authenticated durable
  projection after restart.

**Proof**

```powershell
cargo test -p desktop-ipc --test bmad_help_model_access
cargo test -p desktop-app --lib commands
cargo test -p desktop-runtime
node tools/check-boundaries.mjs
```

**Commit:** `feat(ipc): expose reviewed BMAD Help activation`

## Checkpoint 7: Wire the desktop review and result experience

**Files**

- Modify `apps/desktop-ui/src/lib/hostClient.ts`
- Add `apps/desktop-ui/src/lib/bmadModelProjection.ts`
- Modify `apps/desktop-ui/src/lib/bmadProjection.ts`
- Modify `apps/desktop-ui/src/App.tsx`
- Modify `apps/desktop-ui/src/components/TaskWorkspace.tsx`
- Add `apps/desktop-ui/src/components/ContextEgressReview.tsx`
- Add `apps/desktop-ui/src/components/BmadHelpResultCard.tsx`
- Modify `apps/desktop-ui/src/components/Inspector.tsx`
- Modify `apps/desktop-ui/src/components/UtilityPanel.tsx`
- Modify `apps/desktop-ui/src/styles.css`
- Add/update colocated Vitest files

**RED tests**

- Exact command order: create -> prepare -> approve -> submit -> latest/completed.
- Cancel performs no send; duplicate click dispatches once; submit is disabled before approval.
- Workspace, grant, run, auth epoch, renderer generation, expiry, and recovery clear authority.
- Exact outbound text is inert, visible, and never placed in a live region.
- Completed canonical recommendation and receipt summary render; raw proposal/proof never do.
- Existing created-unbound and D1 context behavior remain compatible.
- Keyboard/focus flow and axe checks pass.

**Implementation**

- Use one discriminated BMAD request state machine owned by `App`.
- Change the initial CTA to `Review request`; approval never transmits.
- Add `Approve context`, `Cancel review`, and one-shot `Send request` controls.
- Keep short status messages in `role=status`, errors in `role=alert`, use `<time dateTime>`, and
  focus the review heading once without announcing context bodies.
- Show deterministic identity/result as development-only, never as production connectivity.

**Proof**

```powershell
pnpm --filter @sapphirus/desktop-ui exec vitest run
pnpm --filter @sapphirus/desktop-ui typecheck
pnpm --filter @sapphirus/desktop-ui build
pnpm verify:boundaries
```

**Commit:** `feat(ui): complete the BMAD Help review flow`

## Checkpoint 8: Developer entry point, documentation, and full gates

**Files**

- Modify root `package.json` with an explicit deterministic desktop dev command
- Modify `README.md`
- Modify `docs/roadmap.md` and D2/BMAD milestone evidence
- Update BigBrain project/focus notes after verification

**Implementation**

- Document the exact default-offline behavior and explicit deterministic command.
- Record that D2-D closes the local/demo vertical but D2-E and release signing remain production
  blockers; do not claim a live support plane.
- Record exact toolchain and gate evidence.

**Proof**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
pnpm verify:contracts
pnpm verify:boundaries
pnpm --filter @sapphirus/desktop-ui test
pnpm --filter @sapphirus/desktop-ui typecheck
pnpm --filter @sapphirus/desktop-ui build
dotnet test services/desktop-support-api/Sapphirus.DesktopSupportApi.slnx --no-restore
```

Then run the explicit deterministic desktop smoke and confirm:

- sign-in state is visibly development-only;
- outbound context matches the approved manifest;
- one send produces one verified receipt;
- completed Help recommendation survives app restart;
- a second send requires a new run/review;
- default build remains offline.

**Commit:** `docs(bmad): close the D2-D desktop activation gate`

## Deferred production blockers (D2-E/release)

- tenant/client/API identifiers and signed tenant policy;
- entitlement lease acquisition and production verifier;
- support API consent/receipt contract reconciliation;
- signed receipt issuer/audience/key/replay verification;
- packaged and Authenticode-verified Windows broker helper;
- managed provider deployment, signed installer, clean-machine smoke, and release audit.

These are not silently replaced by deterministic adapters.
