# Azure support-plane deployment guide (organization-controlled operator runbook)

This is the step-by-step operator guide for readiness-program **Task 10**:
deploying and qualifying the D2 support plane on Azure. It expands the
[rollout runbook](../superpowers/plans/2026-07-20-d2-e-rollout-runbook.md)
into concrete operator checks and commands.

This document is a technical runbook, **not authorization to create or change
Azure resources**. The organization's cloud platform, identity, security,
privacy, FinOps, release, and change-management procedures take precedence.
Every stage must run against an organization-vended workload landing zone,
under an approved change record, through an approved deployment identity and
execution path. Never substitute a personal subscription, personal Azure
Owner assignment, self-created Entra object, or local container push for a
missing organization control.

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
SQL, OpenAI, Log Analytics). Deploy `deployAlerts=true` with an approved
`monthlyBudgetAmount` and organization-owned notification action group.
Azure budgets notify; they do not cap usage or stop resources. FinOps approval
and a separate overrun response are therefore required.

---

## 0. Authorization and prerequisites

### 0.1 Organization intake and approval gate

Open the organization's workload-onboarding and change records before doing
anything in Azure. Record the identifiers, owners, and approvals below. A chat,
verbal approval, or successful CLI command is not a substitute.

| Required record | Minimum content | Accountable approval |
|---|---|---|
| Workload / landing-zone intake | workload name, business owner, technical owner, criticality, environment, subscription, management group, region and resource-group boundary | Cloud platform owner |
| Data and AI review | data classification, prompt/output handling, regional/data-residency decision, retention, approved Azure OpenAI model/version, abuse/safety controls | Data owner plus Security/Privacy |
| Network design | VNet/IPAM allocation, private DNS ownership, ingress and egress paths, operator/runner path to private endpoints | Network/platform owner |
| Identity design | Entra applications, groups, workload identities, federated credentials, Conditional Access impact, RBAC scopes, PIM eligibility | Identity owner |
| Cost plan | cost centre, budget owner, approved monthly amount, quota, alert recipients, overrun action | FinOps/business owner |
| Operations plan | service owner, on-call route, health/alert ownership, backup/restore RPO/RTO, incident and key-rotation procedures | Operations owner |
| Release/change record | immutable source revision and image digest, what-if artifact, test evidence, maintenance window, rollback owner, approvals | Release/change authority |
| Evidence location | access-controlled evidence store, retention period, reviewer list, scorecard-summary policy | Security/release owner |

Do not start Stage 1 until every row has a real organization record and named
owner. If the organization already provides a subscription-vending or
application-landing-zone workflow, use that front door; do not create an
independent resource group in an arbitrary subscription.

### 0.2 Access and separation of duties

- Assign Azure roles to organization-managed Entra groups or workload
  identities, not directly to an operator. Use PIM/JIT activation for
  privileged human access and the narrowest approved scope.
- The application operator does **not** require standing Owner access. The
  deployment identity needs resource writes at the target resource group and,
  because this template creates scoped role assignments,
  `Microsoft.Authorization/roleAssignments/write` only where those assignments
  are created. Prefer a constrained custom role or have the platform team own
  the role-assignment step. Do not grant subscription Owner merely to make the
  template pass.
- Identity administrators own the Entra app registrations, groups, consent,
  app ownership, and federated credentials. The workload team supplies the
  requested names, redirect/audience/scope values, and owners.
- Keep deployment, SQL administration, SQL migration, runtime, read-only
  inspection, and approval identities distinct. Runtime identity never gets
  DDL or Azure control-plane rights.
- Production approval must follow the organization's separation-of-duties
  rule. The requester/developer must not silently self-approve a protected
  environment or policy exemption.
- Break-glass access follows the organization's incident process and is not a
  normal deployment path.

### 0.3 Approved execution path and tooling

The preferred path is an organization-managed deployment pipeline using
GitHub OIDC/workload identity federation and a protected environment with
required reviewers and branch/tag restrictions. Configure
`desktop-support-staging` (and a separate production environment) before
running Azure gates. Store tenant, subscription, resource-group, and endpoint
values as protected environment configuration; do not create a long-lived
client secret.

The current `.github/workflows/desktop-support.yml` Azure job validates and
tests an existing deployment; it does **not** deploy infrastructure or publish
an image. Until an organization-reviewed deployment/release lane exists, do
not describe the workflow as a production deployment lane.

