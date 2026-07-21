# Sapphirus Desktop Support Bicep — Errors, Warnings and Deployment Problems

**Working folder:** `~/clouddrive/desktop-support`  
**Primary template:** `main.bicep`  
**Working parameters:** `main.dev.bicepparam`  
**Generated parameters:** `main.dev.parameters.json`  
**Backup:** `main.bicep.before-warning-fixes`  

## Summary

The uploaded Bicep package now compiles, but deployment is not ready. Three Bicep structural warnings were corrected. Two non-blocking warnings remain. Seven parameter placeholders and a confirmed Microsoft Entra permission restriction still block deployment.

**Repository synchronization (2026-07-21):** the BIC-003, BIC-004, and
BIC-005 corrections originally applied only to the Cloud Shell working copy
are now ported to the repository's `infra/desktop-support/main.bicep`, and
BIC-006 is resolved there with `az.environment().suffixes.sqlServerHostname`
(the unqualified `environment()` function is shadowed by the managed-environment
resource symbol). `main.json` was regenerated from the fixed template. The
only remaining build warning is the intentional BIC-007 experimental-assertions
notice. All CFG/IAM/GOV/NET/AZR/OPS items remain open as recorded below.

## Issue register

### BIC-001 — Example parameter contained a placeholder tenant authority

**File:** `main.example.bicepparam` / working copy `main.dev.bicepparam`  
**Parameter:** `entraAuthority`  
**Initial state:** zero/placeholder tenant value  
**Real tenant ID:** `deff24bb-2089-4400-8c8e-f71e680378b2`  
**Resolution:** replaced and verified against `az account show`; generated JSON comparison returned `true`.  
**Status:** Resolved.

Correct value:

```bicep
param entraAuthority = 'https://login.microsoftonline.com/deff24bb-2089-4400-8c8e-f71e680378b2/v2.0'
```

### BIC-002 — Apparent HTML anchors in pasted terminal output

**Symptom:** URLs appeared in chat as anchor markup such as `a href`, `target`, and `rel`.  
**Finding:** `az bicep build-params` succeeded and the generated JSON matched the expected authority exactly. The markup was introduced by the chat/output presentation layer, not stored as invalid Bicep in the working file.  
**Status:** Resolved / false alarm.

### BIC-003 — Log Analytics `sku` at incorrect structural level

**File:** `main.bicep`  
**Original line area:** 167–179  
**Diagnostic:** `BCP187`  
**Problem:** `sku` was placed beside `properties`, while the recognized workspace schema expects it inside `properties`.  
**Fix applied:** moved `sku` inside `properties`.  
**Status:** Resolved; warning disappeared.

Corrected block:

```bicep
properties: {
  retentionInDays: 30
  publicNetworkAccessForIngestion: 'Enabled'
  publicNetworkAccessForQuery: 'Enabled'
  sku: {
    name: 'PerGB2018'
  }
}
```

### BIC-004 — Container Apps subnet property at incorrect level

**File:** `main.bicep`  
**Original line:** approximately 350  
**Diagnostic:** `BCP037`  
**Problem:** `infrastructureSubnetId` was directly under managed-environment `properties`; the recognized schema requires it under `vnetConfiguration`.  
**Fix applied:** nested the subnet ID under `vnetConfiguration`.  
**Status:** Resolved; warning disappeared.

Corrected block:

```bicep
vnetConfiguration: {
  infrastructureSubnetId: containerAppsSubnet.id
}
```

### BIC-005 — Conditional Container App output could dereference null

**File:** `main.bicep`  
**Original line:** approximately 609  
**Diagnostic:** `BCP318`  
**Problem:** `api` is created only when `deployApi=true`, but the output accessed `api.properties...` directly. With `deployApi=false`, the resource may be null.  
**Fix applied:** safe dereference with empty-string fallback.  
**Status:** Resolved; warning disappeared.

Corrected output:

```bicep
output apiFqdn string = api.?properties.configuration.ingress.fqdn ?? ''
```

### BIC-006 — Hardcoded SQL private DNS public-cloud suffix

**File:** `main.bicep`  
**Current line:** approximately 508  
**Diagnostic:** `no-hardcoded-env-urls`  
**Current value:** `privatelink.database.windows.net`  
**Impact:** portability warning across Azure clouds; does not block compilation in the current public-Azure target.  
**Status:** Resolved in the repository template (2026-07-21) with
`zone: 'privatelink${az.environment().suffixes.sqlServerHostname}'`, which
renders identically in public Azure.

### BIC-007 — Experimental assertions enabled

**File:** `bicepconfig.json`  
**Configuration:**

```json
{
  "experimentalFeaturesEnabled": {
    "assertions": true
  }
}
```

**Diagnostic:** experimental feature warning on every build.  
**Impact:** compilation succeeds, but Bicep warns that experimental features are not guaranteed for production use.  
**Status:** Open. Keep for DEV while the template depends on assertions; review before production.

### CFG-001 — API audience placeholder

**File:** `main.dev.bicepparam`  
**Current value:** `api://00000000-0000-0000-0000-000000000000`  
**Required value:** Application (client) ID of the approved Sapphirus Support API registration.  
**Search result:** no existing app registration found under the searched names.  
**Status:** Blocking.

