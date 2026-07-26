# Implementation packet: P16 — the native Method runtime

## Authority and intent

> **Status note, 2026-07-27.** Defects **A1** (the tool loop unreachable in production — `seal_chat_manifest` hardcoding the v1 output schema) and **A2** (the packaged app unable to start — `source/` missing from `bundle.resources`) from Phase A below have been **fixed**. The rest of Phase A and every later phase remain plan only.

- Owning authority: repository maintainer (RodrigoSousa0).
- Governing decision: [ADR-0010](../adr/ADR-0010-native-agent-runtime-and-turn-scoped-egress.md). ADR-0009 still binds on *what the agent may do*; ADR-0010 binds on *what runs the Method*.
- Replaces: **P15 Phase 2**, **D3**, **D4**, and the **sidecar↔host contract** section of [P15](P15-hybrid-coding-agent-2026-07-26.md). Those describe a rejected option. The work they stood for — discovery, the tool surface, the permission bridge, the transport bounds — still has to happen, natively, and had no specification until this packet.
- Platform: Windows desktop only.
- Status: **plan only.**

The single sentence this packet exists to make true: **Forge runs upstream's 47 `SKILL.md` files as they are written, from its own Rust host, with nothing of OpenCode's in the runtime path.**

---

## 1. Corrections to P15 the reversal invalidated and the banner does not mark

P15's banner marks Phase 2, D3, D4, and the sidecar↔host contract. These are also dead or wrong and are **not** marked. Listing them because the first engineer to read P15 top-to-bottom will otherwise take the unmarked parts as current.