The following workstation commands are reference commands for an authorized
operator. Use them only when the change record explicitly permits the manual
path and the device satisfies organization management and Conditional Access
requirements:

```powershell
winget install Microsoft.AzureCLI
az bicep upgrade
winget install Microsoft.Sqlcmd        # go-sqlcmd, supports Entra auth
az login --tenant <your-tenant-id>
az account set --subscription <subscription-id>
```

Repository state must be the approved, clean revision (record the 40-character
SHA; every evidence item must cite it). Parameter files and what-if artifacts
belong in the organization's approved restricted workspace or CI artifact
store, never in the repository.

### 0.4 Cloud Shell and portal use

Cloud Shell or portal deployment is optional, not a governance bypass. Use it
only if the organization approves browser-based administration, source upload,
storage, session logging, and the operator's Conditional Access posture.

- Default Cloud Shell runs outside your workload VNet and cannot reach this
  stack's private SQL, Key Vault, App Configuration, ACR, or Azure OpenAI
  endpoints. Stage 2 and private data-plane checks require an
  organization-provisioned VNet-isolated Cloud Shell, a VNet-connected
  managed runner, or an approved privileged access workstation/jump host.
- Ephemeral Cloud Shell avoids persistence but does not remove data-handling,
  audit, egress, or device-policy requirements. Do not upload a filled
  parameter file or repository archive unless that transfer is approved.
- Portal custom-template deployment is not a replacement for the recorded
  CLI/REST what-if artifact, independent review, and change approval.
- ACR Tasks or an interactive Docker push is not a replacement for the
  signed/SBOM-attested organization release lane required by Stage 3.

### 0.5 Hard stop conditions

Stop and return to the accountable owner when any of these is true:

- a required intake, security/privacy, cost, change, or production approval is
  missing;
- the target is not an organization-vended workload subscription/landing zone,
  or its management-group/policy inheritance is unknown;
- deployment requires standing personal Owner access, an unapproved policy
  exemption, a client secret, public access to a private dependency, or an
  unreviewed egress path;
- what-if contains an unexpected delete, replacement, scope change, public
  endpoint, broad role assignment, or policy remediation;
- the protected environment, OIDC subject, deployment identity, approver set,
  or evidence store is not configured and independently reviewed;
- Azure OpenAI approval, quota, region, model/version review, or canonical
  profile hashes are unresolved;
- the image is mutable, locally pushed, unsigned/unattested, not SBOM-bound, or
  not tied to the approved source revision; or
- private DNS/operator connectivity, monitoring ownership, backup/restore,
  incident response, or rollback proof is missing.

---

## 1. One-time Entra setup

The identity team provisions or approves these objects under the identity
intake record. The commands below are implementation examples for an authorized
identity administrator; they are not an instruction for the application
operator to grant themselves directory privileges. Require organization naming,
at least two accountable application owners where policy requires it, group-
based lifecycle management, and an offboarding owner.

### 1.1 SQL administrators group

```powershell
az ad group create --display-name "Sapphirus SQL Administrators" --mail-nickname sapphirus-sql-admins
az ad group show --group "Sapphirus SQL Administrators" --query id -o tsv   # -> sqlAdministratorObjectId
```

Membership is assigned through the organization's access-request/PIM process;
do not add the deploying user directly as an undocumented convenience.

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
   client and grant it that scope. Record its Application (client) ID as
   `approvedDesktopClientId`; the API requires this reviewed desktop
   public-client app registration in the delegated token's `azp`/`appid`
   claim. The same ID is used for `SAPPHIRUS_SUPPORT_API_CLIENT_ID`.

The identity review must also record the API and desktop app owners, consent
model, authorized redirect URIs/public-client settings, pilot-group assignment,
federated deployment credential subject, and periodic access-review owner. No
client secret is needed for the desktop or GitHub deployment path.

`entraAuthority` is your tenant-specific authority:
`https://login.microsoftonline.com/<tenant-id>/v2.0`. The template rejects
the `common` authority by design.

### 1.3 Test group

Have the identity team create or approve a small lifecycle-managed Entra group
for pilot/test users. Only these accounts should be entitled to the deployment
until Stage 7 and the pilot approval gate pass.

---

## 2. Prepare the reviewed parameter set (never commit it)

