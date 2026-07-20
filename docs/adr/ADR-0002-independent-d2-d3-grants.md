# ADR-0002: Independent D2 context-read and D3 governed-edit grant epochs

- Status: accepted (2026-07-20)
- Decides: the P0 open decision "Representation of independent D2
  (context-read) vs D3 (governed-edit) grants" (implementation packet P4).

## Context

A workspace grant carries one `grant_epoch` and one `permissions` level
(`ReadOnly` → `GovernedEdits`). Enabling governed edits bumps that single
epoch, which also invalidates every D2 Help binding (prepared manifests,
live decisions) even though nothing about context-read authority changed.
Symmetrically, there is no way to withdraw D2 context-read authority (for
example on model sign-out) without destroying D3 proposals.

D2 model-access authority and D3 file-effect authority are required to stay
disjoint (locked constraint since D2-D). The remaining gap is that their
*lifetimes* are coupled through the shared epoch.

## Decision

One workspace grant, three monotonically increasing epochs:

| Epoch | Authority it versions | Bumped by |
| --- | --- | --- |
| `grant_epoch` (existing) | whole-workspace binding: selection, restore, root identity, revocation | grant / restore / revoke |
| `context_read_epoch` (new) | D2 Help context reads and consent bindings | `advance_context_read_epoch` (driven by `model.auth.sign_out`) |
| `governed_edit_epoch` (new) | D3 proposals, specs, and governed mutations | `enable_governed_edits` |

- `enable_governed_edits` no longer bumps `grant_epoch`; it bumps only
  `governed_edit_epoch`. Escalating (or later changing) edit authority
  leaves in-flight D2 Help state valid, and D1 reads untouched.
- D2 Help lifecycle captures `context_read_epoch` at `prepare` and
  revalidates it (in addition to the binding epoch) at approve, cancel, and
  submit. `model.auth.sign_out` advances every workspace's context-read
  epoch: sign-out withdraws model/context authority without touching local
  work or D3 proposals.
- D3 authority captures `governed_edit_epoch` when edit authority is
  created and revalidates the exact value at every governed mutation.
- `revoke` (whole grant) still invalidates everything: binding epoch
  authority is a precondition for both verticals.
- The renderer contract is unchanged: `workspaceGrantEpoch` remains the
  binding epoch in every pinned command payload. The per-vertical epochs
  are host-internal authority; they never cross IPC. Persisted grant
  projections deserialize with `serde` defaults (both new epochs default to
  1), so existing stored grants remain valid.

## Consequences

- Cross-independence is testable and pinned: bumping one vertical's epoch
  must leave the other vertical's in-flight bindings valid; bumping the
  binding epoch must invalidate both.
- A D2 decision remains structurally unusable in D3 and vice versa
  (pre-existing invariant); the epochs now make the *temporal* coupling
  explicit and independent as well.
- Renderer UX for per-vertical grant display (if wanted later) requires a
  deliberate additive catalog change; nothing in P4 forces it.
