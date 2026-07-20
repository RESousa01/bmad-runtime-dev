# D2-E production rollout runbook (Task 12 — operator execution pending)

This runbook stages the plan's Task 12 rollout. Every stage is an operator
action against real Azure resources with human approval; none of it runs
from local development gates. Code-side prerequisites from Tasks 1–11 are
complete and referenced below.

## Stages

1. **Infrastructure first** — deploy `infra/desktop-support/main.bicep` with
   `deployApi=false`. Before deploying: inspect subscription Azure policy
   assignments, then `az deployment group validate` and `what-if`; the
   what-if must show no local-auth enablement, public data-plane endpoint,
   plaintext secret, mutable image tag, overly broad role, or
   caller-controlled model endpoint.
2. **SQL authority** — apply `services/desktop-support-api/Sql/Migrations/`
   through the emitted `sqlMigrationIdentity*` identity, then apply the
   least-privilege data-plane grants from
   `infra/desktop-support/sql-grants.md`. Never grant the runtime identity
   schema rights.
3. **Image publication** — publish the signed, SBOM-attested API image by
   immutable digest (the `support-container` CI job proves buildability and
   scanning; the release lane signs and pushes).
4. **First revision, model disabled** — set `deployApi=true` with the digest
   and an App Configuration policy whose `approvedModelDeployments` is
   empty; the fixed-profile broker then refuses model access at profile
   resolution.
5. **Gate verification** — run
   `tools/support-smoke/deployed-smoke.ps1` (identity, signed policy,
   fail-closed lease), `tools/support-smoke/privacy-sql-inspect.sql`, and
   the telemetry/health checks; alerts from
   `infra/desktop-support/modules/monitor-alerts.bicep` must be active,
   including the sev-0 privacy canary rule.
6. **Test-tenant model access** — add the fixed deployment to
   `approvedModelDeployments` for the dedicated test group with a fixed
   budget (`modules/budget.bicep`).
7. **End-to-end + failure injection** — Entra-authenticated test
   installation → registration → lease → signed consent → one no-store
   model call → signed receipt → Rust verification
   (`crates/desktop-cloud/tests/production_lifecycle.rs` is the local
   analog) → local Method completion. Recovery drills: revocation race, SQL
   transient failure, Key Vault throttle, model timeout, Container Apps
   revision rollback, database point-in-time restore rehearsal, and signing
   key rotation (the `SigningKeyRing` verification overlap is the client
   contract).
8. **Promotion** — promote the same digest through approved environments;
   never rebuild per environment.
9. **Desktop enablement** — build the desktop with the `production-support`
   feature and the complete package-controlled value set
   (`SAPPHIRUS_SUPPORT_TENANT_ID`, `…_API_CLIENT_ID`, `…_SCOPE`,
   `…_ORIGIN`, `…_REGION`) plus pinned policy/receipt trust keys; activate
   the deployed round-trip in `ProductionHelpTransport` in the same change.
   Partial values leave the client in explicit offline/development
   behavior.
10. **Kill switch** — clearing `approvedModelDeployments` in App
    Configuration blocks new model admission within one policy refresh
    interval without affecting local history, inspect, export, rollback, or
    recovery.

## Rollback

- Route traffic back to the prior Container Apps revision.
- Disable model admission via the signed policy (`approvedModelDeployments`).
- Revoke the affected signing key version or model identity assignment only
  through the audited incident procedure; clients accept the documented
  active/verification overlap and nothing else.
- Never roll back a SQL migration destructively; use compatible forward
  repair (the migration runner refuses hash-divergent re-application).
- Desktop falls back to explicit offline behavior, never deterministic or
  unsigned cloud behavior.
