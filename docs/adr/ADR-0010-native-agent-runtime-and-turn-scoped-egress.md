# ADR-0010: Forge implements the agent runtime natively; egress accounting becomes turn-scoped

- Status: proposed (2026-07-26)
- Replaces: an earlier draft of this ADR that proposed running `opencode serve`
  as a sidecar. That draft is withdrawn — see "Why the sidecar was rejected".
- Amends: [ADR-0009](ADR-0009-forge-as-a-general-coding-agent.md) — ADR-0009 says
  *"tools are executed by the host, never by the renderer"* and describes an
  in-host Rust permission gate. This ADR keeps that literally true and says what
  runs the Method.
- Amends: [ADR-0001](ADR-0001-windows-local-authority.md) — the *accounting*
  half of its egress discipline, which ADR-0009 dropped without saying so.
- Unchanged: [ADR-0002](ADR-0002-independent-d2-d3-grants.md) grant epochs,
  [ADR-0006](ADR-0006-builder-authoring-activation.md) inert Builder output,
  [ADR-0007](ADR-0007-model-to-governed-edits.md) governed-edit review, and the
  sealed-foundation hash chain.

## Context

ADR-0009 withdrew ADR-0001's execution exclusion so Forge could be a general
coding agent. It did not say *what runs the Method*.

OpenCode is the reference implementation of the behaviour Forge wants: skill
discovery, agent and command loading, the tool loop, permission prompting. It is
MIT licensed. It is also entirely TypeScript on Node, with an Electron desktop
shell and no Rust anywhere.

## Decision

**1. Forge implements its agent runtime natively, in its own Rust host.
OpenCode is a reference, not a dependency.**

Forge does not execute OpenCode's code, does not embed its bundle, and does not
spawn it. What Forge takes from it is *design and data*: the shape of skill
discovery, the tool-loop structure, model-facing tool description text,
classification tables, and route conventions. These are adapted and copied under
MIT with attribution, and once copied they are **Forge's own source**, evolving
on Forge's schedule.

Forge is an independent application. Nothing in its runtime path requires
OpenCode to be present, buildable, or fetchable.

**2. The permission boundary is therefore a real enforcement boundary.**

This is the decisive gain, and it is worth stating precisely because the
withdrawn draft could not claim it. A co-process running third-party TypeScript
retains direct IO no matter what the host's gate says — `registry.named()`
returns builtin `read` and `task`, the attachment path calls it with
`ask: () => Effect.void`, and the `/file`, `/find`, `/pty`, snapshot, LSP,
formatter and watcher routes perform IO with no tool involved at all. With a
sidecar, "the host authorises every effect" was a product and audit claim that
was false as a security claim.

Natively, it is simply true. Every filesystem reach goes through
`WorkspaceBroker`, which applies `is_sensitive_relative_path` structurally, and
every tool call is evaluated by the permission gate before it runs. There is no
second process holding a handle.

**3. The two CI gates stay exactly as they are.**

`tools/check-boundaries.mjs:1297-1302` continues to fail the build on any
non-empty `bundle.externalBin`, and the spawn allowlist
(`check-boundaries.mjs:89-92`) keeps its existing two entries. The withdrawn
draft required amending both; this decision requires amending neither. A gate
that never has to be weakened is worth more than one that is weakened carefully.

**4. The host holds the provider credential.**

Trivially true with no co-process, and it resolves O1. The sanctioned provider
is Azure AI Foundry and the host already speaks it
(`DirectProviderKind::AzureOpenAi`, Azure OpenAI v1 endpoint, region-bound,
credential in zeroizing storage).

Azure authentication ships as a static `api-key`. Its costs are accepted
knowingly, not overlooked: no expiry, no per-user attribution in Azure's logs,
no conditional-access enforcement. `ProviderAuth` is the named seam for Entra
ID; the header is the easy part, and credential *lifetime* is the real work.

**5. Egress accounting becomes turn-scoped.**

A human approves a **turn** with bounds — maximum model calls, lifetime — and
every call inside it still mints and consumes its own correctly-bound,
single-use decision.

This is deliberately *not* "one decision reused". Reuse is already impossible
mechanically (`Pending → Consumed`, and `PendingContextDecision` is not
`Clone`). It would also be *wrong*: a decision is bound to a manifest hash that
covers the transcript, and the transcript changes every iteration, so honouring
an old decision would mean consenting to context that is no longer being sent.

`TurnEgressGrant` adds bounds and a ledger *above* the capability system without
touching it. Recording never refuses — egress is caused by the send, not by the
send succeeding — so a call that overruns its grant is recorded and flagged
rather than dropped. Byte totals are summed without deduplication and count
reviewed-context bytes, which is a lower bound on the wire.

## Why the sidecar was rejected

It was the faster route to BMAD fidelity, and it was rejected anyway:

- **It would have made Forge dependent on another project's runtime.** Forge is
  an internal application with a single installer; a second runtime means a
  second supply chain, a second update path, and a build that cannot proceed
  without fetching an external bundle.
- **It would have weakened the security claim precisely where it matters.** See
  decision 2. For a deployment carrying organisational data, an audit boundary
  described as an enforcement boundary is the kind of gap a review finds.
- **It would have forced two CI gates open.**
- **It would have added a Node runtime to the installer**, on top of an offline
  WebView2 runtime already carried.

## Consequences

**Accepted, and this is the real cost.**

- **Fidelity is now Forge's own problem.** Discovery semantics — config roots,
  the `.claude`/`.agents` walk-up, two distinct skill globs, skills-as-commands
  promotion, the `<skill_content>` contract — must be re-derived in Rust, and
  every drift is a silent BMAD workflow failure. This is exactly the risk the
  sidecar avoided, and accepting it means fidelity has to be *tested*, not
  assumed: a conformance suite over the vendored corpus is no longer optional.
- **More Rust work**, and the tool loop, permission prompting and streaming are
  all Forge's to build rather than inherit.
- **The `_bmad/` problem is Forge's to solve.** 39 of 47 skills read
  `{project-root}/_bmad/**` and 36 read a generated `config.yaml`. The virtual
  overlay in the governed-IO layer is now core work, not an integration detail.

**Gained.**

- One runtime, one installer, one toolchain, no Node supply chain.
- A permission boundary that is genuinely enforcing.
- Both CI gates intact.
- No dependency on a project whose direction Forge does not control.

## What this ADR does not decide

- **The Builder.** `bmad-builder` is a separate upstream project (`code: bmb`).
  **ADR-0006 stands** — Builder output remains inert until that adoption is
  scheduled.
- **The `uv`/Python dependency.** 39 skills shell out to
  `uv run {project-root}/_bmad/scripts/*.py`, and none of those scripts are
  vendored. Bundling Python, reimplementing the three scripts natively, or
  degrading those skills is a separate decision — and reimplementing them is now
  the option most consistent with this ADR.
- **Entra ID.** Named seam, unscheduled work.