| P15 location | What is wrong now |
|---|---|
| "What the research settled" §1 (`P15:41-51`) | Concludes *"Full BMAD fidelity is therefore a configuration problem, not a reimplementation problem"*. That is now exactly false and directly contradicts ADR-0010's stated cost (`ADR-0010:117-121`). It is the second paragraph of the packet, above the fold. |
| "What the research settled" §2, §3, §5 (`P15:53-74`) | ACP permission delegation, `{tool,tools}/*.js` shadowing, the `dist/node` bundle. All are properties of a process Forge will not run. |
| "The central problem, stated plainly" (`P15:76-94`) | The whole tool-shadowing half is moot — there is no co-process to shadow. The egress half (`P15:96-102`) survives and is now D7/Phase A. |
| D6, second paragraph (`P15:228-236`) | *"Forge points `$OPENCODE_CONFIG_DIR` at an app-local root … and the sidecar discovers it"*. The **conclusion** (Method is app payload, app-local, no project install) survives. The **mechanism** does not exist. |
| D6 tier table, row 3 (`P15:284`) | "discovered by **OpenCode's own project walk-up**" — no such walk-up exists natively. Forge must define and test project-tier discovery itself, or drop the tier. |
| Phase 3 entirely (`P15:663-679`) | Shadow `.js` tools, `POST /permission/:id/reply`, shipping ripgrep so `core/src/ripgrep/binary.ts:104-108` does not fetch it, the `prompt.ts:813` builtin-`read` leak. All sidecar. **But the work it stood for is the permission dock, and that is now on the critical path** — see §5, Phase C. |
| Phase 4, bullet 5 (`P15:697-698`) | "Point `$OPENCODE_CONFIG_DIR` at that root (Phase 2 already does this…)" — forward reference to a withdrawn phase. |
| Phase 5 (`P15:746-768`) | Cites `packages/app/src/context/server-sdk.tsx:40`, `markdown-stream.ts`, `pages/layout/helpers.ts`. **There is no `packages/app/` in this repo** (`packages/` holds `bmad-fixtures`, `bmad-foundation`, `contracts`, `ui`). Those paths are relative to `library/opencode/`, which is **gitignored** (`.gitignore:40`) and untracked — 0 files under Git. Copying that source is permitted by ADR-0010, but every such citation is unverifiable in CI and invisible to anyone who has not cloned the reference. Mark them. |
| Abort criteria, 3 of 4 (`P15:916-928`) | Sidecar containment, the "stateless sidecar" fallback, "option D (own loop, reuse `@opencode-ai/llm`)". Moot. The **fourth** — a Method skill requiring project-local installation — survives and is now the live risk this packet's §4 is the answer to. |
| Risks table, 4 rows (`P15:936, 941-943`) | Sidecar IO, NSIS sidecar replacement (tauri#15134), SBOM blindness on a downloaded runtime, `dist/node` vs `bun --compile`. Moot. |
| Out of scope (`P15:951, 965`) | "Option B (fork and strip). Planned follow-on; option A is a strict subset" — the A/B framing is gone. "Forge needs exactly one layout, **the config-root one**" — config roots are OpenCode's concept. |
| O1 (`P15:824-856`) | Marked RESOLVED, but the entire resolution is argued in sidecar terms ("the sidecar proposes model calls, the host performs them"). The conclusion is now trivially true and the reasoning is dead weight. |
| O3 (`P15:907-910`) | "Do we ever ship option B (fork and strip)?" — answered by ADR-0010. Close it. |

**Stale because the work landed, not because of the reversal.** Verified against the working tree:

- D7 (`P15:418-419`): *"the grant is not wired into `ChatCoordinator`"*. It now is — `crates/desktop-app/src/chat.rs:476` opens/carries it, `:481-486` builds the entry, `:554-555` and `:667-668` record on both the success and abort paths.
- Phase 1 (`P15:614-617`): *"`desktop-tools` … is currently an **orphan crate**"*. It is now a dependency — `crates/desktop-app/Cargo.toml:29`.
- Phase 0 (`P15:556-558`): *"`ChatCoordinator.in_flight` is one `Option` for the whole process"*. It is now `BTreeMap<u64, InFlightTurn>` keyed by dispatch id — `crates/desktop-app/src/chat.rs:212`.
- Risks (`P15:945`): the three `E0425` errors under a plain `cargo check --tests -p desktop-app`. The `#[cfg]`/import gaps appear closed — `commands.rs:3313` imports `propose_bmad_capability_changes`, and both call sites (`:3985`, `:4027`) sit under `#[cfg(feature = "deterministic-help")]`. **I did not compile, so treat this as "looks fixed, re-verify" rather than fixed.**
- D6 (`P15:269-271`): *"`packages/bmad-foundation/runtime/` holds 36 flattened `*.instructions.md`"*. 36 total is right; 31 are method, 5 are builder.

**Two defects P15 does not name, both of which make the rest untestable.** See Phase A.

---

## 2. What Phase 1 shipped, and must be built on rather than rebuilt

Verified in the working tree (uncommitted; `git diff --stat HEAD` shows 30 files, +2078/−308).

| Asset | Where | What P16 gets for free |
|---|---|---|
| Tool executor | `crates/desktop-app/src/chat_tools.rs:54-108` | The single point where a tool name + subject becomes an effect. Consults `desktop_tools::evaluate` **before** the broker (`:61-64`), bounds results at 8 KiB / 100 rows (`:27-29`), and truncates honestly (`:112-125`). This is the seam the `_bmad/` overlay hangs off — §4. |
| Host-driven tool loop | `crates/desktop-app/src/commands.rs:1414-1469` | Send → `finalize_turn` → either complete or run one tool and `resume_turn`. A refused tool is reported to the model as text (`:1457`), never a turn failure. |
| Turn-scoped egress | `crates/desktop-egress/src/turn_grant.rs` | `MAX_TURN_MODEL_CALLS = 16` (`:45`), not `Clone` (`:82-87`), `authorize_call` before the send (`:133-141`), `record_call` never refuses (`:143-150`). Wired at `chat.rs:476`/`:747-766`. |
| Four bounded read tools | `crates/desktop-workspace/src/lib.rs` | `read_text` `:771-820`, `list_entries` `:494-499`, `glob` `:946-977` (globset, `literal_separator(true)`), `grep` `:995-1048` (real regex, size- and DFA-bounded `:1502-1512`). `is_sensitive_relative_path` applies **structurally** at `:780`, `:859`, and inside `vet_walk_entry` `:1473`. |
| Path normalisation | `crates/desktop-workspace/src/lib.rs:1271-1297` + `validate_segment:1299-1324` | Rejects `..`, `\`, absolute, control chars, reserved device names, >1024 bytes. **Reuse this. Do not re-derive it.** It is currently private; the overlay needs it exported. |
| Permission gate | `crates/desktop-tools/src/permission.rs` | Unmatched = `Ask` (`:132`), last-match-wins, wildcard-allow cannot reach a chaining subject (`:118-133`), `default_rules()` `:227-248`, session grants cleared on sign-out (`state.rs:100`). |
| Four transcript roles + migration | `crates/desktop-runtime/src/chat.rs:50-70`, `crates/desktop-store/src/migrations.rs` | `user`/`assistant`/`tool_call`/`tool_result`, `opens_turn()` (`:83`), whole-turn pruning. |
| The sealed corpus, already in memory | `crates/desktop-app/src/bmad_foundation.rs:426-485`, `:657-684` | **This is the largest gift and P15 does not mention it.** `load_bmad_foundation` reads and SHA-256-verifies *every* one of the manifest's 304 resources — including all 228 files under `source/method/6.10.0/src/` — into `BTreeMap<String, Arc<[u8]>>`. Skill discovery therefore needs **no filesystem walk at all**: the corpus is already verified bytes in RAM, ~1.32 MB, well under `MAX_TOTAL_BYTES = 16 MiB` (`:18`). |
| Governed write stack | `crates/desktop-workspace/src/governed_io.rs` | `enable_governed_edits` `:258-277`, `observe_preimage` `:290-307`, `create/replace/delete_utf8_durable` `:362`, `:396`, `:433`, pinned-handle recovery `:65-213`. P15's D5 (branched approval, no `displayed_diff_hash` reuse) survives the reversal intact. |

---

## 3. The corpus, as measured

Every figure below I re-ran against `packages/bmad-foundation/source/method/6.10.0/src/`. Where it disagrees with P15 or ADR-0010 I say so.

**Shape.** 228 files: 179 `.md`, 37 `.toml`, 6 `.csv`, 3 `.yaml`, 3 `.json`. **Zero `.py`.** 47 `SKILL.md`; 14 under `core-skills/`, 33 under `bmm-skills/` across `1-analysis`, `2-plan-workflows`, `3-solutioning`, `4-implementation`. Pinned by `packages/bmad-foundation/tests/method-fidelity.test.mjs:45-68`.

**Nesting is not uniform.** Three skills sit one level deeper: `bmm-skills/1-analysis/research/bmad-{domain,market,technical}-research/SKILL.md`. Discovery must be a recursive `**/SKILL.md`, not a fixed depth.

**Frontmatter is exactly two keys.** `name` and `description`, on all 47, nothing else. All 47 names are unique, and every one equals its directory basename — which matters because the corpus defines `{skill-name}` as "the skill directory's basename" (`bmm-skills/3-solutioning/bmad-architecture/SKILL.md:45`) while `bmad-customize` derives override filenames from the *skill name*. Those agree today; a Rust test should keep them agreeing.

**The `_bmad/` surface, enumerated exhaustively** (`grep -rho "{project-root}/_bmad/[…]"`, reference counts):

| Path | Refs | Kind |
|---|---|---|
| `_bmad/scripts/resolve_customization.py` | 60 | not a file — a native operation (§5) |
| `_bmad/scripts/memlog.py` | 38 | not a file — a native operation |
| `_bmad/bmm/config.yaml` | 34 | synthesised, read-only |
| `_bmad/custom/{skill-name}.toml` | 25 | read **and write** |
| `_bmad/custom/{skill-name}.user.toml` | 25 | read **and write**, gitignored upstream |
| `_bmad/core/config.yaml` | 4 | synthesised, read-only |
| `_bmad/config.toml`, `_bmad/config.user.toml` | 4 each | read |
| `_bmad/scripts/resolve_config.py` | 3 | not a file — a native operation |
| `_bmad/custom/config.toml`, `config.user.toml` | 2, 4 | read (`bmad-customize` declares central config **out of scope**, `SKILL.md:10`) |
| `_bmad/_config/bmad-help.csv` | 1 | synthesised, read-only |
| `_bmad/style-guides/company-voice.md` | 2 | **comment-only example** in two `customize.toml`. Not a requirement. |

Plus `config.user.yaml` (11 refs) as a sibling of both `config.yaml` files.

**The eight skills that read nothing under `_bmad/`:** `bmad-quick-dev`, `bmad-editorial-review-prose`, `bmad-editorial-review-structure`, `bmad-index-docs`, `bmad-review-adversarial-general`, `bmad-review-edge-case-hunter`, `bmad-review-verification-gap`, `bmad-shard-doc`. 47 − 8 = 39, matching the test.

**The script surface is nine scripts, not three, and forty skills, not thirty-nine.** The fidelity test's regex is scoped to `_bmad/scripts/`, so it correctly pins three *core* scripts but is silent on six more invoked from `{skill-root}`:

| Script | Invoked from | Skill |
|---|---|---|
| `brain.py` | `{skill-root}/scripts/` | bmad-brainstorming (`SKILL.md:49`, `:64`, `references/headless.md:17`, `references/in-chat-techniques.md:10`) |
| `resolve_party.py` | `{skill-root}/scripts/` | bmad-party-mode (`SKILL.md:13`, `:22`, `references/create-party.md:66`) |
| `resolve_personas.py` | `{skill-root}/scripts/` | bmad-forge-idea (`SKILL.md:85`) |
| `list_customizable_skills.py` | `{skill-root}/scripts/` | bmad-customize (`SKILL.md:34`) — invoked as `python3`, not `uv run` |
| `lint_spine.py` | `{skill-root}/scripts/` | bmad-architecture (`references/reviewer-gate.md:5`) |
| `render.py` | `{skill-root}/` | bmad-quick-dev (`SKILL.md:9`) |

Counting by skill directory, **40** of 47 invoke a Python script somewhere in their tree — all 33 BMM skills plus 7 of 14 core. That set is **not** the same as the 39 that read `_bmad/`: `bmad-quick-dev` needs Python and touches no `_bmad/` path at all.

**One non-Python asset is also missing.** `core-skills/bmad-brainstorming/assets/` holds `brain-icons.json` and `brain-methods.csv` — but `SKILL.md:49` names `{skill-root}/assets/brain-selector.html` as "the composer page (**primary**)". It is not vendored. Brainstorming's primary interaction path is already broken independent of the Python question.

**`bmad-quick-dev` is the hardest single case.** Its entire `SKILL.md` body (`:1-12`) is: run `uv run {skill-root}/render.py`, follow what it prints to stdout, and on any failure "report what it printed and **HALT**". The script *is* the skill; the eleven sibling `.md` files (`workflow.md`, `step-01`…`step-05`, `step-oneshot.md`, `spec-template.md`, …) are presumably what it assembles, but the assembly order and any conditional logic are unknown. No option short of adopting `render.py` delivers this skill faithfully.

**What the corpus gives you for free, and P15/D6a does not notice.**

- `config.yaml` is **derivable, not guessable.** `core-skills/module.yaml` declares `user_name` (default `BMad`, scope `user`), `project_name` (default `{directory_name}`), `communication_language` (`English`), `document_output_language` (`English`), `output_folder` (`_bmad-output`, result `{project-root}/{value}`). `bmm-skills/module.yaml` adds `user_skill_level` (`intermediate`), `planning_artifacts`, `implementation_artifacts`, `project_knowledge` (`docs`), a `directories:` list to create, and the six-agent `agents:` roster. Both files are vendored and hash-pinned. Every key has a default, so a zero-input synthesis is possible and correct.
- `bmad-help.csv` is **a concatenation of vendored files.** `core-skills/module-help.csv` (14 lines incl. header) + `bmm-skills/module-help.csv` (31 lines incl. header), one header. Note the catalog names ~33 of 47 skills — that is upstream's own shape, not a gap.
- The customization **merge rules are written down in the corpus**: `bmad-customize/SKILL.md:66-71` ("scalars override wins; append arrays; keyed arrays of tables — matching `code`/`id` replace, new keys append"), repeated as a header comment in every `customize.toml` (e.g. `bmm-skills/2-plan-workflows/bmad-agent-pm/customize.toml:12-14`). 37 `customize.toml` files are vendored, so the base layer is already in memory.
- The **placeholder resolution rules** are written down: `bmm-skills/3-solutioning/bmad-architecture/SKILL.md:43-47`.

**Fidelity is currently zero, and the numbers say why.** The lane that runs today does not run these files. `bmad-architecture/SKILL.md` is 13,888 bytes; what actually reaches the model is `runtime/method/6.10.0/architecture-create.instructions.md`, 4,809 bytes of Forge-authored prose beginning "# Managed architecture-create guidance". The normalized package records the relationship honestly as `"treatment": "adapt"` (`packages/bmad-foundation/normalized/bmad-architecture.package.json`). On top of that, `ADOPTED_INSTRUCTION_BOUNDARY` (`crates/desktop-app/src/bmad_capability_host.rs:470-482`) tells the model it has "**no** ability to run a script or command, read or write a file, spawn a subagent, fetch a URL". P16 is the packet that makes that string false and then deletes it.

---

## 4. Design decisions

### N1 — Skill discovery reads the sealed in-memory corpus. There is no filesystem walk, no config root, and no walk-up.

`load_bmad_foundation` already verifies and holds every `source/**` byte (`bmad_foundation.rs:657-684`). Discovery enumerates keys with the prefix `source/method/6.10.0/src/` ending in `/SKILL.md`, parses frontmatter, and builds a name-keyed index.

This is strictly better than re-deriving OpenCode's discovery. It removes an entire class of failure that ADR-0010 accepted as a cost (`ADR-0010:117-121` — "config roots, the `.claude`/`.agents` walk-up, two distinct skill globs"): none of those concepts need to exist. There is exactly one corpus, it is hash-chained, and a tampered byte fails startup rather than producing a silently wrong skill.

Required: a `resource_paths()` accessor beside `resource_bytes` (`bmad_foundation.rs:305-310`), which today exposes lookup but not enumeration.

Frontmatter parsing: all 47 are two flat keys with optionally single-quoted scalars. A purpose-built parser pinned by a Rust conformance test is defensible and avoids a YAML dependency. **I am not certain this is the right call** — a real YAML parser is ~1 dependency and removes a whole category of bug; the counter-argument is that the parser then accepts shapes the corpus never contains, which is exactly how discovery drifts.

**`{skill-root}` has no on-disk existence under this decision**, and that is deliberate. Skill-relative reads (`references/*.md`, `assets/*.csv`, `customize.toml`) resolve through a skill-scoped reader over the in-memory map, prefix-bounded to that skill's directory. Six of the 47 skills would need `{skill-root}` as an absolute path to hand to a subprocess; those are exactly the six being reimplemented or scoped out (§N6), so 41 skills are unaffected.

The alternative — materialise the verified corpus into an app-local directory at first run so `{skill-root}` is real — buys a future script story and costs the drift-detection problem D6 was pleased to have removed (`P15:288-290`). Recommend against it now; note it as the escape hatch if `render.py` is ever adopted.

### N2 — The model sees a catalog always, and a skill body on demand.

47 names + descriptions is roughly 47 lines and belongs in the turn's instructions. Bodies range to ~14 KB and must not all be resident: `MAX_PROMPT_TRANSCRIPT_BYTES` is 128 KiB (`desktop-runtime/src/chat.rs:37`) and `MAX_CHAT_CONTEXT_BYTES` is 512 KiB (`desktop-app/src/chat.rs:68`). So a `skill` tool takes a name and returns the body; a `skill_resource` tool takes a name and a skill-relative path.

Both are read-only over hash-verified bytes that never touch the workspace, so both are `Allow` by default — but they must be **new tool names in `default_rules()`** (`permission.rs:227-248`), not folded into `read`, because `read`'s subject is a workspace path and conflating the two would make the permission subject ambiguous.

### N3 — The virtual `_bmad/` overlay lives in `desktop-app`, at the `run_tool` seam.

Not in `desktop-workspace`. Three reasons: the broker's value is that it is a dumb, structurally-safe filesystem gate and a synthetic namespace inside it means `read_text` sometimes does not read text; the corpus lives in `BmadLoadedFoundation`, which `desktop-app` owns (`lib.rs:47-57`); and the seam already exists — `chat_tools::run_tool` (`chat_tools.rs:54-64`) is the one place a tool name plus subject becomes an effect, and it already consults the permission gate first.

Mechanism: `run_tool` normalises the subject using the broker's own `normalize_relative_path` (`desktop-workspace/src/lib.rs:1271-1297`, to be exported), then routes on the first segment. `_bmad` → overlay. Anything else → broker, unchanged.

**The subject discipline from P15's contract section survives the reversal and applies here verbatim:** the normalised string that the overlay resolves must be the *same string* that `glob_match` matched, that `PermissionRequest.subject` renders, and that `always_pattern` derives from. Resolving one path while displaying another is approval laundering, and it is easier to get wrong with an overlay than with a sidecar, because the overlay's real backing store is app-local and its path is not the user's.

**Precedence when the workspace really has `_bmad/`.** Recommendation: **the real directory wins for any path that exists on disk; the overlay fills the rest.** A user who ran upstream's installer has a configuration Forge must not silently shadow, and the failure mode of the opposite choice is that Forge ignores the user's own customisation with no signal at all.

I am **not confident** this is right. The opposing case is strong: overlay-always-wins means behaviour is identical in every workspace and there is exactly one thing to test, which is what D6a's "collapses B1, B3, B4, B5 and B6 into one mechanism" (`P15:346-350`) implies. This is a decision the owner should make explicitly rather than inherit from whichever gets implemented first.

**`_bmad/custom/**` is written, and that collides with D6.** `bmad-customize/SKILL.md:83-97` writes the override file ("Write. Create `{project-root}/_bmad/custom/` if needed") and `:97` tells the user to **commit** team overrides. D6's cleanliness property is "nothing written to the user's repository except work products" (`P15:973-975`). Those cannot both hold. Options: (a) write to the workspace through governed IO, accepting `_bmad/custom/` in the diff; (b) write app-local per workspace, keeping D6 but making "commit your team override" a lie; (c) ask, once, per override.

Recommendation: **(a)**, and amend D6 to "nothing written except work products **and customisations the user explicitly authored**". D6's property was aimed at Forge installing a tree behind the user's back, not at a file the user asked for by name and reviewed before it was written. Note that `.user.toml` is gitignored upstream and `bounded_walker` honours gitignore (`lib.rs:1444-1452`, `git_ignore(true)`), so the overlay must read those by path, never by walk.

### N4 — `config.yaml` is synthesised from the vendored `module.yaml` defaults, plus a small settings surface.

Emit `_bmad/core/config.yaml` and `_bmad/bmm/config.yaml` from the two vendored `module.yaml` files. Every key has a declared default, so a zero-input synthesis is correct and a workspace that has never seen Forge works immediately. `{project_name}` defaults to the workspace directory name; `{date}` is host-supplied at run time (`bmm-skills/4-implementation/bmad-create-story/SKILL.md:59`: "`date` as system-generated current datetime").

The non-default values are exactly what upstream's installer questionnaire collects. Forge has no installer step, so **the questions still have to be asked, just not by an installer** — a small settings pane, defaults pre-filled. This is where D6's "one installer" claim gets paid for, and P15 does not budget for it.

Two format traps. First, the resolved values already carry `{project-root}` (`result: "{project-root}/{value}"` in both `module.yaml`s), and the corpus warns "never double-prefix" (`bmad-architecture/SKILL.md:47`). Second, the skills read this as YAML, so Forge must *emit* YAML; and if `config.user.yaml` is user-authored it must also *read* YAML. Recommendation: emit and read a deliberately flat scalar map and refuse anything else. **Uncertain** — a real YAML parser may be less total work than a bespoke one that has to be right about quoting and Unicode.

### N5 — `bmad-help.csv` is the concatenation of the two vendored `module-help.csv` files.

One header, then `core-skills/module-help.csv` rows, then `bmm-skills/module-help.csv` rows. Both are hash-pinned. This closes P15's B4 (`P15:322`) with no new data and no guessing.

### N6 — The `uv`/Python decision: **reimplement natively, and scope out what cannot be.**

**Option 1 — bundle Python and `uv`.** Rejected, and the usual objection (installer size) is not the real one. The scripts **do not exist in this repository**; bundling a runtime for absent scripts ships an interpreter that cannot run anything. Making this option real means additionally adopting upstream's `src/scripts/` (P15 B2 says 16 files — I could not verify that count, nothing here to check it against) *and* the six `{skill-root}` scripts P15 never mentions, into `runtime-manifest.json` and the semantic ledger, with a reseal. Worse, `check-boundaries.mjs:89-92` permits exactly two spawn sites and `desktop-tools/src/shell.rs` is one of them — so every `uv run` becomes a `shell` permission prompt showing a literal command line, several times per skill *activation*. That is the most dangerous prompt in the system, fired routinely, which is precisely how a user is trained to click through it.

**Option 3 — degrade.** Not available wholesale, but **partially sanctioned by the corpus itself**, and this is the most useful thing in this section. Every `resolve_customization.py` call site is written with a fallback: "On failure, read `{skill-root}/customize.toml` directly and use defaults" (`bmad-prd/SKILL.md:18`, `bmad-architecture/SKILL.md:53`, `bmad-spec/SKILL.md:21`, `bmad-forge-idea/SKILL.md:25`, `bmad-ux/SKILL.md:33`, `bmad-party-mode/SKILL.md:19`, `bmad-brainstorming/SKILL.md:21`). So *upstream already specifies the degraded path*, and it is the correct interim behaviour before N-E lands. `memlog.py` is **not** uniformly forgiving: `bmad-forge-idea/SKILL.md:58` and `party-memory.md:45` say carry on, but `bmad-prd` and `bmad-architecture` treat the memlog as the run's canonical memory with no stated fallback.

**Option 2 — reimplement natively. Recommended.** Feasibility, assessed by reading what the corpus asks each script to do:

| Script | Contract, as the corpus states it | Feasible? |
|---|---|---|
| `resolve_customization.py --skill <path> --key <agent\|workflow>` | Merge base `{skill-root}/customize.toml` → `_bmad/custom/{skill-name}.toml` → `.user.toml`; scalars override, arrays append, arrays-of-tables keyed by `code`/`id` replace-or-append; emit the merged table | **Yes.** Rules are in-corpus (`bmad-customize/SKILL.md:66-71`); all 37 base files are vendored and in memory. Needs a `toml` dependency — MIT/Apache-2.0, passes `deny.toml`'s allow-list. |
| `resolve_config.py --project-root <p>` | Merge `_bmad/config.toml`, `_bmad/config.user.toml`, `_bmad/custom/config.toml`, `_bmad/custom/config.user.toml` in that order, emit merged JSON (`bmad-help/SKILL.md:26`) | **Yes**, and largely moot — Forge synthesises the config layer (N4), so this reads state Forge already owns. |
| `memlog.py init\|append\|set` | `init --workspace W [--field k=v]…` creates `W/.memlog.md`, **errors if it already exists** (`party-memory.md:43`); `append --type T --text "…" [--by X]` appends one line atomically; `set --key K --value V` flips frontmatter | **Yes, with one honest gap.** The CLI surface is fully described (`bmad-brainstorming/SKILL.md:39-41`, `bmad-architecture/SKILL.md:39-40`, `party-memory.md:37-45`). The **rendered line format is not** — the only clue is that `--by user` "renders `(idea by user)`" (`bmad-brainstorming/SKILL.md:40`). Forge must choose a format. The only consumer is the model re-reading its own log, and Forge already has the semantic model (`crates/desktop-app/src/bmad_memlog.rs`, 1026 lines, P14 — though it stores entries in SQLite, not as `.memlog.md`, so the file-shaped writer is new). |
| `list_customizable_skills.py` | scan roots for `customize.toml`, report `agents`/`workflows`, `has_team_override`, `has_user_override`, `scanned_roots`, `errors[]` (`bmad-customize/SKILL.md:32-41`) | **Yes, trivially** — N1's index already holds it. |
| `resolve_party.py` | resolve roster from `module.yaml`'s `agents:` + customization; return `party_mode`, `memory_enabled`, scene/`open_cast`, `installed_agents_resolved` (`bmad-party-mode/SKILL.md:22`) | **Yes**, moderate. All inputs vendored. |
| `resolve_personas.py` | same shape, for `bmad-forge-idea` (`SKILL.md:85`) | **Yes**, moderate. |
| `brain.py` | `list --all\|--category`, `show "<name>"` over `assets/brain-methods.csv` (vendored) | **Partly.** list/show are trivial. `html --out` regenerates `assets/brain-selector.html`, **which is not vendored**, so that subcommand has no source to reproduce. |
| `lint_spine.py` | mechanical lint of `ARCHITECTURE-SPINE.md`: placeholders, duplicate `AD` IDs, missing Binds/Prevents/Rule, unpinned Stack versions (`references/reviewer-gate.md:5`) | **Yes**, moderate. The spine format is fixed by the vendored `assets/spine-template.md`. |
| `render.py` | prints the instruction that *is* `bmad-quick-dev` (`SKILL.md:9`) | **No.** Not vendored, behaviour undescribed. |

**Recommendation:** reimplement the three core scripts and four of the six skill-local ones. Declare **`bmad-quick-dev`** and **brainstorming's composer page** out of scope until `render.py` and `brain-selector.html` are adopted as a separate decision with its own licence surface and reseal.

**Say plainly what this costs.** The resolvers become *Forge's* behaviour, and drift from upstream's real scripts is **undetectable**, because those scripts are not present to diff against. No test in this repository can close that gap. State it in the definition of done rather than claiming conformance we cannot demonstrate. The mitigation available today is narrow and worth taking: pin the *observable* contract — the merge rules, the CLI surface, the fallback behaviour — in a Rust conformance suite that quotes its source line from the corpus, so a corpus change that moves the contract fails a test instead of silently changing a skill.

**`method-fidelity.test.mjs:117-157` must be updated when this lands** — its final assertion (no `.py` vendored) is the marker for this unresolved decision, and resolving it in Rust rather than by dropping files in means the test's *comment* becomes wrong while its assertion stays green. Amend the comment; keep the assertion.

### N7 — The tool-call shape must widen, and the widening must not weaken the closed-key check.

Today a tool call is `{tool, subject}` with `subject ≤ 1024` bytes (`desktop-runtime/src/chat.rs:149-182`), and `parse_chat_reply` permits exactly four keys (`desktop-app/src/chat.rs:935-942`). That is enough for `read`/`list`/`glob`/`grep`. It is not enough for `write` (content), `edit` (old/new), `shell` (cwd, timeout), or `memlog append` (`--type`, `--text`, `--by`).

Recommendation: `{schemaVersion, tool, arguments}`, where `arguments` is validated per tool against a closed, `deny_unknown_fields` struct — one per tool. That is the `desktop-ipc` envelope discipline P15 named for the sidecar contract (`crates/desktop-ipc/src/envelope.rs:130`, `:72`), and that part of P15 was never about the sidecar; it survives the reversal unchanged. `apply` and `changes` stay refused, preserving what `replies_outside_the_closed_shape_fail_closed` (`chat.rs:1141`) exists to protect. Note `chat.rs:1198` already carries the successor test, `the_tool_schema_widens_the_shape_without_widening_authority`.

### N8 — Placeholders are bound for the model, not substituted into the skill body.

The set: `{project-root}`, `{skill-root}`, `{skill-name}`, `{workflow.*}`, `{doc_workspace}`, plus config-derived `{user_name}`, `{communication_language}`, `{document_output_language}`, `{planning_artifacts}`, `{implementation_artifacts}`, `{project_knowledge}`, `{output_folder}`, `{project_name}`, `{user_skill_level}`, `{date}`. Rules at `bmad-architecture/SKILL.md:43-47`.

Two options: the host rewrites the body before it enters the transcript, or the host supplies a bindings block and the model resolves as it reads.

Recommendation: **bindings block, no rewriting.** Rewriting changes the bytes and breaks "the skill is carried unchanged", which is the entire point of P16. The safety objection to not rewriting does not hold, because every tool subject is re-normalised and re-permissioned host-side regardless — a mis-resolved placeholder produces a refused tool call, not an escape.

`{skill-root}` is the exception and must **never** be given a value the model can place in a path, because no such directory exists (N1). Skill-relative reads go through `skill_resource(name, relative_path)` instead.

### N9 — What is out of scope, stated now rather than discovered in Phase G

- **The Builder.** P15's B7 stands unchanged: `bmad-builder` is a separate upstream project (code `bmb`), and ADR-0006 keeps its output inert until it is adopted (`ADR-0010:135-138`).
- **`bmad-quick-dev`** — `render.py` is the skill and is not vendored.
- **Brainstorming's composer page** — `assets/brain-selector.html` is not vendored; the in-chat path (`references/in-chat-techniques.md`) still works.
- **`brain.py html`** subcommand, for the same reason.
- **Subagents.** 10 of 47 `SKILL.md` and 55 of 228 files reference subagents; `bmad-architecture`'s reviewer gate is built on parallel reviewers. A native subagent primitive — a nested, budgeted turn — is real work and is not in these phases. Until it exists, those skills run their reviews serially in the parent turn, which changes the run's shape and should be recorded as a known divergence rather than hidden.
- **Web retrieval.** 8 of 47 `SKILL.md` call for web research. `webfetch` is in the ADR-0009 grant set but has no implementation and no egress accounting story; it is not in these phases.

---

## 5. Phases

Independently landable and independently verifiable. **S** ≈ days, **M** ≈ 1–2 weeks, **L** ≈ a month or more. Use `verify:deferred-full` throughout — `verify:source` does not run `rust:test` or `rust:clippy` (`package.json`).

### Phase A — Make the shipped runtime honest — S

Two defects P15 does not name. Everything after this is untestable without them.

**A1. The tool loop is unreachable in production.** `seal_chat_manifest` hardcodes `CHAT_TURN_SCHEMA` (v1) as the sealed `canonical_output_schema_id` (`crates/desktop-app/src/chat.rs:802`, `:811`), unconditionally. The provider picks the model instructions off that field (`crates/desktop-cloud/src/direct_provider.rs:563`, `:605-618`), so a live turn receives the **v1** prompt — which ends "You cannot read, write, or execute anything" — while `parse_chat_reply` happily accepts v2 (`chat.rs:929-942`). `CHAT_TOOL_TURN_SCHEMA` is defined (`desktop-runtime/src/chat.rs:26`) and referenced only by the parser and the provider's match arm; **no code path ever seals it.** The loop exists and the model is never told it exists.
Fix: thread the output schema through `BeginChatTurn` so a tool-granted turn seals v2.

**A2. The packaged app cannot start.** `tauri.conf.json:39-47` bundles seven resource entries; `source/` is not among them. `read_manifest_resources` (`bmad_foundation.rs:657-684`) iterates **every** entry in `runtime-manifest.json` — 304 of them, 228 under `source/method/6.10.0/src/` — and `read_bounded_regular` (`:913-941`) errors on a missing file. `load_bmad_foundation` runs in `setup` (`crates/desktop-app/src/lib.rs:47-57`) and a failure is `StartupError`. P15 frames this as "shipping the Method means adding `source/`" (`P15:263-266`); it is stronger than that — **the installer produces an app that fails to launch**, and no test covers it because every test resolves the foundation from `CARGO_MANIFEST_DIR` (e.g. `bmad_foundation.rs:955-956`).
Fix: add `source/` to `bundle.resources`. ~1.32 MB, comfortably inside `MAX_TOTAL_BYTES` (`:18`).

*Verify:* a `tauri build` artifact launches and `load_bmad_foundation` returns `Ok` — assert it in a packaging smoke check, not only in a unit test rooted at the source tree. A unit test asserts that a tool-granted turn's sealed `canonical_output_schema_id` equals `CHAT_TOOL_TURN_SCHEMA`, and that a turn with no tools still seals v1. In the deterministic build, a turn emits `{"schemaVersion":"sapphirus.chat-turn.v2","tool":"glob","subject":"**/*.rs"}` and `commands.rs:1450` runs it.

### Phase B — Skill discovery over the sealed corpus — M

- `resource_paths()` accessor on `BmadLoadedFoundation` beside `resource_bytes` (`bmad_foundation.rs:305-310`).
- New module (recommend `crates/desktop-app/src/method/`, or a `desktop-method` crate if the capability lane is to be collapsed later — see O2 below): enumerate, parse frontmatter, index by `name`, refuse duplicates loudly.
- `skill` and `skill_resource` tools (N2), added to `default_rules()` as `Allow` (`permission.rs:227-248`), with skill-scoped prefix bounding on `skill_resource`.
- A Rust conformance suite mirroring `method-fidelity.test.mjs` — the JS test pins the *corpus*; this pins *the runtime's reading of it*, which is the half ADR-0010 says is now Forge's problem.
- The catalog (47 names + descriptions) enters the turn instructions; measure its byte cost against `MAX_CHAT_CONTEXT_BYTES`.

*Verify:* a Rust test enumerates exactly 47 skills, 14 core / 33 BMM, four phase folders, all names unique, every name equal to its directory basename, every one carrying non-empty `name` and `description`. `skill_resource("bmad-architecture", "references/reviewer-gate.md")` succeeds; `"../bmad-prd/SKILL.md"` and `"assets/../../.."` are refused. Tampering one byte of one `SKILL.md` on disk fails `load_bmad_foundation` rather than producing a wrong skill.

### Phase C — The permission dock, and the async dispatch it needs — M

This is the surviving substance of P15's Phase 3. `chat_tools.rs:33-36` and `:62-64` implement "ask" as **refuse**, which was correct while nothing could be asked and is a hard ceiling on every skill from here on: 40 of 47 expect to run something.

- `host_dispatch` is synchronous (`commands.rs:247`) and cannot wait on a human. It must become async first.
- `ContextDecisionEvidence` holds a `MutexGuard` over the whole consent ledger for its lifetime (`crates/desktop-egress/src/consent.rs:256`); rework or every lane serialises behind a prompt that is on screen.
- `PermissionRequest` → renderer → `PermissionReply`. Replace the composer while up; **never a modal** — a modal hides the transcript the decision depends on and trains dismissal.
- `title` and `detail` are tool-authored by construction (`permission.rs:141-155`). This is D4's one durable claim and it survives the reversal: the *model* may never supply prompt copy either. A model-supplied justification in a consent dialog is a social-engineering primitive whether it comes from a sidecar or from a completion.
- `GrantedRules` already exists and is already cleared on sign-out (`state.rs:100`, `permission.rs:214-218`). Wire `Always` to it and honour `can_generalize_subject` (`permission.rs:94-99`).
- Reconsider the 5-minute envelope age (`crates/desktop-ipc/src/envelope.rs:23`) and `AdmissionPolicy` 120 req/60 s (`crates/desktop-ipc/src/gate.rs:24-27`) — a prompt left over lunch currently yields an invalid resume.

*Verify:* deny everything and confirm the agent can do nothing at all; allow `read` only and confirm `write` still stops. A prompt open for ten minutes resolves correctly, or fails with a stated reason rather than an invalid-envelope error. `Always` on `shell` with a chaining subject is refused by `GrantedRules::grant` and the UI does not offer it.

### Phase D — The `_bmad/` overlay, read side — M

- Export `normalize_relative_path`; route `_bmad`-prefixed subjects in `run_tool`.
- Synthesise `_bmad/core/config.yaml`, `_bmad/bmm/config.yaml` (N4) and `_bmad/_config/bmad-help.csv` (N5).
- Read `_bmad/custom/**` and `_bmad/config*.toml` from wherever N3's precedence rule says.
- Settings pane for the non-default config values.
- The overlay must answer `read`, `list`, and `glob` — `bmad-customize` discovers by scanning and `bmad-forge-idea/SKILL.md:29` globs for `.memlog.md`.

*Verify:* open a workspace that has never seen Forge; a skill reading `{project-root}/_bmad/bmm/config.yaml` gets a well-formed answer, and **no such file exists on disk in that workspace afterwards**. `glob("_bmad/**")` returns the synthesised set and nothing from outside it. A workspace that *does* contain `_bmad/bmm/config.yaml` returns the user's file, and the permission subject shown is the workspace-relative path in both cases.

### Phase E — Native resolvers, and the `_bmad/custom` write path — M

- `resolve_customization` (merge rules per N6), `resolve_config`, `memlog init|append|set` writing a real `.memlog.md` under `{doc_workspace}`.
- `list_customizable_skills`, `resolve_party`, `resolve_personas`, `lint_spine`, `brain list|show`.
- Surface them as native tools, **not** as `shell`. `check-boundaries.mjs:89-92` stays at two entries.
- `_bmad/custom/**` writes through governed IO per N3(a).
- Until this lands, the corpus's own fallback ("read `{skill-root}/customize.toml` directly and use defaults") is the correct Phase-B/D behaviour.

*Verify:* a golden-file suite over the merge rules quoting its source line from the corpus — scalars override, arrays append, a `code`-keyed table entry replaces its match and appends the rest. `memlog init` on an existing workspace errors (`party-memory.md:43`); `append` is atomic under concurrent turns; `set --key status --value complete` changes only frontmatter. Authoring an override through `bmad-customize` end to end produces a file the user can see and commit, and the resolver's next call reflects it.

### Phase F — Writes, on the existing governed stack — M

D5 (`P15:207-217`) survives the reversal intact. Reuse everything below the approval seam: dual compare-and-swap on content *and* file identity (`governed_io.rs:837-840`), checkpoint-before-effect, the journal, rollback, crash recovery. Do **not** reuse `displayed_diff_hash`. Add the sibling approval whose seal binds the rule that authorised it.

Widen the tool shape per N7 first — `write` cannot be expressed as `{tool, subject}`.

*Verify:* an agent write is checkpointed, journaled and undoable; a stale preimage refuses; `.env` refuses structurally at the broker even with an `Always` grant on `write`; the approval seal names the permission rule that allowed it.

### Phase G — Fidelity, one skill at a time — L

For each adopted skill: stop sending `runtime/method/6.10.0/*.instructions.md` and send the verbatim `SKILL.md` with its bindings block. Delete that skill's row from the projection lane.

- `ADOPTED_INSTRUCTION_BOUNDARY` (`bmad_capability_host.rs:470-482`) becomes false the moment the first skill lands. It must be **generated per run from the granted tool set**, and eventually removed.
- Order, grounded in upstream's own definitions: `bmad-code-review` first (read-only, and it forces the parallel-reviewer question early — see N9's subagent gap), then `bmad-generate-project-context` (it *writes* `project-context.md`, which `bmad-architecture` holds as `persistent_facts` and calls load-bearing for brownfield, `bmm-skills/3-solutioning/bmad-architecture/SKILL.md:53` and `customize.toml`'s `persistent_facts` default), then `bmad-architecture`, then `bmad-dev-story`, with `bmad-prd`/`bmad-product-brief` on a parallel track.
- Every corpus change is a **reseal plus a Rust edit**: `EXPECTED_MANIFEST_HASH` (`bmad_foundation.rs:21`), `EXPECTED_SEMANTIC_LEDGER_HASH` (`:19`) and `EXPECTED_RESOURCE_PATHS: [&str; 76]` (`:67`) are compiled-in constants. Budget for it; it will bite on the first adoption.
- Re-derive the "adapt-for-withheld-authority → 0" metric before using it as a target (P15's keyword method is approximate and it says so, `P15:723-726`).

*Verify, per skill:* it runs end to end in a workspace that has never seen Forge, with nothing written to that repository except work products and user-authored overrides. Its `treatment` in the semantic ledger moves from `adapt` to `adopt`, and `normalized/<skill>.package.json` is deleted rather than left stale. The fidelity matrix's "Faithful upstream execution: still 0" moves by exactly one.

---

## 6. What I could not verify

- **I did not compile.** The `--all-features`/`E0425` risk (`P15:945`) looks closed by inspection but I did not run `cargo check`.
- **P15's B2 claim that upstream ships 16 `.py` under `src/scripts/`** — nothing in this repository can confirm it. `library/opencode` is a checkout of OpenCode, not BMAD-METHOD.
- **The exact `.memlog.md` on-disk format.** Under-specified in the corpus (N6). Any native writer is a choice, not a reproduction.
- **`render.py`'s behaviour.** Unknown, and `bmad-quick-dev` is unimplementable without it.
- **Installer size.** Still unmeasured; `source/` adds ~1.32 MB, which is small, but the ~211 MB figure P15 flags as sourceless (`P15:942`) remains sourceless.
- **Whether real-`_bmad`-wins is the right precedence rule** (N3). Genuinely uncertain; recommend the owner decide rather than the implementation.
- **Whether a bespoke YAML subset beats a YAML dependency** (N1, N4). Uncertain.

## 7. Definition of done

- All 47 vendored skills are discovered by name from the sealed corpus, with a Rust conformance suite that fails when the corpus shape moves.
- The `_bmad/` overlay satisfies the 39 skills that read `{project-root}/_bmad/**` and the 36 that read `config.yaml`, with nothing written to the user's repository except work products and customisations the user explicitly authored.
- The three core scripts and four of the six skill-local scripts are native; **`bmad-quick-dev`, `brain.py html` and the brainstorming composer page are stated as out of scope**, not discovered as broken.
- Every effectful tool call is evaluated by Rust and, when unmatched, reaches a human before it runs — with a dock, not a refusal.
- The subagent gap and the web-retrieval gap are recorded as known divergences with named skills, not left to be found in a run.
- No `externalBin`, no third spawn-allowlist entry. Both CI gates untouched.
- `verify:deferred-full` green.

---

### Critical files for implementation

- `C:\tmp\forge-v2\crates\desktop-app\src\chat_tools.rs`
- `C:\tmp\forge-v2\crates\desktop-app\src\bmad_foundation.rs`
- `C:\tmp\forge-v2\crates\desktop-app\src\chat.rs`
- `C:\tmp\forge-v2\crates\desktop-workspace\src\lib.rs`
- `C:\tmp\forge-v2\packages\bmad-foundation\source\method\6.10.0\src\` (the 228-file corpus; `core-skills/module.yaml`, `bmm-skills/module.yaml` and the two `module-help.csv` are the synthesis inputs)