### CFG-002 — SQL administrator Object ID placeholder

**File:** `main.dev.bicepparam`  
**Current value:** zero GUID  
**Required value:** Object ID of the approved Microsoft Entra SQL administrators group.  
**Status:** Blocking.

### CFG-003 — Model version placeholder

**File:** `main.dev.bicepparam`  
**Current value:** `replace-with-reviewed-version`  
**Required value:** reviewed and available Azure OpenAI model version.  
**Status:** Blocking.

### CFG-004 — Provider profile hash placeholder

**Current value:** `sha256:aaaaaaaa...`  
**Required value:** canonical reviewed provider profile hash.  
**Status:** Blocking.

### CFG-005 — Model profile hash placeholder

**Current value:** `sha256:aaaaaaaa...`  
**Required value:** canonical reviewed model profile hash.  
**Status:** Blocking.

### CFG-006 — Model capability hash placeholder

**Current value:** `sha256:aaaaaaaa...`  
**Required value:** canonical reviewed capability profile hash.  
**Status:** Blocking.

### CFG-007 — Deployment hash placeholder

**Current value:** `sha256:aaaaaaaa...`  
**Required value:** canonical reviewed deployment document/configuration hash.  
**Status:** Blocking.

### IAM-001 — Microsoft Entra group creation denied

**Command:** `az ad group create`  
**Result:** `ERROR: Insufficient privileges to complete the operation.`  
**Meaning:** the current account’s Azure `Contributor` assignment does not provide the Microsoft Entra directory permission needed to create the SQL administrators group.  
**Status:** Blocking; request group creation through the approved identity process.

### IAM-002 — API application registration absent

**Searches:** exact and `Sapphirus`-prefix `az ad app list` searches  
**Result:** no matching application returned.  
**Status:** Blocking; request the app registration and consent through KPMG Marketplace/ServiceNow.

### IAM-003 — Role-assignment capability not proven

**Template behavior:** creates Azure role assignments.  
**Known account role:** inherited Azure `Contributor`.  
**Risk:** ordinary Contributor does not itself prove permission to create role assignments.  
**Status:** Blocking until effective role-assignment permission is verified or an authorized administrator is assigned the RBAC stage.

### GOV-001 — Mandatory KPMG tag values not supplied

Required controlled values include:

```text
Environment
Charge_Code
Application_Service
Application_ID
Ticket_Number
IaC_Status for IaC-managed resources
```

**Status:** Blocking. Placeholder or invented values must not be deployed.

### NET-001 — Container Apps public network access requires review

**File:** `main.bicep`  
**Current setting:**

```bicep
publicNetworkAccess: 'Enabled'
```

**Concern:** the internal deployment guidance favors an internal-only Container Apps environment unless external access is specifically required. The intended desktop-client access path must be agreed before changing this value.  
**Status:** Open design decision; potentially blocking.

### NET-002 — Cloud Shell private endpoint reachability not proven

The infrastructure uses private endpoints for SQL and other data services. Successful Azure resource deployment does not prove that the current Cloud Shell session can reach those private endpoints for SQL grants and migrations.  
**Status:** Open and blocking for the SQL setup stage.

### AZR-001 — Resource-group creation is outside current template

`main.bicep` has no subscription-scope entry point and defaults to resource-group scope. It deploys into an existing resource group and does not create the final application resource group.  
**Status:** Open. Create the approved/tagged RG separately or add a reviewed subscription-scope wrapper.

### AZR-002 — Azure OpenAI model/quota availability not verified

The template declares Azure OpenAI and a model deployment, but the reviewed model version, SKU, capacity and regional quota have not been confirmed.  
**Status:** Blocking.

### OPS-001 — Alerts disabled pending action group

Current DEV parameter intent keeps:

```bicep
param deployAlerts = false
```

until a valid approved `actionGroupId` exists.  
**Status:** Expected staged behavior; alerts must be enabled before qualification is complete.

### OPS-002 — API intentionally disabled for foundation stage

Current DEV parameter intent keeps:

```bicep
param deployApi = false
```

until the image is built, scanned, pushed, and referenced by immutable digest.  
**Status:** Expected staged behavior.

## Verified successful checks

- Archive extracted successfully to persistent Cloud Shell storage.
- `main.dev.bicepparam` created.
- Real tenant ID inserted.
- Tenant authority exact-match check returned `true`.
- `az bicep build` succeeds.
- `az bicep build-params` succeeds.
- `main.dev.parameters.json` generated.
- `BCP187` resolved.
- `BCP037` resolved.
- `BCP318` resolved.

## Required next actions

1. Submit the KPMG application-registration/consent request for the API and desktop client.
2. Request creation of the SQL administrators group and obtain its Object ID.
3. Verify who can perform role assignments created by the template.
4. Obtain mandatory KPMG tag values.
5. Obtain the reviewed Azure OpenAI model version/SKU/capacity and quota confirmation.
6. Obtain the four canonical reviewed hashes.
7. Resolve the Container Apps public/private ingress design.
8. Confirm a network path for private SQL grants and migrations.
9. Replace every placeholder, rebuild parameters, then run `validate` and `what-if` only.

## Deployment prohibition

Do not run `az deployment group create` while any blocking issue above remains.
