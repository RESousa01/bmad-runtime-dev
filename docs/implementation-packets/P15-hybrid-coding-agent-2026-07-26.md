# Implementation packet: P15 — Forge as a hybrid coding agent

## Authority and intent

- Owning authority: repository maintainer (RodrigoSousa0).
- Product intent: Forge becomes a general coding agent of OpenCode's class,
  differentiated by the BMAD Method being its interface rather than by withheld
  capability. The user must be able to run the BMAD Method — methodology,
  skills, agents, workflows — **and the BMAD Builder** exactly as they work in
  the upstream repository.
- Governing decisions:
  [ADR-0009](../adr/ADR-0009-forge-as-a-general-coding-agent.md) and
  [ADR-0010](../adr/ADR-0010-native-agent-runtime-and-turn-scoped-egress.md).

> **Read this first — D1 was reversed after most of this packet was written.**
>
> This packet originally chose to run OpenCode as a **sidecar**. That is
> withdrawn: **Forge is an independent application** and implements its agent
> runtime natively, adapting and copying from OpenCode under MIT rather than
> executing it. See the revised **D1**, the withdrawn **D2**, and
> [ADR-0010](../adr/ADR-0010-native-agent-runtime-and-turn-scoped-egress.md).
>
> Sections written before the reversal still discuss the sidecar, its transport,
> its containment and its handshake. **Treat every such passage as background on
> a rejected option, not as the plan** — in particular Phase 2 as written, D3,
> D4, and the sidecar↔host contract. The research they rest on stays useful:
> what OpenCode does is exactly what Forge must now re-derive.
>
> Unaffected by the reversal, because none of it assumed a second process: D5
> (governed writes), D6/D6a (Method as payload), D7 (turn-scoped egress), and
> every line of Phase 1 already implemented.
- Platform: Windows desktop only.
- Status: **plan only.** Nothing here is implemented beyond the permission gate
  and bounded shell runner already landed in `crates/desktop-tools`.

## What the research settled

Six parallel surveys of `library/opencode` (MIT, © 2025 opencode, local
reference checkout at `v1.18.4`, HEAD `0a601cf`) and of this repository.

**1. OpenCode is already a first-class BMAD target.** Upstream BMAD's installer
defines platform `opencode`, installing skills to `.agents/skills` and commands
to `.opencode/commands`
(`bmad-runtime-lib/_source_review/BMAD-METHOD-main/BMAD-METHOD-main/tools/installer/ide/platform-codes.yaml:255-261`).
OpenCode's v1 discovery reads both roots natively — `.claude` and `.agents`
walked up from the working directory, plus `{skill,skills}/**/SKILL.md` under
every config root (`packages/opencode/src/skill/index.ts:173-233`), and
`{command,commands}/**/*.md` (`src/config/command.ts:15`). **Full BMAD fidelity
is therefore a configuration problem, not a reimplementation problem** — but
only on the v1 route surface; `packages/core` (the `/api/*` surface) has no
`.agents` root at all.

**2. Permission can be delegated outward today, with a shipped reference.**
OpenCode's own ACP adapter subscribes to `permission.asked` on the event stream
and answers over HTTP (`src/acp/permission.ts:37-97`,
`POST /permission/:requestID/reply` at `src/server/.../groups/permission.ts:31-43`).
Default action when no rule matches is `ask` (`src/permission/index.ts:28-38`).

**3. Tool execution can be redirected without a fork.** Custom tools are scanned
from `{tool,tools}/*.{js,ts}` in every config root
(`src/tool/registry.ts:178-192`), appended after builtins (`:253`), and written
into the model-facing map last-wins (`src/session/tools.ts:92-99`). A `read.js`
in `$OPENCODE_CONFIG_DIR/tool/` replaces the builtin `read`.

**4. There is no Rust anywhere in OpenCode**, and its desktop app is Electron.
"Copy the source" is unavailable for the host. What *is* reusable: the model-facing
tool description text, classification data, and — for the renderer — Solid
components that port to React mechanically.

**5. Node runs it.** `packages/opencode/script/build-node.ts:15-29` targets Node; OpenCode's own
Electron app imports that bundle and calls `Server.listen(...)`
(`packages/desktop/src/main/sidecar.ts:57-65`). Forge already pins Node
`24.18.0`, and `node:sqlite` needs ≥22.5. The `dist/node` ESM bundle is the
supported route and avoids a ~100 MB `bun --compile` artifact.

## The central problem, stated plainly

**Tool shadowing is behavioural redirection, not enforcement.** Even with every
builtin shadowed, the sidecar process retains direct IO:

- `registry.named()` returns the *builtin* `read` and `task`
  (`src/tool/registry.ts:337-340`), used at `src/session/prompt.ts:266` and
  `:813` — and the attachment path calls it with `ask: () => Effect.void`
  (`prompt.ts:825`), bypassing permission entirely.
- `edit`/`write` are dropped in favour of `apply_patch` for most `gpt-*` models
  (`registry.ts:292-295`) — the guard excludes `gpt-4*` and any `*oss*` build,
  which keep `edit`/`write` — so a shadowed `edit` silently disappears on the
  models that do swap.
- The `/file`, `/find`, `/pty`, snapshot, LSP, formatter and watcher routes
  perform IO with no tool involved at all.

So "the sidecar proposes, Rust authorizes" is achievable as **product behaviour
and as an audit boundary**, but it is **not an in-process security property**.
Anyone who states it as one is wrong.

**And ADR-0009 dropped egress accounting without saying so.** Every model call
today passes `ContextPreparer` + `PatternSecretScanner` + a sealed manifest +
a single-use decision (`crates/desktop-app/src/chat.rs:412-448`, with the
scanner wired in at `:565`). The moment
`shell` exists, `curl`, `git push` and `npm publish` are egress paths with no
manifest, no secret scan, no region binding and no receipt. ADR-0009 re-bought
the *authorization* half of ADR-0001 and silently dropped the *accounting* half.

## Deployment context, and what it decides

Two facts about the target deployment are design inputs, not background:

**Forge is an internal organisation application.** The user population is known,
the machines are managed, and the adversary worth planning against is not an
anonymous attacker on the internet. It is (a) **model output and workspace
content**, which are attacker-influenced whenever a repository contains
something hostile, and (b) **accidental egress** of organisational material.
Insider misuse matters, but the user already has a shell — Forge is not the
weakest path to it.

That sharpens the security priorities rather than relaxing them:

- **Egress accounting outranks execution sandboxing.** ADR-0009 re-bought the
  authorisation half of ADR-0001 and dropped the accounting half; for an
  internal deployment that dropped half is the one with compliance
  consequences. D7 is therefore not a refinement, it is the point.
- **The `region` binding stops being decorative.** Azure Foundry deployments are
  region-pinned, and an organisation subject to data-residency rules needs the
  guarantee that reviewed context went to the region it was bound to. The
  manifest already carries `region`; the work is keeping it honest once a loop
  makes many calls per turn.
