# Forge v2 comprehensive architecture, BMAD, and cloud-fidelity audit

**Date:** 2026-07-23  
**Candidate:** `codex/forge-v2` at `453be1a314e9dacdb7357be95b0ba70704f57d3c`  
**OpenCode reference:** [`anomalyco/opencode` at
`0a601cf3`](https://github.com/anomalyco/opencode/tree/0a601cf334b9a83cc2854108a2b860f25e6e7e8e)  
**Verdict:** **Blocked for a “fully integrated BMAD” or production-cloud
readiness claim.** The bounded repairs in this pass materially improve the
local prototype and its test fidelity, but they do not close the product,
Azure, installer, or pilot gates.

## Scope and evidence boundary

This is an audit of the current dirty Forge candidate, not a clean-baseline or
release-artifact qualification. The worktree already contained a large
in-flight Forge cutover before this pass. The changes made here were kept
narrow and were reviewed as bounded slices.

The comparison uses the complete pinned OpenCode checkout and its MIT
attribution as an architecture reference. Forge deliberately does not seek
feature parity where OpenCode's broader filesystem, terminal, provider, or
renderer authority would conflict with Forge's Rust-host trust boundary.

## Executive scorecard

| Area | Current assessment | Release consequence |
|---|---|---|
| Native authority boundary | Strong prototype design | Keep: credentials, workspace reads, consent, receipts, and effects remain host-owned |
| OpenCode renderer adaptation | Useful but structurally incomplete | Task/session ownership and modular state need a dedicated milestone |
| BMAD menu projection | Improved and fail-closed at the activation seam | Availability is not proof of upstream workflow-semantic fidelity |
| BMAD execution semantics | Incomplete | Do not call the current one-shot generic lifecycle “full BMAD” |
| Local Docker development lane | Reproducible local rehearsal | Suitable for local/CI proof only |
| Production-shaped Docker lane | Honest boot/configuration contract | It deliberately cannot prove Azure dependency behavior |
| Azure deployment path | Incomplete and not live-qualified | Blocks cloud, AI, and pilot readiness |
| Installer/release/pilot | Not qualified in this audit | Remains a separate signed, clean-machine, operational evidence lane |

## OpenCode comparison

### What Forge adapted well

- The v2 shell, compact workbench, tabs, context, files, review layout, and
  diff presentation use OpenCode as a design reference without importing its
  authority model.
- Forge keeps native filesystem reads and Git diff bytes behind typed Rust
  commands. The renderer does not receive a shell, arbitrary path access, or
  mutation authority.
- The direct-provider prototype uses a no-redirect HTTPS client, allowlisted
  endpoints, no-store requests, native credential custody, explicit reviewed
  context, and signed/typed receipt seams.
- OpenCode source and licensing are pinned and attributed in-repository.

### Material architecture gaps

| Concern | OpenCode reference | Forge evidence | Assessment |
|---|---|---|---|
| Tab identity | `packages/app/src/context/tabs.tsx` represents draft and session tabs with explicit `draftID`, `server`, and `sessionId` | `App.tsx` stores only drafts by task; BMAD, context, capability, model, and edit state are app-global | **Critical:** Forge tabs are visual routes, not isolated execution sessions |
| Prompt ownership | `packages/app/src/context/prompt.tsx` and `prompt-state.ts` scope prompt state by draft/session route | `App.tsx` owns a single controller tree and `hostClient/client.ts` sends `sessionId: null` for projections | **Critical:** results can be displayed under a newly selected task |
| Stale async work | `server-session.ts` and `session-ownership.ts` use per-session generations and ownership checks | Forge has several global generation refs but no task-owned host session | **Watch:** good local guards do not replace session identity |
| Composition | OpenCode separates tabs, prompt, server/session, layout, and ownership contexts | Forge v2 imports the shared `desktop-ui/src/App`; `App.tsx` is about 2,875 lines | **Watch:** the v2 boundary is a thin entry point with a large shared blast radius |
| Authority | OpenCode supports a much broader interactive coding environment | Forge intentionally keeps credentials, reads, consent, and effects native | **Good divergence:** do not copy renderer-owned authority to close visual parity |

This pass also made projection-cursor recovery re-run the same complete host
bootstrap/rebind path as initial connection, invalidate every host-bound
renderer domain before polling resumes, retain only still-authoritative
workspace selection, and fail closed if rebootstrap cannot complete. The
active v2 source root is now included in renderer-boundary scans, and the
Context panel distinguishes a local review budget from provider context while
showing the actual model ID and no synthetic zero cost.

The next OpenCode-parity milestone should therefore be **task/session
ownership**, not more visual borrowing.

## BMAD integration audit

### What is actually integrated

- The sealed foundation verifies 114 selected source members and 40 managed
  outputs.
- The product denominator contains six Method agents, 26 roster menu paths,
  24 unique Method capabilities, and five enabled Builder authoring
  capabilities whose outputs remain inert drafts.
- The honest runtime projection currently exposes 25 of those 26 Method menu
  paths (23 unique capabilities). Architect/CA remains fail-closed because its
  sealed guidance requires a later state-machine packet.
  **(Superseded 2026-07-25: all 26 paths / 24 unique capabilities are exposed;
  CA runs as an ordered progression — see P9.)**
- All user-visible activation is mediated by product-owned host bindings; the
  sealed roster remains provenance data and grants no authority.
- Capability requests use a prepare/approve/cancel/submit lifecycle, reviewed
  context limits, typed output archetypes, and D3 re-entry for proposed
  workspace changes.
- Builder output remains an inert draft and cannot install, register, or
  execute an agent or workflow.

### Repairs completed in this pass

- Corrected document and Builder provider instructions to the canonical
  schema fields rather than legacy nested shapes.
- Closed nested object parsing for document sections, Builder files, and
  governed change entries.
- Projected the host-validated capability ID with each runnable Method menu,
  removed the renderer-owned 26-entry menu/capability table, and added
  valid-ID swap, drift, and negative tests across the host/IPC/renderer seam.
- Enforced the same enabled set at direct capability preparation so a crafted
  renderer request cannot bypass an unavailable menu.
- Preserved real source display labels for adopted prompt-reference
  replacements.
- Corrected governed-change provider instructions so create, replace, and
  delete variants match the canonical schema and host parser.
- Included each reviewed outbound content hash in provider context, serialized
  replace/delete preimages with canonical `preimageSha256`, and proved stale
  workspace bytes still fail closed when a result re-enters D3.
- Replaced permissive JSON-to-`Value` decoding with recursive duplicate-member
  rejection at the direct-provider envelope/payload, cloud verifier, host wire
  verifier, and stored-result D3 reparse boundaries.

### Why “correctly and fully integrated” is still false

#### Critical — menu paths are not faithful workflow engines

The generic capability lane is a single reviewed model request that returns
one document, change set, or inactive draft. Its prepare input contains a
binding, configuration, instruction, context, identities, and timestamp, but
no user objective, authoring request, ordered step, decision history, or
multi-turn dialogue state required by several upstream Method workflows.

The clearest contradiction is Architecture: its sealed instruction says the
capability remains unavailable until a later state-machine packet and assumes
host-selected ordered steps, while the generic host lane is one-shot. The
runtime must either keep that menu fail-closed or implement and verify the
state machine before claiming it is available.

Other managed adaptations also differ materially from upstream semantics:

- Market Research records external facts as open questions because live
  retrieval is unavailable.
- Dev Story and Quick Dev are reduced to schema-first one-shot change
  proposals rather than their upstream ordered/scripted workflows.
- Several document workflows return a bounded inert artifact without their
  original collaborative progression.

These can be legitimate Forge adaptations, but they must be named as adapted
capabilities rather than asserted as complete upstream BMAD execution.

> **Update (2026-07-25):** the Architecture contradiction named in this section
> is **closed**. The ordered state-machine packet is implemented and verified
> end to end
> ([P9](../implementation-packets/P9-architecture-state-machine-2026-07-25.md)):
> eight host-selected steps with a stable decision log, refusing to skip,
> repeat, or finalize early, distilled locally into one inert spine. CA is
> enabled, so the runtime now exposes all 26 reviewed Method menu paths and the
> sealed Help binding's "Create Architecture" recommendation is actionable.
> The rest of this finding stands: the other adaptations remain one-shot, and
> the ordered lane is still **not** asserted as upstream fidelity.
>
> **Update (2026-07-24):** an explicit per-capability matrix now exists at
> [`bmad-capability-fidelity-matrix.md`](bmad-capability-fidelity-matrix.md).
> It classifies all 29 capabilities as adapted one-shot (23), unavailable (1 —
> Architecture), or inert draft (5), asserts **zero** as faithful upstream
> execution, and records the Help/Architecture conflict as an open item. The
> runtime now renders an honest "Adapted"/"Unavailable" badge on every runnable
> Method menu (`apps/desktop-ui/src/lib/bmadFidelity.ts`,
> `components/BmadLibraryPanel.tsx`).

#### Critical — task identity is missing

The renderer supports multiple task labels, but native projection requests use
`sessionId: null`, the host rejects every non-null Agent session as unavailable,
and the principal BMAD/context/edit states are global. Selecting another tab
changes presentation without creating a separate host session. A slow
completion can therefore be associated with the wrong visible task.

Acceptance requires:

1. a durable host-issued task/session identity;
2. every prepare, approval, submit, result, projection, and receipt bound to it;
3. task-owned renderer state and stale-generation invalidation;
4. restart recovery and cross-tab attribution tests.

#### Watch — Help sees only a fraction of the enabled product

The sealed Help binding requires exactly two installed skills and two actions
(`Create Architecture` and `BMAD Help`) while the generic host exposes the
broader Method capability table. Help therefore cannot reliably recommend,
sequence, or explain the product's full enabled denominator; its Architecture
row also conflicts with the runtime decision to keep CA fail-closed.

#### Critical — generic capabilities do not compose the production transport

The generic capability transport selector supports direct-development,
deterministic-development, and offline modes only. It does not select the
production support-plane round trip. Even the separate production Help
transport remains an explicitly fail-closed seam until the desktop shell
provides a Windows identity broker and HTTPS round-trip composition.

Deploying the support API therefore does not make the advertised Method
capabilities production-operational.

#### Watch — lifecycle durability and concurrency remain incomplete

- Generic coordinator state is in memory; restart recovery is not equivalent
  to the durable Method Help lifecycle.
- Result finalization and “latest result” consumption are separate operations.
- The host holds the capability coordinator mutex across the model transport
  call, allowing a long request to block unrelated capability operations.
- Response binding/receipt verification is not preserved as a task-visible,
  same-renderer durable lineage after completion.

#### Info — provenance remains intentionally blocking

The selected foundation bytes verify, but upstream archive/source-identity
promotion evidence remains incomplete. This is correct fail-closed behavior
and should not be bypassed to improve a percentage.

## Docker and cloud-fidelity audit

### The two honest local lanes

| Lane | What it proves | What it does not prove |
|---|---|---|
| `infra/local/compose.yaml` | Pinned frontend build, Forge v2 production assets, Development support API, loopback networking, health, exact image use | Tauri/WebView2, WAM, DPAPI, Entra, Azure SQL, Key Vault, App Configuration, Azure OpenAI |
| `production-configuration-contract` profile | Production option validation, dependency composition, process boot, safe liveness under `network_mode: none` | Dependency readiness, managed identity, private DNS/VNet, revisions/scaling, signing, model invocation, monitoring |

The second lane is deliberately network-isolated and checks liveness only.
Calling it Azure emulation would be misleading.

### Repairs completed in this pass

- Added the required approved desktop client ID to Bicep, parameter examples,
  generated ARM JSON, and the Container App environment.
- Fixed locked NuGet restore ordering in the support image.
- Pinned the frontend Node base by version and digest.
- Made Development harnesses readiness-based, loopback-only, initialized, and
  image-injectable.
- Added exact-image Development and Production-configuration runtime smoke,
  image identity checks, frontend HTTP/health proof, workflow trigger coverage,
  and unconditional cleanup.
- Wired the local contracts into `verify:source`.
- Made an empty approved-model list a valid explicit kill switch while keeping
  profile resolution fail-closed.
- Reordered the SQL runbook to bootstrap the migration identity and ledger,
  run migrations, and only then create the runtime identity/object grants.
- Added an explicit Production-only `migrate-sql` image command that selects a
  distinct migration managed identity, runs the embedded migration runner, and
  exits before web-service composition; unsafe manual ledger writes were
  removed from the runbook.
- Exposed deterministic runtime, image-pull, and migration identity outputs
  from Bicep, bound the migration job contract to those exact identities, and
  exercised the final rebuilt image in both ordinary Production startup and
  fail-closed migration modes.
- Removed a command-only empty migration identity from ordinary application
  defaults after a fresh-image Compose replay proved it crash-looped the normal
  Production API during configuration binding.
- Replaced the runbook's unsafe local build/push path with a hard requirement
  for one source-bound digest from the protected organization release lane,
  together with scan, SBOM, signature, attestation, and approval evidence.

### Remaining cloud blockers

#### Critical — protected smoke uses the wrong identity shape

The protected workflow signs in with a GitHub workload identity and
`deployed-smoke.ps1` simply requests a token from that Azure CLI session.
The API requires a delegated desktop-client/user token and validates the
authorized-party client. The script accepts `ApiClientId` but does not use it.
That smoke cannot prove the desktop authentication contract.

#### Critical — the native production round trip is not composed

Local direct-provider proof and support-plane API proof exist as separate
lanes. No deployed run currently proves:

desktop sign-in → registration → lease → exact consent → no-store model call →
signed receipt → Rust verification → BMAD completion → optional governed edit.

#### Critical — no trusted production image publisher exists yet

The corrected deployment guide now forbids local rebuilds and accepts only the
exact tested, scanned, SBOM-bound, signed, attested, and approved digest emitted
by the protected organization release lane. The current workflow builds and
scans a CI image but does not publish that production artifact or its complete
evidence bundle. Deployment therefore remains blocked rather than silently
substituting a locally rebuilt image.

#### Watch — the migration job is specified, not deployed

The one-shot Container Apps Job has a closed operational contract (immutable
image, exact identities, no ingress, one replica, zero retries, bounded
timeout), but the job resource is still a runbook definition rather than a
deployed Bicep resource. It has not been executed against managed identity and
a clean Azure SQL database.

#### Watch — monitoring is specified but not deployed-proven

Alert rules query custom `Sapphirus.DesktopSupport` metrics. Production
composition enables Azure Monitor tracing but does not explicitly register the
custom meter. No live qualification shows that every alert query receives the
expected metric and dimensions.

#### Watch — Azure-only behavior is outside Docker

Docker cannot reproduce WAM/Entra delegated authentication, DPAPI/WebView2,
managed identity, Azure SQL locking/migrations, Key Vault HSM behavior, App
Configuration refresh/last-known-valid policy, Azure OpenAI private endpoints,
Container Apps revision/scaling behavior, private DNS/VNet, Azure Monitor, or
staged rollback and recovery.

## Ranked findings and next modules

### Critical

1. **Task/session isolation:** bind every renderer and host operation to a
   durable session and test cross-tab races.
2. **BMAD semantic engines:** implement or explicitly defer each workflow whose
   upstream semantics require ordered, multi-turn, or scripted execution.
   **(2026-07-25: the ordered lane exists and Architecture uses it — P9. The
   remaining workflows stay explicitly labelled as adapted one-shots in the
   fidelity matrix rather than silently overclaimed.)**
3. **Production capability transport:** compose the signed support-plane
   round trip into generic capability dispatch and retain its verified lineage.
4. **Delegated authentication qualification:** use an approved desktop public
   client and real test user for the deployed smoke.
5. **Trusted production artifact:** publish and verify the exact CI-tested
   support image with its scan, SBOM, signature, attestation, and source
   revision bound to one immutable digest.
6. **Native cloud composition:** prove the signed receipt round trip from the
   actual packaged desktop against the staged Azure deployment.

### Watch

1. Split the shared `App.tsx` controller into task-owned state domains before
   adding more OpenCode-inspired features.
2. Expand or deliberately constrain the Help catalog so it matches the
   product's advertised capability denominator.
3. Make generic capability recovery/result finalization durable and release
   coordinator locks before model network waits.
4. Reconcile managed adaptation text with actual runtime semantics and retain
   explicit upstream-vs-Forge labels.
5. Prove custom metrics export and every scheduled alert with live synthetic
   signals.
6. Rehearse the corrected identity/ledger/migration/runtime-grant order against
   a clean Azure SQL database through the one-shot Container Apps Job.
7. Encode and deploy the closed one-shot migration Job resource rather than
   relying only on a prose operator definition.
8. Replace the experimental Bicep assertion dependency before production if
   the organization's toolchain policy disallows it.

### Info

- The Rust-host authority design is stronger than literal OpenCode parity for
  Forge's privacy and governance goals.
- The Builder slice correctly remains inert.
- The Production Docker profile is useful because it is narrow and honest;
  expanding its name or claims would reduce, not improve, fidelity.

## Verification record

| Check | Result |
|---|---|
| Pinned Node 24.18.0 / pnpm 11.15.1 renderer/source scope | Pass on the final tree: toolchain, workflows, local/cloud contracts, foundation, contracts, secrets, release tests, both UI typechecks, lint, 397 renderer tests across 37 files, boundaries, and production build |
| BMAD foundation | Pass: 116/116 package tests; 114 selected source members and 40 managed outputs verified |
| BMAD desktop host | Pass: 117/117 all-feature library tests, including menu binding and strict untrusted-JSON cases |
| Rust formatting | Pass |
| Rust workspace, locked, all features | Pass with `--jobs 1`; initial parallel run exhausted the Windows paging file rather than exposing a code failure |
| Rust all-target/all-feature Clippy | Pass with warnings denied and `--jobs 1` |
| Support API tests | Pass: 181/181 after the model kill-switch, migration command, and normal-Production binding regression additions |
| Docker support Development lane | Pass on rebuilt image `sha256:e0563831501d…`: exact-image identity, readiness, and HTTP 200 |
| Docker Production configuration lane | Pass on the same rebuilt image: network-isolated normal API startup plus internal liveness |
| Docker migration guards | Pass on the same rebuilt image: Development refusal and Production equal-identity refusal both exit exactly `1` without a listener |
| Docker frontend | Pass: pinned image built and returned HTTP 200 |
| Compose configuration | Pass for both default files and the optional Production profile |
| Bicep build / parameter compile / generated JSON parity | Pass; experimental-assertion warning remains |
| Readiness scorecard | Expected fail: stale revision; full BMAD breadth 35%, production model-backed Help 40%, complete installable executable 25%, internal pilot 10%, plus missing clean-checkout, signed-installer, Azure E2E, accessibility, security, and pilot evidence |
| Trusted production support-image publication | Not implemented; deployment correctly remains blocked |
| Live Container Apps migration Job / clean Azure SQL | Not run |
| Live Azure deployment | Not run |
| Live user-owned model-provider smoke | Not run |
| Signed installer / clean-machine / pilot | Not run |

## Release decision

The bounded patch set is appropriate to continue local development after its
focused reviews are green. It is **not** evidence that Forge has full BMAD
workflow parity, that Docker reproduces Azure, or that the candidate is ready
for release or pilot use.

The first valuable next checkpoint is task/session identity. The first cloud
checkpoint is a clean-database bootstrap plus delegated desktop authentication
in a real staged Azure environment. The first BMAD checkpoint is an explicit
per-capability matrix that marks each path as adapted one-shot, faithful
state-machine, or unavailable.