Copy the example into the organization's approved restricted workspace or
protected pipeline artifact/configuration store. Do not rely on a merely
git-ignored local file for production. `main.example.bicepparam` documents
every parameter:

```bicep
using 'C:/Users/rodri/source/bmad-runtime-dev/infra/desktop-support/main.bicep'

param namePrefix = 'sapphirus-int'          // 3-18 chars, lowercase
param deployApi = false                     // stage 1 is always false
param entraAuthority = 'https://login.microsoftonline.com/<tenant-id>/v2.0'
param entraAudience = 'api://<api-app-client-id>'
param approvedDesktopClientId = '<desktop-public-client-app-id>'
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
param tags = {
  application: 'sapphirus'
  distribution: 'internal'
  owner: '<organization-owner-id>'
  costCenter: '<approved-cost-centre>'
  dataClassification: '<approved-classification>'
}
```

**The four canonical hashes** (stage-1 note: the example's
placeholder values are acceptable while `deployApi = false`; the real
reviewed values are required before stage 4 sets `deployApi = true`)
bind the server to the exact reviewed
provider/model/capability/deployment documents the desktop client also
pins (`crates/desktop-cloud`). They must be the canonical
`sapphirus:<purpose>:v1` document hashes of the reviewed profile documents
for the model you actually deploy — the client refuses a policy whose
hashes differ from its pinned set, so **these values and the desktop's
pinned values must come from the same review**. If you change model name or
version, the hashes must be re-derived and re-reviewed together.

**Action group** (for alerts and the budget):

Prefer an existing organization-owned action group whose recipients and
escalation path are maintained by Operations. If the change record authorizes
a workload-specific group, create it under the Operations owner rather than a
personal mailbox:

```powershell
az monitor action-group create --resource-group <rg> --name sapphirus-support-alerts `
  --short-name sapph-sup --action email operations <operations-distribution-list>
az monitor action-group show --resource-group <rg> --name sapphirus-support-alerts --query id -o tsv
```

---

## 3. Stage 1 — infrastructure only (`deployApi=false`)

### 3.1 Resource group and policy inspection

The platform team must vend or hand off the target subscription/resource group,
place the subscription under the approved management group, register required
resource providers, allocate network ranges/private DNS, and confirm naming and
tag policy. The workload operator verifies that handoff; they do not create an
ad hoc resource group as part of this runbook.

```powershell
az account show --query "{tenant:tenantId,subscription:id,name:name}" -o json
$rgId = az group show --name <approved-resource-group> --query id -o tsv
az policy assignment list --scope $rgId --disable-scope-strict-match `
  --query "[].{name:displayName,scope:scope,enforcement:enforcementMode}" -o table
```

Attach the account output and inherited policy list to the change record. A
`modify` or `deployIfNotExists` policy may legitimately alter or add resources;
its effects must be represented in the what-if review. If a policy conflicts,
return to the platform/security owner. Never add, request, or rely on an
exemption without the organization's documented exception process, owner, and
expiry.

### 3.2 Validate and what-if

```powershell
az bicep build --file infra/desktop-support/main.bicep
az deployment group validate --resource-group <approved-resource-group> `
  --parameters <approved-parameter-file>
az deployment group what-if --resource-group <approved-resource-group> `
  --parameters <approved-parameter-file> --result-format FullResourcePayloads
```

Export the complete what-if result to the protected change/evidence store and
have a reviewer who did not author the change compare it with the approved
architecture and prior environment. What-if predicts changes but is not an
approval and can report noise for unresolved template expressions.

Review against this checklist:

- [ ] no unexpected delete, replacement, location, SKU, network, DNS, policy, or tag change
- [ ] no resource with `localAuth`/`disableLocalAuth=false` or shared-key access enabled
- [ ] ACR, Key Vault, App Configuration, SQL, and Azure OpenAI public data-plane access remain disabled
- [ ] public Container Apps ingress is the only workload ingress; Log Analytics/Application Insights public ingestion/query in the current template is explicitly accepted by Security or the template is changed before deployment
- [ ] no plaintext secret or connection string in properties
- [ ] no container image referenced by mutable tag (digest only)
- [ ] role assignments are limited to the documented managed identities, roles, and resource scopes; no human or subscription-wide assignment is introduced
- [ ] no caller-controlled model endpoint (the OpenAI endpoint is template-owned)
- [ ] every policy-added or modified resource is understood and owned