- **Prompt injection is the live threat.** A tool result is
  attacker-influenced content. This is why the v2 chat instructions end by
  telling the model that results are data and never instructions, and why the
  delimiter forgery risk (`direct_provider.rs:426-437`) needs nonce-fencing
  rather than a comment.

**The sanctioned provider is Azure AI Foundry, and the host already speaks it.**
`DirectProviderKind::AzureOpenAi` exists, targets the Azure OpenAI v1 endpoint,
binds a region, and holds its credential in zeroizing storage. This resolves O1
(below) and removes provider breadth from the list of reasons to run a sidecar —
which leaves BMAD fidelity as the *only* reason, and that is worth being honest
about when weighing option B in O3.

**On performance.** The security posture above costs little where it counts,
because the dominant cost in this application is the model round trip, not local
work. Keep the boundary crossings cheap and the model calls few: the loop's
iteration cap and the transcript window matter far more for perceived speed than
any in-process check. Where there *is* a genuine tension, it is Phase 0's
concurrency work — a global `in_flight` gate and a whole-ledger mutex will bind
as soon as several people use several task tabs. Security-first does not mean
serialising the app.

## Design decisions

**D1 — REVERSED. Forge implements the agent runtime natively. OpenCode is a
reference, not a dependency.**

An earlier revision chose option A — run `opencode serve` as a sidecar — on the
grounds that it delivered BMAD fidelity on day one with no compatibility risk.
That is withdrawn. **Forge must be an independent application**: it does not
execute OpenCode's code, embed its bundle, or spawn it. What it takes is design
and data — discovery semantics, tool-loop structure, model-facing tool
description text, classification tables, route conventions — adapted and copied
under MIT with attribution, and thereafter Forge's own source.

See [ADR-0010](../adr/ADR-0010-native-agent-runtime-and-turn-scoped-egress.md).

*What this gains, and it is more than independence.* The sidecar's boundary was
never an enforcement boundary: a co-process running third-party TypeScript keeps
direct IO regardless of the host's gate (`registry.named()`, the
`ask: () => Effect.void` attachment path, and the `/file`, `/find`, `/pty`,
snapshot, LSP, formatter and watcher routes). "The host authorises every effect"
was a product claim that was false as a security claim. Natively it is simply
true. Both CI gates also stay untouched — no `externalBin`, no third spawn
allowlist entry — where option A required forcing both open.

*What this costs, and it is the real price.* Fidelity becomes Forge's own
problem. The four config roots, the `.claude`/`.agents` external walk-up, the
two genuinely different skill globs (`{skill,skills}/**/SKILL.md` for config
roots versus `skills/**/SKILL.md` for the walk-up), walk-up semantics,
skills-as-commands promotion and the `<skill_content>` contract must all be
re-derived in Rust, and every drift is a silent BMAD workflow failure. **So
fidelity has to be tested rather than assumed** — a conformance suite over the
vendored 47-skill corpus stops being optional and becomes the thing that makes
this decision safe.

**D2 — WITHDRAWN with D1.** There is no sidecar to contain, so the Job Object,
scrubbed environment and pinned image path are moot. The Job Object work does
*not* disappear entirely: `shell.rs`'s `child.kill()` still fails to kill a
`cmd /C` process tree, and that leak is worth fixing on its own merits when
`shell` is wired.

**D3 — Transport is stdio with newline-delimited JSON.**
An anonymous inherited pipe is the only option where "can another local process
connect?" is answered by the kernel rather than by a DACL plus a PID check. It
also gives free lifecycle coupling: the sidecar sees EOF the instant the host
dies. Loopback TCP is rejected outright — Windows does not access-control
loopback by user. Named pipes are the acceptable second choice and have
precedent here (`crates/desktop-cloud/src/windows_broker.rs:102-132`).

**D4 — Rust owns permission evaluation, unconditionally.**
The sidecar may never supply a rule, a pattern, an action, a prior approval, or
any prompt copy. `PermissionRequest.title` and `.detail` are tool-authored by
construction (`crates/desktop-tools/src/permission.rs:145-154`); a
sidecar-supplied justification string in a consent dialog is a social-engineering
primitive. Every `permission.asked` from the sidecar is re-evaluated by
`desktop_tools::permission::evaluate` and answered **`once`** — never `always`,
because OpenCode caches an `always` in its own process
(`src/permission/index.ts:145-151`) and Rust must stay authoritative.

**D5 — Agent file writes go through the existing D3 stack, at a branched
approval type.** Reuse everything below the approval seam without exception:
dual compare-and-swap on content *and* file identity
(`crates/desktop-workspace/src/governed_io.rs:837-840`), checkpoint-before-effect,
the 12-state journal, rollback and crash recovery. Do **not** reuse
`displayed_diff_hash` — it means "the exact bytes a human reviewed"
(`crates/desktop-runtime/src/edits.rs:109-111`) and for an auto-approved agent
write nothing was displayed. Add a sibling approval whose seal binds *the rule
that authorized it* — `{candidate_hash, proposed_diff_hash, permission_tool,
permission_subject, permission_rule_matched, permission_reply}`. Everything from
`consume_spec_record` down is unchanged.

**D6 — Forge ships its own copy of the Method as app payload. There is exactly
one installer, and it is the app's.**

An earlier revision of this packet said "Forge's own installer", which was
wrong on an important point. **The app has one installer — the NSIS per-user
setup — and the Method ships inside it as payload.** There is no second
installer, no separate BMAD setup step, and no "install BMAD into this project"
action the user has to perform.

Nor does the Method get written into the user's repository. OpenCode appends
`$OPENCODE_CONFIG_DIR` to its config-directory list
(`packages/opencode/src/config/paths.ts:23-41`, verified), and every config root
is scanned for skills, commands, agents and custom tools. So Forge points that
variable at an **app-local** root under its own install directory, lays the
Method out there as `skills/<name>/SKILL.md` (the config-root pattern is
`{skill,skills}/**/SKILL.md`, *not* the `.agents/skills` project pattern), and
the sidecar discovers it in any workspace with nothing written to the user's
project at all.

That is better than the project-install route on four counts, and it is the
route this packet takes:

- one installer, satisfying the constraint by construction;
- the Method is available in *every* workspace immediately, with no per-project
  setup;
- the user's repository stays clean — no `.agents/` or `.opencode/` directories
  appearing in their diff;
- the Method upgrades with the app, so there is no drift between the shipped
  copy and an installed one, and no stale copy to detect or repair.

Work products still go to the workspace — a PRD, a spine, a change set are the
user's files and belong in their repo, through governed IO. It is only the
*Method itself* that stays app-local.

The corpus is already here and already sealed: 47 `SKILL.md` files across 228
files under `packages/bmad-foundation/source/method/6.10.0/src/` — four BMM
phase folders (`1-analysis`, `2-plan-workflows`, `3-solutioning`,
`4-implementation`) plus 14 core skills — every one of them tracked in
`runtime-manifest.json` and covered by the hash chain.

