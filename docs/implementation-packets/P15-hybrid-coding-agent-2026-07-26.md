# Implementation packet: P15 — Forge as a hybrid coding agent

## Authority and intent

- Owning authority: repository maintainer (RodrigoSousa0).
- Product intent: Forge becomes a general coding agent of OpenCode's class,
  differentiated by the BMAD Method being its interface rather than by withheld
  capability. The user must be able to run the BMAD Method — methodology,
  skills, agents, workflows — **and the BMAD Builder** exactly as they work in
  the upstream repository.
- Governing decision: [ADR-0009](../adr/ADR-0009-forge-as-a-general-coding-agent.md).
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

**5. Node runs it.** `script/build-node.ts:15-29` targets Node; OpenCode's own
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
- `edit`/`write` are dropped in favour of `apply_patch` for `gpt-*` models
  (`registry.ts:292-295`), so a shadowed `edit` silently disappears.
- The `/file`, `/find`, `/pty`, snapshot, LSP, formatter and watcher routes
  perform IO with no tool involved at all.

So "the sidecar proposes, Rust authorizes" is achievable as **product behaviour
and as an audit boundary**, but it is **not an in-process security property**.
Anyone who states it as one is wrong.

**And ADR-0009 dropped egress accounting without saying so.** Every model call
today passes `ContextPreparer` + `PatternSecretScanner` + a sealed manifest +
a single-use decision (`crates/desktop-app/src/chat.rs:412-448`). The moment
`shell` exists, `curl`, `git push` and `npm publish` are egress paths with no
manifest, no secret scan, no region binding and no receipt. ADR-0009 re-bought
the *authorization* half of ADR-0001 and silently dropped the *accounting* half.

## Design decisions

**D1 — Run `opencode serve` as a sidecar (option A), not a fork.**
It is the only option delivering full BMAD fidelity on day one with no
compatibility risk. Options C and D must re-derive five discovery roots,
walk-up semantics, skills-as-commands promotion and the `<skill_content>`
contract; every drift is a silent BMAD workflow failure. A fork (option B) is
the planned follow-on when the enforcement gaps are worth paying for — the DI
graph supports it (`packages/core/src/effect/app-node-builder.ts:6-17`), and
option A is a strict subset of that work.

**D2 — Contain the sidecar with the OS, not with tool-map surgery.**
Because D1's boundary is not an enforcement boundary, the sidecar runs under a
Windows Job Object for process-tree lifetime, with a scrubbed environment and a
pinned image path. State the containment claim precisely: *process-tree
lifetime control*, never "sandbox". A restricted token or dedicated user is the
follow-on if the trust claim needs to be stronger.

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

**D6 — Forge ships its own copy of the Method, and its own installer.**
Forge does not run upstream's installer against the user's workspace. It
vendors the Method and materialises it itself.

The corpus is already here and already sealed: 47 `SKILL.md` files across 228
files under `packages/bmad-foundation/source/method/6.10.0/src/` — four BMM
phase folders (`1-analysis`, `2-plan-workflows`, `3-solutioning`,
`4-implementation`) plus 14 core skills — every one of them tracked in
`runtime-manifest.json` and covered by the hash chain. The missing piece is
only the installer.

Three tiers, with different rules:

| Tier | What | Verified how |
|---|---|---|
| **Shipped** | `packages/bmad-foundation/source/…` — Forge's own copy of the Method | Hash-chained, immutable at runtime |
| **Installed** | what the installer writes into the workspace (`.agents/skills/`, `.opencode/commands/`) so the sidecar discovers it | Materialised *from* the shipped tier, so Forge can re-verify it against those hashes and report drift |
| **Authored** | what the user or the Builder writes | Ordinary governed workspace content. Not hash-pinned — that is the point |

The installed tier being derived from a sealed source is *stronger* than
upstream's arrangement, not weaker: Forge can tell a user that their installed
Method has been modified, and offer to restore or upgrade it. Upstream cannot.

Adaptation is permitted and expected — layout, naming, and host-specific
wiring — but **the Method itself does not change**: its phases, its skills,
its agents, its workflows, and the sequence they impose are upstream's.
Divergence in the Method is a defect, not a feature.