The cloud platform, Security/Privacy, Operations, and FinOps reviewers sign the
change record as applicable. Production also requires the release/change
authority and tested rollback evidence.

### 3.3 Deploy

```powershell
az deployment group create --resource-group <approved-resource-group> `
  --name support-stage1 --parameters <approved-parameter-file>
az deployment group show --resource-group <approved-resource-group> `
  --name support-stage1 --query "{id:id,outputs:properties.outputs,correlationId:properties.correlationId}"
```

Run this apply only from the approved deployment identity/path after the change
record is authorized and, for a manual path, the operator's time-bound PIM role
is active. A locally authenticated apply is not the production default. Record
the deployment ID, correlation ID, operator/workload identity, approver,
timestamp, source SHA, parameter-set hash, and what-if artifact reference.

Record every output — you need `sqlServerFqdn`, `sqlDatabaseName`,
`registryLoginServer`, `apiManagedIdentityObjectId`,
`apiManagedIdentityClientId`, `apiManagedIdentityName`,
`imagePullIdentityObjectId`, `imagePullIdentityResourceId`,
`sqlMigrationIdentityObjectId`, `sqlMigrationIdentityClientId`,
`sqlMigrationIdentityResourceId`, `sqlMigrationIdentityName`,
`signingKeyUri`, `appConfigurationEndpoint`, `modelEndpoint`.

Role-assignment convergence can take minutes. Diagnose propagation first;
re-run only under the same open change record and after confirming a new
what-if has no unexpected changes.

---

## 4. Stage 2 — bootstrap SQL migration authority

The bootstrap step runs as a member of the SQL administrators group.
`sqlcmd -G` uses Entra authentication; the SQL public endpoint is disabled,
so run it from a network path allowed by the private endpoint (the simplest
operator route is a temporary Azure VM inside the VNet or temporary approved
private-endpoint peering — never enable public access).

### 4.1 Bootstrap the migration identity and ledger

Copy the **migration identity block only** from
[sql-grants.md](../../infra/desktop-support/sql-grants.md) into an approved
scratch file. Replace the bracketed placeholder with the actual
`sqlMigrationIdentityName` output (`<namePrefix>-sqlmigrate-id`), then run
that block as an Entra SQL administrator:

```powershell
sqlcmd -G -S <sqlServerFqdn> -d <sqlDatabaseName> -i <migration-bootstrap-filled.sql>
```

This creates the migration user, assigns its DDL role, creates the empty
`dbo.desktop_schema_migrations` ledger, and only then grants the migrator
SELECT/INSERT on that existing ledger. Do not apply the runtime block yet:
the application tables do not exist on a clean database.

---

## 5. Stage 3 — publish the image and apply SQL migrations

### 5.1 Consume the trusted release image by digest

The repository's current `support-container` CI job builds, tests, scans, and
creates an SBOM for an ephemeral image, but it deliberately does not publish
that image. Production requires an organization release lane that checks out
the exact approved source revision, builds the image once, runs the required
tests and scan against that same artifact, publishes it under the unique
`sha-<40-character-source-revision>` tag, and emits:

- the full source revision and immutable image digest;
- the SBOM and vulnerability-scan result bound to that digest; and
- the organization-approved image signature and provenance attestation bound
  to the same source revision, workflow identity, and digest.

This repository does not currently contain that trusted publisher. Deployment
is **blocked until** the organization release lane exists and an independent
reviewer accepts all of the exact-digest evidence above. Never substitute a
local `docker build` or `docker push`.

After the trusted lane completes, take its protected outputs — do not derive
the source revision or digest from the operator checkout — and verify that the
source-specific ACR tag still resolves to the exact published digest:

```powershell
$registryName = '<registry-name>'
$registryLoginServer = '<registryLoginServer output>'
$sourceRevision = '<release-lane source revision>'
$publishedDigest = '<release-lane image digest>'
if ($sourceRevision -cnotmatch '^[0-9a-f]{40}$') {
  throw 'The release source revision must be one full lowercase Git SHA.'
}
if ($publishedDigest -cnotmatch '^sha256:[0-9a-f]{64}$') {
  throw 'The release lane did not emit one valid immutable image digest.'
}
$imageTag = "sha-$sourceRevision"
$resolvedDigest = (az acr repository show --name $registryName `
  --image "desktop-support-api:$imageTag" --query digest --output tsv).Trim()