**But an earlier revision of this section said the missing piece was "only the
build-time layout step and the config-root wiring", and that understated the
work.** Two things are also required, and both are load-bearing:

- **The installer does not currently carry `source/` at all.**
  `crates/desktop-app/tauri.conf.json:39-47` bundles `normalized/`, `runtime/`,
  `licenses/`, both ledgers and `NOTICE.md`. Shipping the Method means adding
  `source/` to `bundle.resources`.
- **What *is* bundled contains no skills.**
  `packages/bmad-foundation/runtime/` holds 36 flattened
  `*.instructions.md` files and **zero** `SKILL.md`. Today's installer therefore
  ships nothing OpenCode's skill discovery would recognise. The layout step is
  not a reshuffle of something already present — it is the first time the
  skill corpus enters the installer.

Still no second installer. But this is a bundle-size and packaging change, not
just wiring, and it interacts with the installer-size question in the risks
table.

Three tiers, with different rules:

| Tier | Where | Verified how |
|---|---|---|
| **Shipped** | `packages/bmad-foundation/source/…`, laid out at build time into the app-local config root and carried by the one installer | Hash-chained, immutable at runtime, re-verified at load |
| **Authored — global** | the same app-local root, user area | Governed content. Not hash-pinned; survives app upgrade |
| **Authored — project** | the workspace, discovered by OpenCode's own project walk-up | Governed workspace content, in the user's repo because it is theirs |

The shipped tier never leaves the app, so there is no installed copy to drift
from it and nothing to detect or repair — the verification problem is removed
rather than solved. What Forge still owes the user is the *inverse* check: the
app-local root must be re-verified against the hash chain at load, so a
tampered-with Method is refused rather than run.

The authored tiers mirror upstream's own global-versus-project split, which is
the behaviour a BMAD user already expects: an agent you want everywhere lives
with the app; an agent that belongs to one codebase lives in that codebase and
is committed with it.

Adaptation is permitted and expected — layout, naming, and host-specific
wiring — but **the Method itself does not change**: its phases, its skills,
its agents, its workflows, and the sequence they impose are upstream's.
Divergence in the Method is a defect, not a feature.

The seal must stop answering *"what can the model do?"* — that is now the
permission rule set, evaluated per call — and keep answering *"what text did we
ship, and where did it come from?"*, which matters more once the agent can act.

### D6a — What actually blocks "exactly as the source repo"

A dedicated fidelity review returned **"no — not as written"**, and the counts
below are ones I re-ran myself against
`packages/bmad-foundation/source/method/6.10.0/src/`. The finding is not that
D6 is wrong in ambition; it is that D6's cleanliness property — *nothing written
to the user's repository* — and the fidelity requirement — *the Method behaves
exactly as it does upstream* — are in direct tension, because upstream's skills
are written against a tree that upstream's installer creates **inside the user's
project**.

| # | Blocker | Verified |
|---|---|---|
| B1 | **39 of 47** `SKILL.md` reference `{project-root}/_bmad/…`; the same 39 shell out to `uv run {project-root}/_bmad/scripts/*.py` | counted |
| B2 | **Zero** `.py` files are vendored. Upstream ships 16 under `src/scripts/` (10 scripts + 6 tests); skills invoke three by name — `resolve_customization.py`, `resolve_config.py`, `memlog.py` | counted |
| B3 | **36 of 47** `SKILL.md` read `config.yaml`, which upstream's installer *generates* — it is not a vendored file | counted |
| B4 | `bmad-help` reads a generated `bmad-help.csv` | relayed |
| B5 | D6's "Authored — global" tier has no reader — nothing resolves it | relayed |
| B6 | `bmad-customize` hard-stops when `_bmad/` is absent | relayed |
| B7 | The **Builder is a separate upstream project** | counted |

**B7 deserves restating, because the review understated it.** The Builder was
not omitted from vendoring by oversight — it is not in BMAD-METHOD at all.
`bmad-modules.yaml` registers it as `bmad-builder`, code `bmb`, npm package
`bmad-builder`, sourced from its own repository. Upstream's installer fetches it
as an optional module. The user's requirement that the Builder work "exactly as
the repo" therefore means **adopting a second upstream project**: new licence
and attribution surface, new entries in `runtime-manifest.json` and the semantic
ledger, and a reseal. That is a distinct scope item, not a folder to copy, and
this packet currently does not budget for it.

**Two consequences the packet must absorb:**

1. **A Python/`uv` runtime dependency.** 39 skills shell out to `uv run`. Forge
   ships neither Python nor `uv`. Either they are bundled (installer size, and a
   second toolchain to seal and update), or the three scripts are reimplemented
   host-side and the skills adapted — which is Method divergence, exactly what
   D6 forbids — or those skills degrade. Pick one deliberately; do not discover
   it in Phase 4.
2. **`_bmad/` has to exist from the skills' point of view.** The recommended fix
   is a **virtual `_bmad/` overlay in the D3 governed-IO layer**: reads of
   `{project-root}/_bmad/**` resolve to the app-local sealed tree plus a
   per-workspace generated `config.yaml`, without those bytes ever landing in
   the user's repo. This collapses B1, B3, B4, B5 and B6 into one mechanism and
   preserves D6's cleanliness property. It does **not** address B2's missing
   scripts or B7's separate project.

Until B2 and B7 are resolved, the honest statement of scope is that Forge
delivers the Method's **skills, phases and sequence** faithfully, and does not
yet deliver the **script-backed customization layer** or the **Builder**. That
belongs in the definition of done rather than in a later surprise.

**Licence and marks.** BMAD-METHOD is MIT (© BMad Code, LLC), which permits
copying, modifying and redistributing with attribution, and upstream's own
trademark notice explicitly permits forking and distributing your own version
under a different name. Forge does exactly that: it is named Forge, uses BMAD
marks descriptively only, and claims no endorsement — the carve-out already in
`THIRD_PARTY_NOTICES.md` covers this and must stay accurate. Two obligations
follow from shipping a *modified* copy: the notice must say adapted-from rather
than implying it is upstream, and the outstanding caveat already recorded there
— that adopted bytes must be pinned to an immutably identified upstream release
before public distribution — has to be discharged, not carried forward.

**D7 — Consent becomes turn-scoped, and the change is written down. IMPLEMENTED
— and building it corrected the design below.**

One reviewed manifest per model call cannot survive an agent loop, and the chat
lane's justification for auto-approval — *"the reply can only be text"*
(`chat.rs:415-418`) — is exactly what this pivot deletes. Approve once per user
turn against a **budget and scope** (max model calls, max outbound bytes,
lifetime), and show a running per-turn egress ledger. The alternative —
unreviewed per-call manifests with the decision moved onto the tool prompt — is
where the code drifts by default, and it is security theatre: the tool prompt
shows a command line, not what context is about to be shipped.

