# Implementation packet: D0/D1 source hardening and read-only workbench

## Authority and intent

- Owning authority design: `desktop-app` is the sole Rust composition root and is intended to be
  Authenticode-signed for internal distribution; signing evidence remains deferred. The React
  renderer is an untrusted projection client.
- Source-level outcome: an internal Windows workbench implements the typed paths to bootstrap,
  display and switch opaque local workspaces, request the native folder picker, revoke workspace
  access, review read-only context, and enter an explicit recovery presentation without exposing a
  generic native primitive.
- Contracts read: `desktop-bootstrap.v1`, `desktop-ipc-command.v1`,
  `desktop-dispatch-reply.v1`, `desktop-projection-request.v1`, and
  `desktop-projection-reply.v1`.
- Non-goals: connected model access, sign-in, proposed-edit authority, file mutation, apply, undo,
  command execution, remote jobs, package activation, and public distribution.
- Stop conditions: any absolute path in renderer state, any non-D1 command accepted by the D1 IPC
  gate, any local effect after recovery or renderer-session revocation, or any store integrity
  failure reported as Ready.

## Tests first

- Success fixture: bound D1 read command, opaque workspace bootstrap/list/switch/revoke, native
  workspace selection, projection refresh, and the browser-only preview fallback.
- Negative/bypass fixture: duplicate JSON keys, renderer/install/window mismatch, later-phase and
  generic command names, `governed_edits` permission drift, path-shaped safe error text, unknown
  fields/discriminators, stale or discontinuous projection sequences, revoke identity/epoch drift,
  replay or non-advancing mutation sequences, concurrent projection events, in-flight
  traversal-versus-revoke races, and mismatched CAS evidence references.
- Failure/recovery fixture: store key loss/corruption, recovery capability downgrade, recovery or
  re-bootstrap during an in-flight Ready-only request, payload/evidence/consumption tampering, and
  duplicate/missing outbox linkage.
- Compatibility or migration fixture: forward-only store v3 to v4 index migration and rejection of
  unsupported future store versions. Native execution of these Rust tests remains pending the
  paused Windows toolchain lane.

## Change and rollback

- Files/surfaces allowed: `apps/desktop-ui`, `packages/ui`, `desktop-app`, `desktop-ipc`,
  `desktop-workspace`, `desktop-store`, repository boundary checks, and this packet.
- Disable or rollback path: enter `read_only_recovery`; project only `app.get_boot_state` and
  `workspace.list`; keep task submission, proposed effects, Apply changes, and all command surfaces
  unreachable.
- Observability/evidence: monotonic host projections, renderer-safe stable errors, the local
  hash-linked evidence/outbox transaction, and explicit browser-preview/recovery labels.

## Review ledger

- Implementer full-diff review: completed 2026-07-13 for naming, fail-closed behavior, error paths,
  generated drift, and unintended authority expansion.
- Independent source-level bug/security/accessibility review: completed for the renderer boundary,
  store invariants, and capability/recovery/session races. Reported blockers were corrected and
  re-reviewed; no native runtime claim is made.
- Commands executed after the final source change: two consecutive clean non-native verification
  sweeps, each covering 105 reference-vault Markdown files; deterministic TypeScript-only contract
  generation across 58 controlled files; the exact TypeScript 7.0.2 compiler; 26/26 contract
  conformance tests; 3 sealed BMAD descriptors and 64/64 fixture tests; 62/62 renderer
  Vitest/RTL/axe tests; TypeScript typechecks; the 198-file first-party secret scan; and the
  Node-based architecture-boundary audit. The 26-case contract suite runs at both the explicit
  contract gate and the recursive package-test gate, so suite counts—not inflated test events—are
  recorded here.
- Environment-blocked check: the final production Vite asset build could not be rerun because the
  installed helper process required an execution escalation that was unavailable in this session.
  An earlier pre-final-change build is not treated as current evidence.
- Checks deliberately deferred: every Rust/Cargo compile, format, metadata, Clippy, test, and
  generated-binding conformance command; every C#/.NET/MSVC/Build Tools command; Tauri/native
  packaging; WAM/auth-broker and support-plane execution; WebdriverIO; clean-VM install/update;
  and crash injection remain frozen at the user's request. `verify:source` performs read-only Node
  inspection of Cargo manifests/lock data, Rust source, capabilities, and Tauri configuration, but
  invokes none of those toolchains and neither reads nor regenerates C# bindings.
- Remaining risks/gaps: the D1 read-only embedded Git view is not implemented; IPC/workspace/store
  behavior has no compiled native runtime evidence; selected-root handle/file-ID/reparse/case/TOCTOU
  proofs, Windows write-through CAS/key durability, generated Rust structural validation, and
  complete migration/crash suites remain open; rendered responsive/keyboard/forced-colors QA is
  unverified beyond RTL/axe; D2 and governed D3 are not composed.
