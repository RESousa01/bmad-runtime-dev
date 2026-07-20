# ADR-0004: Uninstall and offboarding data retention

- Status: accepted (2026-07-20)
- Decides: the P0 open decision "Uninstall/offboarding data retention"
  (implementation split P1/P6; this ADR governs the P6 application side).

## Context

The desktop keeps all app-owned durable state under one authority root
(`%LOCALAPPDATA%/…/authority-v1`): a DPAPI-wrapped store key, the
authority database (identity, persisted workspace grants, Method
sessions, execution journals/checkpoints, BMAD help runs, evidence
streams), and content-addressed payloads. Work product — files created
or edited through governed D3 changes — lives in the user's own
workspace folders, never inside the authority root.

## Decision

1. **Uninstall retains application data.** The installer removes program
   binaries and uninstall registration (qualified by the P1 lifecycle
   lane) but never deletes the authority root. Standard Windows posture:
   uninstalling must not destroy user history, and reinstalling restores
   access to it.
2. **Work product is never the app's to delete.** No uninstall or
   offboarding flow touches workspace folders. Erasure covers only
   app-internal state under the authority root.
3. **Offboarding is an explicit in-app flow with two commands:**
   - `app.offboarding.inspect` — a retention manifest of what the
     authority root holds: bounded category counts and byte totals only,
     never paths or content.
   - `app.offboarding.erase` — cryptographic erasure: the payload takes
     the exact confirmation phrase `erase-local-authority-data`; the host
     signs out model authority (withdrawing D2 context-read epochs,
     ADR-0002), revokes every workspace grant, deletes all authority
     rows and content-addressed payloads, and deletes the DPAPI-wrapped
     store key — after which any residual bytes are undecryptable. The
     session drops to read-only recovery; the next launch starts a fresh
     identity.
4. **Cloud-side retention is out of scope here.** Sign-out already
   withdraws local model/context authority; support-plane data lifetimes
   are governed by the D2-E service policies and its operator runbook.

## Consequences

- Erase is irreversible by design and must be double-gated (typed
  confirmation phrase in the renderer, exact-phrase validation in the
  IPC payload, request tracking as a mutating command).
- The P1 clean-machine lane can now assert the decided posture: after
  uninstall the authority root remains; after in-app erase the store key
  is gone and remaining bytes are ciphertext.
- The retention manifest gives users an honest answer to "what do you
  keep about me?" without exposing paths or content over IPC.