**The correction: "one decision reused across the turn" is not implementable,
and should not be.** A `PendingContextDecision` is bound to a manifest hash, and
`manifest_hash` is `canonical_hash` over the manifest *including its items*
(`manifest.rs:257`). The reviewed context **grows every iteration**, because
each tool call and result joins the transcript. So iteration 2's manifest hash
necessarily differs from iteration 1's, and a decision minted for the first
cannot authorise the second without severing the binding between consent and
what was actually sent. That binding is the property worth having; severing it
would turn *"the user approved this context"* into *"the user approved
something, once, a while ago"*.

So the implemented shape keeps the existing single-use machinery **completely
untouched** and adds a layer above it. `TurnEgressGrant`
(`desktop-egress/src/turn_grant.rs`) holds the bounds the human agreed to once;
every call inside the turn still mints and consumes its own correctly-bound,
single-use decision. The grant enforces the budget and records what each call
actually sent.

Bounds as built: a hard ceiling of `MAX_TURN_MODEL_CALLS = 16` that a caller
cannot raise, a lifetime, and a grant that is **not `Clone`** — matching
`PendingContextDecision`, because a copyable budget is not a budget.

**There is no byte ceiling.** An earlier revision of this section listed "max
outbound bytes" among the bounds; that was never implemented and the sentence
was wrong. `authorize_call` checks expiry and call count only. Byte totals are
reporting, not enforcement, and they are summed **without deduplication** —
the transcript is re-sent each iteration, so the honest figure is the sum. Note
it counts *reviewed-context* bytes, not HTTP body size: the envelope,
instructions and provenance fence are excluded, so it is a lower bound.

**Recording never refuses.** `record_call` always appends and reports
`WithinGrant` or `Overrun`. The tempting alternative — re-check the bounds and
reject — loses exactly the entries that matter: a call authorised at t=59s on a
sixty-second grant whose round trip returns at t=62s has already put bytes on
the wire, and dropping it would make the ledger under-report precisely when a
turn misbehaved. The budget is enforced *before* the send, by `authorize_call`.

**What the integration must still guarantee**, and does not yet — the grant is
not wired into `ChatCoordinator`:
- exactly one grant per turn, owned by `InFlightTurn`, never duplicated;
- `record_call` on **every dispatch that was sent**, including failure paths.
  Egress is caused by the send, not by the send succeeding — so the current
  `abort_turn` path, which discards everything, would otherwise turn every
  failed turn into unaccounted egress;
- entry fields derived from the sealed manifest rather than passed by hand.
  They are currently `pub`, so the ledger is a self-report; a crate-owned
  constructor taking `&ContextEgressManifest` would close that.
- the ledger records how much left, never *where it went* — there is no
  `region` on an entry, so the region-binding claim needs that added before it
  holds across a looping turn.

**D8 — ADR-0010 written, then rewritten. `docs/adr/ADR-0010-native-agent-runtime-and-turn-scoped-egress.md`, status proposed.**

The first draft recorded the sidecar architecture and named the two CI gates it
would force open. That draft is **withdrawn** along with D1. The ADR now records
the native runtime instead, and the consequences invert:

- ADR-0009's *"tools are executed by the host, never by the renderer"* stays
  literally true, so the ADR amends rather than contradicts it.
- **Neither CI gate changes.** `bundle.externalBin` keeps failing on any
  non-empty value (`check-boundaries.mjs:1297-1302`) and the spawn allowlist
  keeps its two entries (`check-boundaries.mjs:89-92`). A gate that never has to
  be weakened is worth more than one weakened carefully.
- The boundary becomes a **real enforcement boundary**, because there is no
  co-process retaining direct IO. The earlier draft had to disclaim this.

It still resolves O1 (host holds the credential), still documents turn-scoped
egress and why decision reuse is neither possible nor right, and still declines
to supersede ADR-0006 because the Builder is a separate upstream project.

What it now carries instead of gate amendments is the honest cost: **fidelity is
Forge's to prove.** `shell` bypassing the read denylist and the governed-write
journal remains accepted knowingly and stated, not discovered later.

Without this, the plan's own citation of ADR-0009 as its governing decision is
false, and the first engineer to read both will not know which one binds.

## The sidecar↔host contract

This is the interface the whole architecture rests on, so it is specified here
rather than discovered during Phase 3.

Model it on the `desktop-ipc` **envelope** discipline, not on `LocalCommand`
itself: `deserialize_strict_json` (`envelope.rs:130`), a `deny_unknown_fields`
wire struct (`:72`, and 33 payload structs beside it), and a hand-built parse
from an already-strict `Value` into a closed tagged enum (`:418`, producing
`command.rs:116`). `LocalCommand` carries no `deny_unknown_fields` and is never
deserialized from wire bytes — the strictness lives one layer up, and that layer
is the part worth copying. **Nine operations.
Nothing else.** Every new operation is a deliberate widening of what a
compromised sidecar can reach.

Request: `{callId, op, args}`. `callId` is sidecar-chosen and host-deduplicated,
so a replayed frame cannot re-drive an effect.
Response: `{callId, ok: true, data}` or `{callId, ok: false, error: {code, message}}`
over a **closed** code set mapping onto existing errors —
`permission_denied | outside_workspace | path_blocked | invalid_path |
not_found | already_exists | stale_preimage | limit_exceeded |
unsupported_text | grant_unavailable | edits_not_enabled | timed_out |
spawn_failed | invalid_argument | unavailable`.

A denial is a **normal response, not a transport error**: the model is told and
may reason about it (ADR-0009 §3).

| Op | Args | Permission `tool` | `subject` shown to the user |
|---|---|---|---|
| `fs.read` | `path`, `maxBytes?` | `read` | normalized relative path |
| `fs.list` | `path`, `cursor?`, `limit?` | `list` | normalized directory path |
| `fs.glob` | `pattern`, `limit?` | `glob` | the pattern |
| `fs.grep` | `pattern`, `literal`, `pathGlob?`, `limit?` | `grep` | the pattern |
| `fs.propose_edit` | `edits[{path, op, content?}]` | `write`/`edit`/`delete` | the path, one prompt each |
| `shell.run` | `command`, `relativeDir?`, `timeoutMs?` | `shell` | **the literal command line** |
| `git.status` | — | `git` | `status` |
| `git.diff` | `path`, `comparison` | `git` | `diff <path>` |
| `net.fetch` | `url` | `webfetch` | `scheme://host/path`, query stripped |

**The `subject` discipline is easy to get wrong and security-relevant.** The
host-normalized value must be the *same string* that (a) `glob_match` matches,
(b) `PermissionRequest.subject` renders, and (c) `always_pattern` derives from.
Matching a normalized path while displaying the sidecar's raw input is
approval laundering.

**Never accept from the sidecar**, because these are host authority facts:

