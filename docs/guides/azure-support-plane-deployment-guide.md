# Azure support-plane deployment guide (operator, self-service)

This is the step-by-step operator guide for readiness-program **Task 10**:
deploying and qualifying the D2 support plane on Azure. It expands the
[rollout runbook](../superpowers/plans/2026-07-20-d2-e-rollout-runbook.md)
into concrete commands you run yourself. Nothing here runs from repository
gates; every stage is a human action against your subscription.

**What gets created** (all from `infra/desktop-support/main.bicep`, one
resource group): a VNet-integrated Container Apps environment, a private
Azure Container Registry, Key Vault with an ES256 signing key, App
Configuration, Azure SQL (Entra-only auth), Azure OpenAI with one pinned
model deployment, Log Analytics + Application Insights, three user-assigned
managed identities (API runtime, image pull, SQL migration), and optional
scheduled-query alerts plus a monthly budget. All data services have local
authentication and public data-plane access disabled; the API alone has
public TLS ingress, restricted to your tenant and audience.

**Cost note:** the stack runs continuously once deployed (Container Apps,
SQL, OpenAI, Log Analytics). Deploy `deployAlerts=true` with
`monthlyBudgetAmount` set so you get budget notifications from day one.

---

## 0. Prerequisites

Tooling on your workstation:

```powershell
winget install Microsoft.AzureCLI
az bicep upgrade
winget install Microsoft.Sqlcmd        # go-sqlcmd, supports Entra auth
az login --tenant <your-tenant-id>
az account set --subscription <subscription-id>
```

Azure permissions you personally need:

- **Owner** (or Contributor + User Access Administrator) on the target
  subscription or resource group — the template creates role assignments.
- Ability to create **Entra security groups** and one **app registration**.
- Membership in (or control of) the SQL administrators group you create
  below, so you can run the SQL grant scripts.

Repository state: `origin/main` at the revision you intend to qualify
(record the 40-char SHA now — every piece of evidence must cite it).

---

## 1. One-time Entra setup

### 1.1 SQL administrators group

```powershell
az ad group create --display-name "Sapphirus SQL Administrators" --mail-nickname sapphirus-sql-admins
az ad group member add --group "Sapphirus SQL Administrators" --member-id (az ad signed-in-user show --query id -o tsv)
az ad group show --group "Sapphirus SQL Administrators" --query id -o tsv   # -> sqlAdministratorObjectId
```

### 1.2 API app registration (the token audience)

```powershell
az ad app create --display-name "Sapphirus Desktop Support API" --sign-in-audience AzureADMyOrg
```

Then, in the Entra portal for that app:

1. **Expose an API** → set the Application ID URI (e.g.
   `api://<app-client-id>`). This becomes `entraAudience`.
2. Add a scope (e.g. `Desktop.Support`) so the desktop client can request
   `api://<app-client-id>/Desktop.Support` — this is the
   `SAPPHIRUS_SUPPORT_SCOPE` value at desktop-enablement time.
3. Create (or reuse) the **desktop client** app registration as a public
   client and grant it that scope. Record its client ID for
   `SAPPHIRUS_SUPPORT_API_CLIENT_ID`.

`entraAuthority` is your tenant-specific authority:
`https://login.microsoftonline.com/<tenant-id>/v2.0`. The template rejects
the `common` authority by design.

### 1.3 Test group

Create a small Entra group for pilot/test users; only these accounts should
be exercised against the deployment until Stage 7 passes.

---

## 2. Prepare your parameter file (never commit it)

Copy the example and fill it in **outside the repository** (e.g.
`C:\secrets\sapphirus-int.bicepparam`) or add it to a path git ignores.
`main.example.bicepparam` documents every parameter:

```bicep
using 'C:/Users/rodri/source/bmad-runtime-dev/infra/desktop-support/main.bicep'

param namePrefix = 'sapphirus-int'          // 3-18 chars, lowercase
param deployApi = false                     // stage 1 is always false
param entraAuthority = 'https://login.microsoftonline.com/<tenant-id>/v2.0'
param entraAudience = 'api://<api-app-client-id>'
param sqlAdministratorObjectId = '<group object id from 1.1>'
param sqlAdministratorLogin = 'Sapphirus SQL Administrators'
param modelName = 'gpt-5'                   // the reviewed Azure OpenAI model
param modelVersion = '<reviewed pinned version>'
param releaseChannel = 'beta'
param providerProfileHash = 'sha256:<...>'
param modelProfileHash = 'sha256:<...>'
param modelCapabilityHash = 'sha256:<...>'
param deploymentHash = 'sha256:<...>'
param deployAlerts = true
param actionGroupId = '<action group resource id>'   // create one first, see below
param monthlyBudgetAmount = 200
param environmentTag = 'staging'
param tags = { application: 'sapphirus', distribution: 'internal' }
```

**The four canonical hashes** bind the server to the exact reviewed
provider/model/capability/deployment documents the desktop client also
pins (`crates/desktop-cloud`). They must be the canonical
`sapphirus:<purpose>:v1` document hashes of the reviewed profile documents
for the model you actually deploy — the client refuses a policy whose
hashes differ from its pinned set, so **these values and the desktop's
pinned values must come from the same review**. If you change model name or
version, the hashes must be re-derived and re-reviewed together.

**Action group** (for alerts and the budget):

```powershell
az monitor action-group create --resource-group <rg> --name sapphirus-support-alerts `
  --short-name sapph-sup --action email admin <your-email>
az monitor action-group show --resource-group <rg> --name sapphirus-support-alerts --query id -o tsv
```

---

## 3. Stage 1 — infrastructure only (`deployApi=false`)

### 3.1 Resource group and policy inspection

```powershell
az group create --name sapphirus-support-rg --location <region>
az policy assignment list --disable-scope-strict-match --query "[].{name:displayName, enforcement:enforcementMode}" -o table
```

Read the policy list: nothing may auto-enable local auth, inject public
network access, or attach mutable-tag requirements. If a policy conflicts,
resolve it before deploying — do not add exemptions silently.

### 3.2 Validate and what-if

```powershell
az bicep build --file infra/desktop-support/main.bicep
az deployment group validate --resource-group sapphirus-support-rg --parameters C:\secrets\sapphirus-int.bicepparam
az deployment group what-if  --resource-group sapphirus-support-rg --parameters C:\secrets\sapphirus-int.bicepparam
```

Review the complete what-if output against this checklist (all must be
absent):

- [ ] no resource with `localAuth`/`disableLocalAuth=false` or shared-key access enabled
- [ ] no data service with `publicNetworkAccess: Enabled` (the API ingress is the only public endpoint)
- [ ] no plaintext secret or connection string in properties
- [ ] no container image referenced by mutable tag (digest only)
- [ ] no role assignment broader than the module comments state
- [ ] no caller-controlled model endpoint (the OpenAI endpoint is template-owned)

### 3.3 Deploy

```powershell
az deployment group create --resource-group sapphirus-support-rg `
  --name support-stage1 --parameters C:\secrets\sapphirus-int.bicepparam
az deployment group show --resource-group sapphirus-support-rg --name support-stage1 --query properties.outputs
```

Record every output — you need `sqlServerFqdn`, `sqlDatabaseName`,
`registryLoginServer`, `apiManagedIdentityObjectId`,
`sqlMigrationIdentityObjectId`, `sqlMigrationIdentityClientId`,
`signingKeyUri`, `appConfigurationEndpoint`, `modelEndpoint`.

Role-assignment convergence can take minutes; re-running the same
deployment is safe and idempotent.

---

## 4. Stage 2 — SQL authority

Everything here runs as a member of the SQL administrators group.
`sqlcmd -G` uses Entra authentication; the SQL public endpoint is disabled,
so run from a network path allowed by the private endpoint (the simplest
operator route is a temporary Azure VM / Cloud Shell inside the VNet, or a
temporary approved private-endpoint peering — never enable public access).