The seal must stop answering *"what can the model do?"* — that is now the
permission rule set, evaluated per call — and keep answering *"what text did we
ship, and where did it come from?"*, which matters more once the agent can act.

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

**D7 — Consent becomes turn-scoped, and the change is written down.**
One reviewed manifest per model call cannot survive an agent loop, and the chat
lane's justification for auto-approval — *"the reply can only be text"*
(`chat.rs:415-418`) — is exactly what this pivot deletes. Approve once per user
turn against a manifest describing a **budget and scope** (max model calls, max
tool calls, max outbound bytes, workspace subtree), and show a running per-turn
egress ledger. The alternative — unreviewed per-call manifests with the decision
moved onto the tool prompt — is where the code drifts by default, and it is
security theatre: the tool prompt shows a command line, not what context is
about to be shipped.

## The sidecar↔host contract

This is the interface the whole architecture rests on, so it is specified here
rather than discovered during Phase 3.

Model it on `LocalCommand` (`crates/desktop-runtime/src/command.rs:116`): one
closed tagged enum, `#[serde(deny_unknown_fields)]`, parsed with
`deserialize_strict_json` so duplicate keys are rejected. **Nine operations.
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

**Not a hard prerequisite — ship Phase 1 first if you want value sooner.** A
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
`permission.rs:182-183` ships rules for tools that do not exist, and
`WorkspaceBroker::search` (`lib.rs:853-933`) is substring-only, no regex, no
path scoping.

**Close the two permission holes first**, because the defaults are currently
weaker than what already ships:
- `grep`/`glob` are allow-by-default with **no** secret exclusion, while
  `WorkspaceBroker` enforces `is_sensitive_relative_path` *structurally* on
  `read_text` (`lib.rs:757`), `git_diff` (`:833`) and `search` (`:888`).
  Backing them with the broker closes it nearly for free.
- Requires a tool-result role in `ChatSession` (two roles today,
  `crates/desktop-runtime/src/chat.rs:40-45`, strict alternation enforced at
  `:193-209`) and a store migration.

*Verify:* a chat turn reads its own context; `.env` still refuses structurally.

### Phase 2 — The sidecar — M

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

- **Build Forge's own installer** (D6). It materialises the sealed corpus into
  the workspace in the layout the sidecar discovers — skills to
  `.agents/skills/<name>/SKILL.md` with their `assets/` and `references/`
  intact, commands to `.opencode/commands/`. Preserve each skill's frontmatter
  `name`, because that is the discovery key and duplicates resolve
  nondeterministically upstream.
- Install is a **governed write**: it goes through the D3 journal like any other
  change set, so it is reviewable, undoable and crash-safe. Installing a method
  into someone's repository is a workspace mutation and should look like one.
- **Verify and repair.** Because the installed tier is derived from hashed
  sources, Forge can compare an existing install against them and report
  drift — modified, missing, or extra files — then offer restore or upgrade.
  Build this with the installer, not after it; it is what makes shipping our
  own copy better than shelling out to upstream's.
- Re-installing must be idempotent, and must not clobber the **authored** tier.

*Verify:* install into a scratch repo, run a capability end to end, then modify
an installed file and confirm verify reports the drift and repair restores it.
The Builder authoring a new agent that subsequently loads is the acceptance
test for the whole phase — it exercises all three tiers at once.
- Supersede **ADR-0006** (Builder output is inert): the Builder must author
  real agents, workflows and skills into the workspace tier (D6).
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
architecture all depend on), then `document-project` (produces
`project-context.md`, load-bearing for brownfield), then `architecture`, then
`dev-story`, with `prd`/`product-brief` on a parallel track.

### Phase 5 — Renderer — L

~500 lines of pure, already-tested logic port first with no framework risk:
event coalescing, markdown streaming, error-text extraction, tool-arg display,
part grouping. Then the permission dock (replacing the composer while up, never
a modal — a modal hides the transcript the decision depends on and trains
dismissal), tool cards, streaming turn, retry banner, cancel.

