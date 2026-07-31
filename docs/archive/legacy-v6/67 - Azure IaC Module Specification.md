---
title: "Azure IaC Module Specification"
aliases:
  - "67 - Azure IaC Module Specification"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 67
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: azure-iac-spec
status: v6-modernized-implementation-guide
validated_on: 2026-07-09
---



# Azure IaC Module Specification

## V6.17 infrastructure scope

The web module provisions the existing control/workspace/model/execution/evidence topology. The desktop support module provisions Entra app/API registrations, entitlement/licensing API, Model Access API with managed identity, package/update distribution metadata, optional sync/collaboration stores, privacy-safe telemetry intake, and optional remote-job services.

The desktop module has no local-file mount, remote desktop/control channel, local lifecycle database, or ordinary edit/test executor. Network/RBAC/storage policies isolate support-plane purposes and explicit-upload containers. Remote jobs reuse the fixed web execution boundary but create separate `web_managed` work records and cannot address a device path.

## 1. IaC position

Bicep is the v1 Azure IaC source of truth. Azure Verified Modules should be used where they simplify common resources without hiding required security, identity, networking, or logging controls. Azure Developer CLI may orchestrate dev/test workflows but cannot bypass reviewed Bicep. Local Aspire/Docker/Kubernetes orchestration is not a baseline requirement; the documented no-container workflow must remain complete.

## 2. Resource groups / environments

Use separate environments:

| Environment | Purpose | Characteristics |
|---|---|---|
| `dev` | cheap cloud integration and first real provider/job lane | minimum scaling, short retention, budget/teardown controls, fake model option, remote ACR build, fixed disabled-by-default Job template. |
| `test` | reproducible integration/e2e | production-like identity, deterministic fixtures. |
| `staging` | release-candidate verification | production-shaped policy/identity/network/retention with test data and promotion/rollback gates. |
| `prod` | internal production | managed identities, private endpoints where required, production retention/alerts. |

## 3. Bicep modules

| Module | Resources | Required outputs |
|---|---|---|
| `foundation.bicep` | resource group tags/policy, budget/alerts, deployment diagnostics | environment identity, budget/action-group ids. |
| `identity.bicep` | Entra/app config inputs and separate managed identities/role assignments for API, migration, ACR build, pull, job start, worker storage, Key Vault, monitoring | principal ids plus role-assignment evidence. |
| `web.bicep` | App Service plan, web app, auth settings, app config | web URL, identity principal ID. |
| `container-apps.bicep` | ACA environment, Runtime API container app, digest-pinned fixed job definitions, workload profiles | environment ID, API URL, immutable job template ids/names. |
| `sql.bicep` | Azure SQL server/db, firewall/private endpoint, identities | DB resource ID, connection secret ref/MI config. |
| `storage.bicep` | Blob account, containers, lifecycle policies, versioning/soft delete | account URI, container names. |
| `keyvault.bicep` | Key Vault, RBAC, secret references | vault URI. |
| `acr.bicep` | Azure Container Registry, retention/quarantine policy, ACR Task/remote-build identities, pull permissions | registry URI, task/build identity, evidence locations. |
| `model.bicep` | Foundry/Azure OpenAI account/deployment inputs where supported, network/diagnostic settings, quota references | endpoint/resource/deployment ids only; no secrets or floating model alias. |
| `monitor.bicep` | Log Analytics, App Insights, alerts, dashboards | workspace ID, app insights connection string ref. |
| `signalr.bicep` | SignalR service if selected; otherwise SSE config | endpoint and access mode. |
| `network.bicep` | VNet/private endpoints/NAT where required | subnet IDs, private DNS zones. |

## 4. Required app settings

- `Runtime__EnvironmentName`
- `Runtime__SchemaVersion`
- `Storage__AccountUri`
- `Sql__ConnectionStringSecretRef` or managed identity config
- `ModelGateway__ActiveRoleProfileMapRef`
- `Airlock__PolicyVersion`
- `Execution__FixedJobTemplateMapRef`
- `Trace__RawRetentionEnabled=false`
- `OpenTelemetry__ServiceName`
- `OpenTelemetry__Exporter=AzureMonitor`
- `Security__RequireApprovedExecutionSpecForGovernedDispatch=true`

## 5. Deployment gates

- Bicep build passes.
- IaC what-if is reviewed.
- Secrets are references, not literal values.
- Managed identities have least privilege.
- ACR Tasks/hosted CI remote build binds source/lock/component licenses/scan/SBOM/provenance/signature to an approved digest without local Docker.
- Fixed job template binds digest/entrypoint/identity/secrets/network/resource/output settings and denies request-time overrides; dispatcher can start but not mutate it.
- Database migration reviewed.
- App Service/Entra auth configured for internal access.
- Storage lifecycle policies exist for logs/manifests/snapshots/exports.
- Smoke tests pass in order: health/auth/fake run/evidence first, then Phase-4 fixed ACA result import/recovery; no local container/model/emulator prerequisite.
- Rollback plan exists for API image, worker image, and database migration.

## 6. Environment smoke sequence

```text
provision infra
→ submit remote image build and verify evidence/digest
→ deploy API/web and fixed disabled job template
→ run database migration
→ verify auth
→ create project/thread
→ run fake model plan
→ create proposal
→ deny raw execution dispatch
→ approve exact fake candidate and import simulated result (Phase 1 contract smoke)
→ enable/start the allowlisted fixed ACA template with a new single-use spec (Phase 4)
→ import WebWorkerResultManifest with completion/ledger/outbox recovery
→ render evidence bundle
```
