# ADR-0001: Windows-local authority and edits-first release

- Status: accepted
- Date: 2026-07-13
- Delivery model: `windows_local`

## Decision

The signed Rust host is the only local lifecycle, workspace, policy, checkpoint, file-effect, and
evidence authority. React/WebView2 is an untrusted presentation client. Azure is a support plane and
cannot address a local path, mint local execution authority, or apply a local result.

The planned edits-capable internal organization release supports only the application's journaled
UTF-8 patch engine. The current D1 slice is read-only and does not expose that engine. Neither slice
contains a callable shell, terminal, command runner, test runner, package script, hook, or
child-process primitive. A future command release requires a separate E3 containment ADR and
measured tool matrix; Job Objects alone do not satisfy that gate.

## Consequences

- The release target requires a closed, generated IPC catalog. The current D1 catalog is closed and
  source-checked, but its renderer/native shapes are still manually mirrored and require canonical
  schema generation before release.
- Workspace roots are selected by a host-owned dialog and represented to the renderer by opaque IDs.
- At D3, every proposed file effect must be bound to the current root, grant epoch, preimages,
  policy, and hash.
- At D3, `Apply changes` must record the exact decision; the UI never directly invokes an execution
  primitive.
- At D3, multi-file changes must be journaled and crash-recoverable with per-file atomic replacement
  where the filesystem supports it. The product will not claim global multi-file atomicity.
- Deferred capabilities are absent from the D1 IPC catalog, capability projection, and navigation.
  D2/D3 domain scaffolding is not callable, but it must be compile-pruned or otherwise proven absent
  before a release binary can satisfy the stronger binary-inventory gate.
- Entra and support-plane configuration is single-tenant; no public sign-up or multi-tenant customer
  boundary is part of the desktop release.