- absolute paths, drive prefixes, UNC, `\` separators, `..` — reuse
  `normalize_relative_path` (`crates/desktop-workspace/src/lib.rs:1156-1166`)
  rather than re-validating in TypeScript;
- `workspaceId`, any epoch, `rootIdentityHash`, `workspaceTargetHash`;
- the shell working-directory *root* — only `relativeDir`;
- preimage or identity hashes as authority (ADR-0007 §3 already settles this:
  a model-declared preimage is a staleness tripwire, never authority);
- `displayedDiffHash`, `candidateHash`, `approvalId`, nonces, timestamps;
- permission actions, rules, or any claim of prior approval;
- **prompt copy.** `title` and `detail` are tool-authored by construction
  (`crates/desktop-tools/src/permission.rs:145-154`). A sidecar-supplied
  justification string in a consent dialog is a social-engineering primitive.

Reuse the bounds that already exist rather than inventing a second set: 512 KiB
read, 1000 results, 4 MiB scan budget, 1024-byte paths, 120 s shell timeout,
64 KiB per stream, 1 MiB governed file, 20 files and 1 MiB per proposal. Add
what does not exist: a frame cap before parse, a per-turn tool-call ceiling, and
a per-turn cumulative-bytes ceiling.

## Phases

Each phase is independently landable and independently verifiable. Sizes are
order-of-magnitude, for sequencing only: **S** ≈ days, **M** ≈ 1–2 weeks,
**L** ≈ a month or more.

### Phase 0 — Unblock concurrency (no sidecar, no tools) — M

> **Phase 1 has shipped, and that makes this urgent rather than optional — the
> framing below now understates it.** The original advice assumed a turn was
> *one* model call. A turn is now a loop of up to
> `MAX_TURN_MODEL_CALLS = 16`, and the process-global `in_flight` slot is held
> for the whole loop: every round trip, plus every tool execution between them.
>
> The blast radius therefore changed from "one conversation is busy for one
> Azure round trip" to "**every** conversation in the app is blocked for the
> duration of a multi-call agent loop". On an internal deployment where people
> keep several task tabs open, that is the difference between a brief wait and
> the app appearing hung. **Phase 0 is now the next phase.**

**Original framing, retained because the reasoning still holds for a
single-call turn.** A
single tool-using turn works without any of this, because the global
`in_flight` gate (`chat.rs:281-283`) only rejects a *second* concurrent turn.
What you get without Phase 0 is one agent turn at a time, occupying the whole
app, inside a synchronous `host_dispatch` (`commands.rs:247`) that cannot be
cancelled and blocks every other command for its duration. That is tolerable
for a first internal build and intolerable for a shipped one.

Sequence it before Phase 2: a sidecar plus per-run streaming makes the global
slots and the whole-ledger mutex the binding constraint immediately.

- `ChatCoordinator.in_flight` is one `Option` for the whole process
  (`chat.rs:155`, conflict at `:281-283`); `BmadCapabilityCoordinator.active`
  likewise (`capability_coordinator.rs:294`). Both become per-session maps.
- `ContextDecisionEvidence` holds a `MutexGuard` over the entire consent ledger
  for its lifetime (`crates/desktop-egress/src/consent.rs:253-257`). Rework or
  every lane serialises on consent.
- `host_dispatch` is synchronous and blocks on transport
  (`commands.rs:1412-1416`). It must become async before anything can run
  concurrently with it.
- Raise `AdmissionPolicy` (120 req/60 s, `crates/desktop-ipc/src/gate.rs:24-27`)
  and reconsider the 5-minute envelope age (`envelope.rs:23`) — a permission
  prompt left on screen over lunch currently produces an invalid resume.

*Verify:* two task tabs run turns simultaneously; neither returns `Conflict`.

### Phase 1 — Host-side tools, no sidecar — M

The smallest change delivering real user value, and it needs no new process.

Wire the four already-`Allow` tools — `read`, `list`, `glob`, `grep` — through
the **existing** `WorkspaceBroker` into the chat lane, host-driven loop, capped
at N iterations. `glob` and a real `grep` must be built:
`default_rules()` ships `Allow` rules for tools that do not exist
(`permission.rs:234-235`), and
`WorkspaceBroker::search` (`lib.rs:853-933`) is substring-only, no regex, no
path scoping.

**D7 lands here.** It was stated as a decision but given no phase, which was an
omission: the loop above is what breaks the existing consent model. Today one
reviewed manifest authorises exactly one model call, and
`PendingContextDecision` is single-use with a five-minute expiry. A loop capped
at N iterations makes N model calls, so under today's rules the user is
re-prompted every iteration — or, worse, the loop is quietly exempted from
accounting. Turn-scoped consent is therefore a Phase 1 deliverable, not a later
refinement, and the egress ledger it implies is what keeps ADR-0009's dropped
*accounting* half from staying dropped.

**Close the two permission holes first**, because the defaults are currently
weaker than what already ships:
- `grep`/`glob` are allow-by-default with **no** secret exclusion, while
  `WorkspaceBroker` enforces `is_sensitive_relative_path` *structurally* on
  `read_text` (`lib.rs:757`), `git_diff` (`:833`) and `search` (`:888`).
  Backing them with the broker closes it nearly for free.
- Requires a tool-result role in `ChatSession` (two roles today,
  `crates/desktop-runtime/src/chat.rs:40-45`, strict alternation enforced at
  `:193-209`) and a store migration.

**The model-output contract has to change, and an earlier revision of this phase
omitted it.** There is currently no way for a tool call to *reach* the host:
`parse_chat_turn` (`crates/desktop-app/src/chat.rs:662-681`) strict-parses the
reply and rejects **any** key other than `schemaVersion` and `text`
(`:668-672`). A tool call is by construction a third key, so today it is
`OutputRejected`. Phase 1 must widen this shape deliberately — and widening it
is precisely what `replies_outside_the_closed_shape_fail_closed`
(`chat.rs:850`) exists to prevent, so that test must be *rewritten*, not
deleted, preserving its real intent: authority-bearing fields like `apply` and
`changes` stay refused while a tool-call field becomes legal.

**Wire `desktop-tools` into the graph.** It is currently an **orphan crate** —
listed as a workspace member (`Cargo.toml:10`) and depended on by nothing. Its
permission gate and shell bounds are dead code until `desktop-app` takes a
dependency on it. That wiring belongs in this phase, not to an assumed earlier one.

**`apply_patch` is the default path, not an edge case.** Forge's default model
is `gpt-5-mini` (`crates/desktop-app/src/bmad_model/config.rs:10`), which
matches OpenCode's swap guard (contains `gpt-`, is not `gpt-4*`, is not `*oss*`).
On a default install `edit`/`write` are therefore *already* replaced by
`apply_patch`. Any plan treating the swap as a corner case has it backwards —
`apply_patch` is the path that has to work first.

*Verify:* a chat turn reads its own context; `.env` still refuses structurally;
a tool call survives `parse_chat_turn` while `{"apply":true}` and
`{"changes":[]}` still fail closed.

### Phase 2 — The sidecar — M

**Blocked on ADR-0010 (D8) and on O1.** Do not start until both are settled:
ADR-0010 authorises the architecture and unblocks the two CI gates that
currently forbid it, and O1 decides whether the sidecar holds the provider
credential — which determines the transport message schema.

- Amend the boundary checker deliberately: `externalBin` becomes an exact-value
  assertion rather than a blanket prohibition, and the spawn allowlist gains a
  third entry with a written justification naming the bounds that file owns.
- Spawn under a Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`; requires
  adding `Win32_System_JobObjects` to the `windows` crate features. Also fixes
  a pre-existing leak: `child.kill()` in `shell.rs` does not kill a `cmd /C`
  process tree.
