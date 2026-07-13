# Sapphirus Desktop

Sapphirus is an internal Windows desktop workspace for governed AI-assisted development. Its
locked architecture assigns local state, workspace access, approvals, checkpoints, file effects,
rollback, and evidence to `desktop-app`, the sole Rust composition root. That host is intended to
be Authenticode-signed for organization distribution; signing evidence remains deferred. The
React/WebView2 renderer is a typed presentation client and never receives generic filesystem,
process, token, database, or updater authority.

The current D1 source slice is read-only: typed native folder-selection IPC; opaque local-workspace
listing, switching, and access revocation; bounded tree/read/search projections; BMAD inspection;
and exact context review. The Rust host has not yet been compiled or exercised on Windows under
the paused native-toolchain lane. Agent submission, sign-in, model access, proposed file effects,
`Apply changes`, checkpoints, undo, installer validation, and command execution remain
unintegrated. The planned edits-capable internal release remains **edits first**.

## Repository map

- `apps/desktop-ui` — Agent + Sessions renderer
- `crates/desktop-*` — native authority and adapters
- `packages/contracts` — JSON Schema 2020-12 source, fixtures, and generated boundaries
- `packages/ui` — accessible product design system
- `helpers/windows-auth-broker` — frozen, non-integrated WAM broker scaffold (D2)
- `services/desktop-support-api` — frozen, non-integrated Azure support-plane scaffold (D2)

The optional `bmad-runtime-lib` folder is external, untracked implementation context. It is not a
workspace package, build input, CI dependency, application resource, or distribution artifact.

## Local verification

```powershell
pnpm install --frozen-lockfile
pnpm verify:source
```

`verify:source` (and the default `verify`) covers deterministic TypeScript 7.0.2 contract generation
and conformance, sealed BMAD fixtures, renderer/UI typechecks and tests, the first-party secret
scan, Node-based boundary inspection, and the Vite web-asset build. It does not require the
external context library. The broader cross-language and native checks are enabled only when their
installed toolchains are available; manual native CI jobs additionally require the
organization-controlled `SAPPHIRUS_NATIVE_LANE_ENABLED` gate.

The planned internal deployment is single-tenant and uses organization-managed identity, policy,
and signed packages. No Docker, local server, local model, or GPU is required on employee
workstations.