### 4.1 Create the two contained users

Using the exact statements in
[sql-grants.md](../../infra/desktop-support/sql-grants.md) — with the
**actual identity names** (`<namePrefix>-migration-id` and the API identity
name shown in the deployment outputs; adjust the bracketed names in the
script to match):

```powershell
sqlcmd -G -S <sqlServerFqdn> -d <sqlDatabaseName> -i infra/desktop-support/sql-grants-filled.sql
```

(Copy `sql-grants.md`'s two SQL blocks into a local scratch file with the
real identity names; keep migration and runtime grants exactly as written —
the runtime identity must never receive DDL.)

### 4.2 Apply migrations under the migration identity

Apply `services/desktop-support-api/Sql/Migrations/*.sql` **in name order**
(currently `0001_support_authority.sql`), recording each script name and
SHA-256 into `dbo.desktop_schema_migrations` — this is what
`SqlMigrationRunner.ApplyAsync` does. Two options:

- Run the API's migration runner once from a machine that holds the
  migration identity (client ID = `sqlMigrationIdentityClientId`), or
- Apply the script manually via `sqlcmd -G` as the migrator identity, then
  insert the `(name, sha256)` row yourself:

```powershell
Get-FileHash -Algorithm SHA256 services/desktop-support-api/Sql/Migrations/0001_support_authority.sql
```

The runner refuses hash-divergent re-application; never edit an applied
migration — add a new numbered script instead.

---

## 5. Stage 3 — publish the API image by digest

CI (`.github/workflows/desktop-support.yml`, `support-container` job)
proves the image builds and scans cleanly, but the publish is yours:

```powershell
az acr login --name <registry name from registryLoginServer>
docker build -t <registryLoginServer>/desktop-support-api:build services/desktop-support-api
docker push <registryLoginServer>/desktop-support-api:build
az acr repository show-manifests --name <registry> --repository desktop-support-api --query "[0].digest" -o tsv
```

Record the immutable reference:
`<registryLoginServer>/desktop-support-api@sha256:<digest>`. From here on,
**only the digest form is ever used** — the Bicep asserts this
(`immutableApiImage`). Note: the Dockerfile base images carry reviewed
digest pins (readiness Task 3); if it still contains
`REPLACE_WITH_REVIEWED_*` placeholders, resolve and review those digests
first — the build fails closed on purpose until then.

---

## 6. Stage 4 — first API revision, model disabled

Re-deploy the same template with two parameter changes:

```bicep
param deployApi = true
param containerImage = '<registryLoginServer>/desktop-support-api@sha256:<digest>'
```

```powershell
az deployment group create --resource-group sapphirus-support-rg `
  --name support-stage4 --parameters C:\secrets\sapphirus-int.bicepparam
```

Then ensure the App Configuration policy's `approvedModelDeployments` list
is **empty** — with no approved deployment, the fixed-profile broker
refuses model access at profile resolution, so the API is live but the
model path is closed. Verify ingress:

```powershell
$fqdn = az deployment group show -g sapphirus-support-rg -n support-stage4 --query properties.outputs.apiFqdn.value -o tsv
Invoke-RestMethod "https://$fqdn/healthz/ready"
```

---

## 7. Stage 5 — gate verification

Run the repository's smoke gates against the live deployment:

```powershell
# Identity, signed policy shape, lease fail-closure, health:
tools/support-smoke/deployed-smoke.ps1 -SupportOrigin "https://$fqdn" `
  -TenantId <tenant-id> -Scope api://<api-app-client-id>/.default

# Privacy inspection — read-only identity, confirms no content columns:
sqlcmd -G -S <sqlServerFqdn> -d <sqlDatabaseName> -i tools/support-smoke/privacy-sql-inspect.sql
```

Also confirm in the portal:

- the seven scheduled-query alerts from `modules/monitor-alerts.bicep` are
  **enabled**, including the sev-0 privacy canary rule;
- Key Vault audit and SQL diagnostics flow into the Log Analytics
  workspace;
- the budget from `modules/budget.bicep` exists with your action group.

All gates must pass before any model deployment is approved.

---

## 8. Stage 6 — enable model access for the test group

In App Configuration (`appConfigurationEndpoint`), add the single fixed
model deployment to `approvedModelDeployments`. Keep the budget fixed and
the test group as the only user population. One deployment, one version —
the same values your parameter hashes were derived from.

---

## 9. Stage 7 — end-to-end and failure injection

**Happy path** (Entra test account, desktop test installation): sign-in →
registration → signed policy fetch → entitlement lease → reviewed consent →
one **no-store** model call → signed receipt → Rust-side verification →
local Help completion. The local analog of the full sequence is
`crates/desktop-cloud/tests/production_lifecycle.rs`; the deployed run must
match it check-for-check.

**Failure matrix** — exercise each and record the observed fail-closed
behavior:

| Drill | How | Expected |
|---|---|---|
| Revocation race | Revoke the registration mid-lease | In-flight call rejected; client falls to explicit offline |
| SQL transient | Restart SQL or inject a failover | API retries bounded, no partial receipt |
| Key Vault throttle | Burst signing requests | Bounded backoff, no unsigned document served |
| Model timeout | Reduce model timeout / use oversized context | Timeout surfaces as typed error, no store |
| Receipt replay | Re-submit a consumed consent | Rejected (single-use consumption) |
| Key rotation overlap | New Key Vault key version, keep old for verification | Client accepts documented overlap only |
| Revision rollback | `az containerapp revision` route to prior revision | Old digest serves, health green |
| Point-in-time restore | Rehearse SQL PITR to a scratch database | Restore succeeds; production untouched |
| Kill switch | Clear `approvedModelDeployments` | New admissions blocked within one policy refresh; local history/rollback unaffected |

---

## 10. Stages 8-9 — promotion and desktop enablement

- **Promotion:** promote the **same** image digest through environments;
  never rebuild per environment.
- **Desktop enablement:** build the desktop with the `production-support`
  feature and the complete package-controlled value set —
  `SAPPHIRUS_SUPPORT_TENANT_ID`, `SAPPHIRUS_SUPPORT_API_CLIENT_ID`,
  `SAPPHIRUS_SUPPORT_SCOPE`, `SAPPHIRUS_SUPPORT_ORIGIN`
  (`https://<apiFqdn>`), `SAPPHIRUS_SUPPORT_REGION` — plus the pinned
  policy/receipt trust keys. Partial values intentionally leave the client
  in explicit offline behavior; exact production configuration that cannot
  initialize fails closed.

## 11. Rollback (always available)

1. Route traffic to the prior Container Apps revision.
2. Clear `approvedModelDeployments` (model kill switch).
3. Revoke signing-key versions or identity assignments only through the
   audited incident procedure.
4. Never destructively roll back a SQL migration — forward repair only.

---

## 12. Evidence to record (readiness scorecard)

For `docs/readiness/100-percent-scorecard.json`, this deployment produces
the `production_model_backed_help` evidence set. Record — **bounded values
only, no tenant subjects, tokens, prompts, response bodies, or secret
URIs**:

| Evidence kind | What to capture |
|---|---|
| `azure_e2e` | Source revision, container digest, deployment name/revision, pass timestamp of the Stage 7 happy path |
| `signed_receipt` | Receipt verification pass (count + timestamp), signing key version identifier |
| `failure_injection` | The Stage 7 drill table with pass/fail and timestamps |

These also feed `docs/qualification-evidence/d2-production-summary.json`
(readiness Task 10 Step 5) with: source revision, container digest,
deployment revision, workflow URLs, test counts, timestamps, key-version
identifiers, and pass/fail — nothing else.