- stdio NDJSON, 64 KiB line cap, both streams drained concurrently, `hello`/
  `ready` handshake carrying a protocol version and a 32-byte nonce passed in
  the frame — **not** on the command line, which is world-readable via WMI.
- Health: `ping`/`pong` with a deadline, because a wedged JS event loop keeps
  the process alive but stops answering.
- Restart with backoff and a circuit breaker. In-flight tool calls do **not**
  survive a restart, and any permission decision for the dead sidecar is
  discarded, never replayed.
- Point `$OPENCODE_CONFIG_DIR` at a Forge-private root so Forge's shadow tools
  and config participate in discovery without touching the user's `.opencode`.
- **Set `directory` on every request** — query param or `x-opencode-directory`.
  It defaults to the *server's* `process.cwd()`
  (`middleware/workspace-routing.ts:87`), which for a spawned sidecar is wrong.
  This is the single most likely "BMAD isn't showing up" bug.

*Verify:* kill the host process and confirm the sidecar dies with it; wedge the
sidecar's event loop and confirm the health check fails rather than hanging;
restart it mid-run and confirm no permission decision is replayed.

### Phase 3 — Tool delegation and the permission bridge — M

- Shadow `read`, `write`, `edit`, `glob`, `grep`, `bash`, `apply_patch`,
  `webfetch` as `.js` in `$OPENCODE_CONFIG_DIR/tool/`; each round-trips to Rust
  over the D3 surface.
- Bridge `permission.asked` → `desktop_tools::permission::evaluate` → the Forge
  prompt → `POST /permission/:id/reply` with `once`.
- Ship ripgrep rather than letting OpenCode fetch it from GitHub on first use
  (`core/src/ripgrep/binary.ts:104-108`) — the `skill` tool needs it to
  enumerate a skill's files, so BMAD skills fail without it.
- Handle the documented leaks explicitly: `prompt.ts:813`/`:266` builtin `read`,
  and the `gpt-*` `apply_patch` swap.

*Verify:* every effectful call reaches Rust — assert by denying everything and
confirming the agent can do nothing at all. Then allow `read` only and confirm
`write` still stops. A leak shows up as an effect with no corresponding host
evaluation, so log every evaluation and diff it against observed effects.

### Phase 4 — BMAD full fidelity — L

- **Lay the Method out at build time** (D6), into the app-local config root the
  one installer carries. Skills go to `skills/<name>/SKILL.md` with their
  `assets/` and `references/` intact — the config-root pattern is
  `{skill,skills}/**/SKILL.md`, which is *not* the `.agents/skills` project
  pattern, so getting this layout wrong means silent non-discovery.
- Preserve each skill's frontmatter `name`: it is the discovery key, and
  duplicates resolve nondeterministically because loading runs unbounded.
- **No workspace write, and no runtime install step.** The Method is app
  payload; there is nothing to install, so nothing to make idempotent and no
  installed copy to drift.
- **Verify the app-local root at load** against the hash chain, and refuse to
  run a tampered Method rather than running it. This is the inverse of the
  drift check the project-install route would have needed, and it is cheaper.
- Point `$OPENCODE_CONFIG_DIR` at that root (Phase 2 already does this for the
  shadow tools; the Method rides the same mechanism).
- Route the **authored** tiers: global authoring writes to the app-local root,
  project authoring to the workspace through governed IO. The Builder must be
  able to target either, and must say which.

*Verify:* open a workspace that has never seen Forge, and run a Method
capability end to end with **nothing written to that repository** except the
work product. Tamper with a file in the app-local root and confirm load refuses
rather than runs. Exercise the virtual `_bmad/` overlay (D6a) directly: a skill
that reads `{project-root}/_bmad/config.yaml` must succeed, and the file must
not exist on disk in the workspace afterwards.

The acceptance test **cannot** be "the Builder authors a new agent that then
loads" — an earlier revision said it could. The Builder is a separate upstream
project (B7) and is out of scope here. Substitute: author an agent by hand into
each authored tier and confirm both are discovered, which exercises all three
tiers without depending on the Builder.
- **ADR-0006** (Builder output is inert) can only be superseded once the Builder
  is actually adopted. Until then it stands, and the packet says so rather than
  implying the Builder ships.
- Re-check ADR-0005 and ADR-0008 for clauses assuming a closed, pre-reviewed
  capability set.
- Regenerate the sealed projections. Measured from
  `packages/bmad-foundation/semantic-source-ledger.json` (294 source members):
  243 carry an `adopt`, **96 carry an `adapt`**, 70 a `reject`, 16 a `defer`.
  Of the 96, **45** have an adapt rationale naming a withheld authority —
  counted by matching the rationale text against *subprocess, web retrieval,
  web fetch, file write, file read, subagent, process, external handoff,
  execution*. That keyword method is approximate and the exact figure will move
  with the phrasing; re-derive it before using it as a target. ADR-0009 grants
  four of the six withheld intents, so the success metric is
  *adapt-for-withheld-authority count → 0*.
- `ADOPTED_INSTRUCTION_BOUNDARY` (`bmad_capability_host.rs:470-482`) currently
  tells the model it cannot run scripts or read files. It becomes false in Phase
  1 and must be **generated per run from the granted tool set**.

*Upgrade order, grounded in upstream definitions:* `code-review` first
(read-only, and it forces the parallel-reviewer mechanism that PRD, brief and
architecture all depend on), then `generate-project-context` — it is the skill
that *writes* `project-context.md`, which `bmad-architecture` holds as
`persistent_facts` and calls "load-bearing for brownfield"
(`3-solutioning/bmad-architecture/SKILL.md:53`). Note it lives in **phase 3**,
not phase 1, so it is reached on the way to `architecture` rather than early
alongside `document-project` — which *consumes* `project-context.md`
(`bmad-document-project/customize.toml:34`) and produces `index.md`,
`project-overview.md`, `source-tree-analysis.md` and `project-parts.json`. Then
`architecture`, then `dev-story`, with `prd`/`product-brief` on a parallel
track.

### Phase 5 — Renderer — L