if ($LASTEXITCODE -ne 0 `
    -or $resolvedDigest -cnotmatch '^sha256:[0-9a-f]{64}$' `
    -or $resolvedDigest -cne $publishedDigest) {
  throw 'The source-revision tag does not match the approved release digest.'
}
$immutableImage = "$registryLoginServer/desktop-support-api@$publishedDigest"
```

Record `$sourceRevision`, `$imageTag`, `$publishedDigest`, `$immutableImage`,
the release workflow run, SBOM, scan, signature, attestation, and independent
approval together. From here on, **only the digest form is ever used** — the
Bicep asserts this (`immutableApiImage`).

### 5.2 Apply migrations under the migration identity

Run the published support image as a one-shot, manually triggered Azure
Container Apps Job in the same VNet-integrated managed environment. The
platform-owned job definition must satisfy all of these conditions:

- use the exact immutable image digest recorded in section 5.1;
- attach the `imagePullIdentityResourceId` and
  `sqlMigrationIdentityResourceId` outputs only — never attach the API
  runtime identity;
- inherit the image `ENTRYPOINT` and pass exactly one argument,
  `migrate-sql`; do not configure ingress, a listening port, or HTTP probes;
- set `ASPNETCORE_ENVIRONMENT=Production`, the same validated non-secret
  `Sapphirus__*` Production values intended for the API, and
  `Sapphirus__SqlMigrationIdentityClientId=<sqlMigrationIdentityClientId>`;
  set `Sapphirus__ManagedIdentityClientId=<apiManagedIdentityClientId>` so the
  command can verify identity separation, but do not attach that runtime
  identity to the job;
- set parallelism and replica completion count to `1`, retries to `0`, and a
  bounded execution timeout; and
- retain the execution record in the protected deployment evidence.

The container contract is therefore:

```text
$immutableImage migrate-sql
```

The command composes and validates Production configuration, verifies that the
migration and runtime client IDs are both present and distinct, constructs
`SqlMigrationRunner` with the migration client ID, applies embedded migrations
in name order, and records the runner-computed `sha256:<lowercase-hex>` plus
`applied_at`. It never starts the web server. Success exits `0`; any invalid
configuration, identity mismatch, connectivity failure, hash divergence, or
SQL error exits nonzero and blocks the deployment.

Start the approved job once and inspect its terminal execution status. Do not
continue unless the sole replica completed successfully. The runner is the
only supported migration path: never apply embedded scripts or write ledger
rows manually. It refuses hash-divergent re-application; never edit an applied
migration — add a new numbered script instead.

### 5.3 Create the runtime identity and apply DML grants

After the migration job exits `0`, copy the **runtime identity block only**
from `sql-grants.md` into a separate approved scratch file. Replace the
bracketed placeholder with the `apiManagedIdentityName` output, then run it as
an Entra SQL administrator from the approved private network path:

```powershell
sqlcmd -G -S <sqlServerFqdn> -d <sqlDatabaseName> -i <runtime-grants-filled.sql>
```

Keep the grants exactly as written. The runtime identity must never receive
DDL, and object grants must not be attempted before migrations create their
tables.

---

## 6. Stage 4 — first API revision, model disabled

Re-deploy the same template with two parameter changes:

```bicep
param deployApi = true
param containerImage = '<registryLoginServer>/desktop-support-api@sha256:<digest>'
```

Before this first API revision, replace the stage-1 placeholder
`approvedDesktopClientId` with the reviewed desktop public-client Application
(client) ID from section 1.2. The API validates this value at startup and
rejects delegated tokens whose authorized-party client does not match.

```powershell
az deployment group create --resource-group sapphirus-support-rg `
  --name support-stage4 --parameters C:\secrets\sapphirus-int.bicepparam
```

Then retain the required App Configuration
`approvedModelDeployments` key and set its value to the exact empty string
(`""`). Do not delete the key or use whitespace. With no approved
deployment, the fixed-profile broker refuses model access at profile
resolution, so the API is live but the model path is closed. Verify ingress:

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
| Kill switch | Retain `approvedModelDeployments` with the exact empty-string value (`""`); do not delete it or use whitespace | New admissions blocked within one policy refresh; local history/rollback unaffected |

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
2. Retain `approvedModelDeployments` with the exact empty-string value (`""`)
   (model kill switch); do not delete the required key or use whitespace.
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
