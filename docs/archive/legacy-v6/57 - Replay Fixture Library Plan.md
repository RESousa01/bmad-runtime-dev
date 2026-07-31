---
title: "Replay Fixture Library Plan"
aliases:
  - "57 - Replay Fixture Library Plan"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 57
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: replay-fixture-plan
status: implementation-guide
---



# Replay Fixture Library Plan

## V6.17 fixture partitions

Fixtures are grouped as `shared/`, `web/`, and `desktop/`. Shared fixtures cover canonical JSON/hashes, BMAD packages/workflows, Airlock decisions, events, errors, and schema migrations. Web fixtures cover SQL/Blob transactions, fixed templates, worker manifests, import/retry, and cloud rollback. Desktop fixtures cover IPC commands/events, folder capabilities, NTFS path/reparse/hardlink races, filesystem capability snapshots, SQLite/CAS crash boundaries, journal/rollback, command identity/drift, containment profiles, auth/egress, and updater/package verification.

Cross-delivery fixtures cover remote handoff hashes and prove returned results cannot apply directly. Every shared golden vector must deserialize/validate/canonicalize/hash identically in C#, Rust, and TypeScript.

## 1. Required fixture families

| Fixture | Purpose |
|---|---|
| `vertical-slice-simulated-success` | Proves chat→context→proposal→candidate→policy→exact approval→single-use spec→simulated result import→Evidence Ledger/Bundle with a non-isolating fake. |
| `aca-fixed-job-success` | Proves remote image/fixed template/spec/attempt/result/completion/outbox/import evidence for the first real isolated effect. |
| `preimage-drift-blocked` | Proves stale proposal cannot apply. |
| `policy-denied-command` | Proves Airlock blocks risky command. |
| `worker-result-invalid` | Proves Runtime rejects bad candidate/spec/audience/attempt/template/image/workspace/output/completion bindings. |
| `validation-failed-repair` | Proves repair loop and partial failure semantics. |
| `bmad-package-valid` | Proves BMAD package loading. |
| `bmad-package-invalid` | Proves validation errors block activation. |
| `presentation-adapter-golden` | Proves existing presentation workflow adapter preserves behavior. |
| `trace-redaction` | Proves secrets do not appear in evidence by default. |
| `approval-grant-scope` | Proves reusable approval cannot be abused outside scope. |
| `candidate-hash-drift` | Proves approval of candidate A cannot authorize candidate B or changed mutable inputs/policy. |
| `completion-import-crash` | Proves completion/outbox redelivery imports once and never reruns a consumed spec. |
| `event-cursor-gap-upcast` | Proves explicit cursor expiry/gap, pure upcast, projection checkpoint, and no side-effect replay. |
| `source-license-blocked` | Proves missing immutable provenance/unresolved license/restrictive component blocks promotion while research evidence remains. |
| `provider-lookalike-credential-denied` | Proves user-info/path/attacker-host strings containing `azure.com` never receive Azure credentials. |
| `model-profile-critical-eval-failed` | Proves aggregate quality cannot hide a failed safety/privacy lane and no fallback/profile activates. |
| `responses-retention-tools` | Proves baseline calls use `store=false`, hosted tools off, app-owned state, and typed refusal/incomplete handling. |

## 2. Fixture structure

```text
replay/fixtures/<fixture-name>/
  input/
    project.zip
    user-message.json
    model-outputs.json
    approvals.json
  expected/
    evidence-ledger.jsonl
    projection-state.json
    evidence-bundle.json
    evidence-summary.md
    changed-files.json
    policy-decisions.json
    work-attempt-completion.json
  Start Here.md
```

## 3. Replay rules

- Fake model provider is used for deterministic replay.
- Timestamps and generated IDs are normalized before comparison.
- Hash fields are compared when input files are deterministic.
- Redacted outputs are compared; privileged raw payloads are not required for normal replay.
- Replay must fail closed on schema drift unless migration is explicitly supplied.
- Scenario replay never starts a process, network call, package script, dependency restore, or real worker; it exercises deterministic fakes and sealed fixtures only.
- A real ACA rehearsal is a new governed work attempt with a new candidate/policy/approval/spec, not “replay” of a historical effect.
- Projection/transport/forensic replay cannot invoke Model Gateway or Execution Dispatcher and must produce identical state/evidence hashes from the same retained ledger versions.
- Fake result evidence is labeled simulated/non-isolating and cannot satisfy the internal-alpha ACA gate.