Needs a host→renderer push channel: a `tauri::ipc::Channel<T>` per run. The
projection lane cannot carry deltas — 1024 slots polled at 1.5 s, and a gap
triggers a full re-bootstrap (`App.tsx:1668-1671`).

Deltas for one part must arrive in order; `part.delta` appends to what the
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
- **Catalog tests** assert byte-identical command lists across three sites
  (`commands.rs` `READY_COMMANDS`, `envelope.rs` `is_known_command`,
  `contracts.ts` `desktopHostCommands`). Every new command touches all three.
- **`desktop-cloud`** pins request/response shape; tool-calling changes it.
- **Least affected:** `desktop-workspace`, `desktop-execution`,
  `desktop-airlock` — which is the evidence that D5 (reuse the governed write
  path) is the right call.

**A gate trap worth knowing before it bites:** `pnpm verify:source` does **not**
run `rust:test` or `rust:clippy` — only `verify:deferred-full` and the desktop
CI workflow do. The default local verify will stay green while the Rust suite is
broken. Use `verify:deferred-full` throughout this work.

## Open decisions

**O1 — Does the sidecar hold the provider credential?**
Today the API key enters through one narrow IPC command, moves straight into
zeroizing memory, and is never returned or serialized. If the sidecar runs the
provider loop it must hold the key, and a compromised sidecar exfiltrates it —
a genuine regression this split introduces.
*Option A:* sidecar holds it. Streaming and provider breadth work naturally.
*Option B:* the sidecar proposes model calls and the **host** performs them.
Preserves the property, but reduces the sidecar to a state machine and gives
back much of the reason for choosing it.
Not decidable from the code; it is a product call about the threat model.
**Decide before Phase 2**, because it determines the transport message schema.

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
- **The installed-tier verification cannot be made to work** — e.g. install is
  inherently lossy against the sealed hashes. That removes the main advantage
  of shipping our own copy over shelling out.

## Risks

| Risk | Handling |
|---|---|
| The sidecar performs IO regardless of shadowing | D2. State the claim as audit + UX, never enforcement, until option B lands |
| Egress accounting silently dropped | D7. Turn-scoped budget manifests plus a visible per-turn ledger, written into the ADR |
| Prompt injection reaching a tool call | Context items are joined into one blob with a forgeable delimiter (`direct_provider.rs:426-437`). Nonce-fence the delimiters; never put persona and workspace content in the same block |
| `shell` bypasses the read denylist and the write journal | Real and unavoidable under ADR-0009. `type .env` is a `shell` subject, not a `read` subject. State it in the ADR rather than discovering it later |
| Four disagreeing byte budgets (32 KiB msg / 64 KiB shell / 128 KiB transcript / 512 KiB manifest) | One owner, one constants module, host-side truncation with a marker the model can see |
| NSIS does not replace the sidecar on upgrade (tauri#15134) | Host verifies the sidecar's expected hash at startup and refuses on mismatch |
| Installer size (already ~211 MB with the offline WebView2 runtime) | Measure before planning. Prefer `dist/node` over `bun --compile` |
| SBOM blindness — a downloaded runtime is in neither lockfile | Pin the runtime as a locked npm dependency so it lands in `pnpm-lock.yaml` |
| `pnpm verify:source` does not run Rust tests | Local verify goes green while the Rust suite is broken. Use `verify:deferred-full` throughout |

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
- Installing to platforms other than the one the sidecar reads. Upstream's
  installer targets a dozen IDEs; Forge's needs exactly one layout.

## Definition of done

- Forge's own installer materialises the Method into a workspace, and it is
  discovered and runnable end to end, including the Builder authoring a new
  agent that then
  loads — with no Forge-specific projection involved.
- Every effectful tool call is evaluated by Rust and, when unmatched, reaches a
  human before it runs.
- Agent file writes are checkpointed, journaled and undoable through the
  existing D3 machinery.
- A turn's egress is visible to the user as a running ledger.
- The fidelity matrix's *"Faithful upstream execution: 0"* line moves — one
  capability at a time, and only when that capability genuinely runs its
  upstream loop.
- Full gate green, including `verify:deferred-full`.
