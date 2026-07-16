# Sapphirus Desktop

Sapphirus is an internal Windows desktop workspace for governed AI-assisted development. Its
locked architecture assigns local state, workspace access, approvals, checkpoints, file effects,
rollback, and evidence to `desktop-app`, the sole Rust composition root. That host is intended to
be Authenticode-signed for organization distribution; signing evidence remains deferred. The
React/WebView2 renderer is a typed presentation client and never receives generic filesystem,
process, token, database, or updater authority.

The current source slice covers D1 reads and the first D3 governed-edits vertical. Reads: typed
native folder-selection IPC; opaque local-workspace listing, switching, and access revocation;
bounded tree/read/search projections; BMAD inspection; and exact context review. Edits: explicit
per-workspace `GovernedEdits` enablement at a fresh grant epoch; host-observed proposed changes;
an exact review projection whose canonical hash the approval decision binds; durable single-use
spec consumption; checkpointed, journaled, atomic UTF-8 patch application with postimage
verification; governed `Undo changes` through a fresh reviewed rollback proposal; and boot-time
journal reconciliation that fails closed to manual review. The pinned Rust workspace is compiled
and tested on Windows; packaged, signed installer validation is still outstanding. Agent
submission, sign-in, model access, and command execution remain unintegrated; proposals currently
originate from the renderer's review flow, not from a model.

## Repository map

- `apps/desktop-ui` — Agent + Sessions renderer
- `crates/desktop-*` — native authority and adapters
- `packages/contracts` — JSON Schema 2020-12 source, fixtures, and generated boundaries
- `packages/ui` — accessible product design system
- `helpers/windows-auth-broker` — frozen, non-integrated WAM broker scaffold (D2)
- `services/desktop-support-api` — frozen, non-integrated Azure support-plane scaffold (D2)

The tracked `bmad-runtime-lib` folder is a reference-only context vault. It is not a workspace
package, build input, CI dependency, application resource, or distribution artifact, and its
imported third-party scripts are never executed by the product toolchain.

## Local verification

Tauri does not need a global installation. The exact CLI is a root development dependency and is
invoked by the root `pnpm` scripts; installing the pinned Rust toolchain is sufficient for Cargo.

```powershell
pnpm install --frozen-lockfile
pnpm tauri:version
pnpm verify:source
```

`verify:source` (and the default `verify`) covers the self-contained BMAD foundation gate,
deterministic TypeScript 7.0.2 contract generation and conformance, sealed BMAD fixtures,
renderer/UI typechecks and tests, the first-party secret scan, Node-based boundary inspection, and
the Vite web-asset build. It does not read or execute imported context-vault content. The optional
`vault:check` command is a development audit only. The contract generator-qualification job is an
unconditional Windows CI gate with pinned Rust and .NET tools. Desktop packaging remains a manual
native CI job guarded by the organization-controlled `SAPPHIRUS_NATIVE_LANE_ENABLED` setting.

The planned internal deployment is single-tenant and uses organization-managed identity, policy,
and signed packages. No Docker, local server, local model, or GPU is required on employee
workstations.