Pure, already-tested logic ports first with no framework risk. Three modules are
confirmed to exist and be tested — event coalescing
(`packages/app/src/context/server-sdk.tsx:40`), `markdown-stream.ts` (110 lines)
and `pages/layout/helpers.ts` (150 lines). The "~500 lines" total an earlier
revision quoted is an estimate, not a measurement: error-text extraction,
tool-arg display and part grouping have no modules under those names, so budget
them as unscoped until someone looks. Then the permission dock (replacing the composer while up, never
a modal — a modal hides the transcript the decision depends on and trains
dismissal), tool cards, streaming turn, retry banner, cancel.

Needs a host→renderer push channel: a `tauri::ipc::Channel<T>` per run. The
projection lane cannot carry deltas — 1024 slots polled at 1.5 s, and a gap
triggers a full re-bootstrap (`App.tsx:1668-1671`).

Deltas for one part must arrive in order; `message.part.delta` appends to what the
renderer already holds, so a reorder silently corrupts text. If the channel
cannot promise ordering, carry a per-part sequence and drop out-of-order frames.

*Verify:* a turn streams token by token; Stop halts it mid-stream and the
transcript keeps what arrived; a permission prompt blocks the composer and
cannot be dismissed by Escape or click-away.

## What breaks in the existing suite

737 Rust tests pass today. This pivot invalidates some of them *by design*, and
the dangerous case is a test whose intent is still valid even though its
assertion is not.

- **`chat.rs` `replies_outside_the_closed_shape_fail_closed`** rejects
  `"apply": true` and `"changes": []` as smuggled authority-bearing fields. A
  tool call **is** an authority-bearing field. This test does not get fixed; it
  gets deleted. **Its intent must be re-expressed** — the model still must not
  be able to smuggle authority through an unexpected key; what changes is that
  some keys are now expected. Deleting it without a replacement silently drops
  the property.
- **`ChatSession` tests** assume two roles and strict alternation
  (`crates/desktop-runtime/src/chat.rs:193-209`, whole-turn pruning at
  `:238-251`). A tool-result role changes all of them.
- **Store tests** persist `role` as `user`/`assistant` and bail on anything
  else. New roles are a migration.
- **Catalog tests** assert byte-identical command lists across **four** sites,
  not three. `check-boundaries.mjs` parses each one independently:
  `commands.rs` `READY_COMMANDS` (`:1109`), `contracts.ts`
  `desktopHostCommands` (`:1137`), the expected literal array *inside*
  `commandCatalog.test.ts` (`:1151`), and `envelope.rs` `is_known_command`
  (`:1162`). The test file is a pinned copy in its own right, not merely an
  assertion over the others — so every new command touches all four, and
  `READY_COMMANDS: [&str; 46]` means the arity changes too.
- **`desktop-cloud`** pins request/response shape; tool-calling changes it.
- **Least affected:** `desktop-workspace`, `desktop-execution`,
  `desktop-airlock` — which is the evidence that D5 (reuse the governed write
  path) is the right call.

**Two gate traps worth knowing before they bite.**

*First*, `pnpm verify:source` does **not** run `rust:test` or `rust:clippy` —
only `verify:deferred-full`, the desktop CI workflow
(`.github/workflows/desktop.yml:46`) and the nightly security workflow
(`.github/workflows/security-nightly.yml:73`) do. The default local verify stays
green while the Rust suite is broken. Use `verify:deferred-full` throughout this
work.

*Second, and undocumented until now:* the Rust suite compiles **only** under
`--all-features`. Every gate that runs it passes that flag, so the breakage is
invisible in CI. A plain `cargo check --tests -p desktop-app` fails at HEAD
(`9a5bfa36`, clean tree) with three `E0425` errors: `commands.rs:3951` and
`:4005` call `run_capability_to_completion`, which is defined behind
`#[cfg(feature = "deterministic-help")]` at `:3886`, and
`propose_bmad_capability_changes`, which is defined at `:2012` but missing from
the test module's `use super::{…}` at `:3258`. Confirmed by running it. Anyone
who reaches for the narrower per-crate command while iterating hits a failure
that has nothing to do with their change — so fix this *before* Phase 1, not
during it.

## Open decisions

**O1 — Does the sidecar hold the provider credential? — RESOLVED: Option B, the
host performs every model call.**

The deployment context settles this. Forge is an **internal organisation
application** whose sanctioned provider is **Azure AI Foundry**, and that
collapses the trade-off the original framing assumed.

Option A's advantages were *streaming* and *provider breadth*. Breadth is worth
nothing here: there is exactly one sanctioned provider, and the host **already
speaks it**. `DirectProviderKind::AzureOpenAi` is implemented today
(`direct_provider.rs:112-130`), builds the Azure OpenAI **v1** endpoint
(`https://{resource}.openai.azure.com/openai/v1/`), carries a region for the
egress manifest, holds the credential in zeroizing storage that is never
returned or serialised, and is covered by shape-pinning tests. Nothing needs
building to keep the provider loop host-side.

Streaming survives too: the host streams from Azure and forwards deltas over the
Phase 5 channel. The sidecar never needs the socket, only the deltas.

*The performance objection does not survive contact with the numbers.* The
latency of a model call is dominated by the network round trip to Azure —
hundreds of milliseconds. The extra hop Option B introduces is a local IPC frame
between two processes on the same machine: microseconds to low milliseconds,
three orders of magnitude smaller. Paying that to keep the credential out of a
process running third-party TypeScript is not a security-versus-performance
trade at all; it is a rounding error bought with a real property.

So: **the sidecar proposes model calls, the host performs them.** The credential
never enters the sidecar's address space, and a compromised sidecar cannot
exfiltrate what it never holds. The transport schema follows from this — a
`model.call` operation carrying the prompt and returning the completion, not a
credential handoff.

**O1a — Azure auth. DECIDED: static `api-key` for now, with a named seam for
Entra ID.**

The shared key ships for the first internal release. The costs below are
therefore **accepted knowingly rather than overlooked**, and that distinction is
the reason this is written down: no expiry, no per-user attribution in Azure's
logs, and no conditional-access enforcement. Before Forge carries real
organisational data, this should be revisited — a shared key means an audit
trail that cannot answer "who asked for that?".

The seam is in place: `ProviderAuth` (`direct_provider.rs`) names the scheme
separately from the provider, `DirectProviderKind::auth()` is the single place
the choice is made, and the transport matches on it in one spot. Adding Entra is
a new arm plus a token source.

*The header is the easy part; the hard part is lifetime.* `DirectProviderConfig`
currently stores a credential that never expires and is captured once. Access
tokens are short-lived, so Entra needs acquisition **and refresh**, plus a
`CloudError` variant that distinguishes "could not acquire a token" from "the
model rejected the request" — otherwise the user is told to fix the wrong thing.
Scope is `https://cognitiveservices.azure.com/.default`.

*Original analysis follows.*

**Why Entra ID is the right eventual answer.**
Azure auth today sends a static shared key in the `api-key` header
(`direct_provider.rs:326`), sourced from `AZURE_OPENAI_API_KEY` or supplied at
runtime. For an internal organisation app on Entra-joined Windows machines that
is the weakest remaining link:
- a shared key gives **no per-user attribution** — every call looks identical in
  Azure's logs, which defeats the audit trail an internal deployment needs;
- it does not expire, so revocation means rotating a secret for everyone;
- it bypasses conditional access entirely.

Entra ID bearer tokens fix all three: short-lived, per-user, revocable, and
subject to organisational policy. The change is small at the call site — the
`api-key` header becomes `Authorization: Bearer` — and the real work is
acquisition and refresh. The repo already has Windows-native security plumbing
to build on (`windows_broker.rs:102-132`: named pipe, DACL, PID check).
**This is a scope addition, not part of Phase 1-5 as written.** It should be
decided before the app carries real organisational data.

**O2 — What happens to the existing capability lane?**
Once chat turns can call tools, the justification for a separate one-shot
capability lane disappears — its own doc says the split exists because "a chat
turn produces text and nothing else". Either the capability lane collapses into
the chat lane as a governed session type, or the two are maintained in parallel
indefinitely. The first is almost certainly right and is a large refactor;
scoping it is Phase 4 work, but the decision shapes Phase 1.

**O3 — Do we ever ship option B (fork and strip)?**
Option A's boundary is not an enforcement boundary. If Forge's positioning
depends on it being one, option B is not optional and should be scheduled, not
left as "planned follow-on".

## Abort criteria

Stop and reconsider rather than pushing through if any of these hold:

- **The sidecar cannot be contained well enough for the trust claim being made
  in product copy.** Either weaken the copy or schedule option B. Do not ship a
  claim the architecture does not support.
- **Streaming plus cancellation costs more than Phase 0–3 combined.** That is
  the signal that the seam is in the wrong place, and the fallback is the
  provider-adapter seam — a stateless sidecar that receives a sealed request and
  returns a sealed response, holding no conversation state and no tool loop.
- **BMAD fidelity requires forking OpenCode anyway.** The main reason to run it
  unmodified is that BMAD works on it as-is; if that stops being true, option D
  (own loop, reuse `@opencode-ai/llm`) becomes competitive again.
- **A Method skill turns out to require project-local installation** to work at
  all — for instance because a workflow resolves a path relative to the repo
  rather than to the skill. That would reopen the project-install route and its
  drift problem, and it should be discovered by running real capabilities in
  Phase 4 rather than assumed away now.

## Risks

| Risk | Handling |
|---|---|
| The sidecar performs IO regardless of shadowing | D2. State the claim as audit + UX, never enforcement, until option B lands |
| Egress accounting silently dropped | D7. Turn-scoped budget manifests plus a visible per-turn ledger, written into the ADR |
| Prompt injection reaching a tool call | **Delimiter half done.** Context items were joined with a fixed, forgeable delimiter; they are now fenced with a 128-bit per-request marker plus an explicit provenance rule telling the model to trust only that marker, with a request refused outright if content already contains it. Still open: persona and workspace content share one block, so separate them |
| `shell` bypasses the read denylist and the write journal | Real and unavoidable under ADR-0009. `type .env` is a `shell` subject, not a `read` subject. State it in the ADR rather than discovering it later |
| Four disagreeing byte budgets (32 KiB msg / 64 KiB shell / 128 KiB transcript / 512 KiB manifest) | One owner, one constants module, host-side truncation with a marker the model can see |
| NSIS may not replace the sidecar on upgrade (reported as tauri#15134 — *unverified here; no network access. Confirm before relying on it*) | Host verifies the sidecar's expected hash at startup and refuses on mismatch |
| Installer size (**unmeasured** — the frequently-repeated "~211 MB" figure has no source in this repo and no built installer exists to check it; `webviewInstallMode: offlineInstaller` is confirmed, the size is not) | Measure first, then plan. Prefer `dist/node` over `bun --compile`. Note D6a adds the `source/` corpus to the bundle, so the number moves |
| SBOM blindness — a downloaded runtime is in neither lockfile | Pin the runtime as a locked npm dependency so it lands in `pnpm-lock.yaml` |
| `pnpm verify:source` does not run Rust tests | Local verify goes green while the Rust suite is broken. Use `verify:deferred-full` throughout |
| The Rust suite compiles only under `--all-features` (verified: `cargo check --tests -p desktop-app` fails with 3 `E0425` at HEAD) | Fix the two `#[cfg]`/import gaps before Phase 1, so per-crate iteration is usable |
| 39 skills shell out to `uv run`; Forge ships no Python and no `uv` (D6a/B2) | Decide bundle-vs-reimplement-vs-degrade explicitly, before Phase 4 |
| The Builder is a separate upstream project, not a folder (D6a/B7) | Scope it out of this packet or budget a second adoption with its own licence surface and reseal |

## Out of scope

- Option B (fork and strip). Planned follow-on; option A is a strict subset.
- Durable cross-restart permission grants — needs its own ADR (ADR-0009 §4).
- Streaming *verified* output. Forge verifies a complete payload against a
  closed schema before it is admitted; streaming means showing unverified bytes.
  That is a governance decision, not a refactor.
- Builder self-activation without a human step. An agent that authors *and*
  activates its own instructions defeats the permission model's audience.
- macOS/Linux. Windows only; note recovery mutation is already `#[cfg(windows)]`.
- Changing the Method itself. Layout, naming and host wiring may be adapted;
  the phases, skills, agents and workflows are upstream's, and divergence in
  them is a defect (D6).
- A second installer, a BMAD setup step, or any per-project install action.
  One installer, and the Method is payload inside it (D6).
- Supporting the platform layouts upstream's installer targets. It emits for a
  dozen IDEs; Forge needs exactly one layout, the config-root one.

## Definition of done

- The Method ships inside the single app installer, is discovered from the
  app-local config root in any workspace with no per-project setup, and is
  runnable end to end with no Forge-specific projection involved.
- The virtual `_bmad/` overlay (D6a) satisfies the 39 skills that read
  `{project-root}/_bmad/**` and the 36 that read `config.yaml`, with **nothing
  written to the user's repository** except work products.
- **Scoped out of this packet, and stated rather than discovered:** the
  script-backed customization layer (B2 — no `.py` is vendored, and 39 skills
  shell out to `uv run`) and the Builder (B7 — a separate upstream project,
  `bmad-builder`/`bmb`, requiring its own adoption, licence surface and reseal).
  Each needs its own decision before it can be scheduled.
- Every effectful tool call is evaluated by Rust and, when unmatched, reaches a
  human before it runs.
- Agent file writes are checkpointed, journaled and undoable through the
  existing D3 machinery.
- A turn's egress is visible to the user as a running ledger.
- The fidelity matrix's *"Faithful upstream execution: still 0."* line moves — one
  capability at a time, and only when that capability genuinely runs its
  upstream loop.
- Full gate green, including `verify:deferred-full`.
