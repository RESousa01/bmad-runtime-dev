# Desktop support plane infrastructure

> **Status: undeployed historical scaffold.** This Bicep stack documents an earlier support-plane
> composition and has not been qualified against a real current environment. Parts of the stack
> still depend on Entra-era assumptions that are superseded as product direction by
> [ADR-0012](../../docs/adr/ADR-0012-no-entra-authentication-and-model-access.md). Do not deploy it
> as the current Forge production architecture; use it only as implementation evidence while the
> no-Entra production design, credentials, attribution, networking, and operator gates are closed.

This resource-group-scoped Bicep stack describes the D2 Azure support plane. It creates a
VNet-integrated Azure Container Apps environment, user-assigned managed identities, private
Azure Container Registry, Key Vault ES256 signing key, App Configuration, Azure SQL metadata
database, and workspace-based Application Insights. All data services have local
authentication and public network access disabled.

The model plane is **not** created here. It is an external, pre-existing Microsoft Foundry
project reached at `https://<account>.services.ai.azure.com/api/projects/<project>`. This
stack references that project and authenticates to it with a **backend-only API key**, never
a managed identity: it injects the complete project endpoint, the selected inventory mode, the
prerequisite-evidence hash, and the key (through a Container Apps secret) into the API. It
never creates a Foundry account, a Foundry project, or a model deployment, and it never names
a deployment: the model broker reads the project's own deployment catalog at runtime.

This is an internal, single-tenant deployment. The API itself has public TLS ingress so managed
employee devices can reach it, but tokens are accepted only for the configured organization tenant
and audience and the reviewed `approvedDesktopClientId`; there is no public registration or
multi-tenant onboarding path.

The template deliberately does not assign an Entra application registration or publish an image.
Its first stage uses `deployApi=false`, so the registry and support infrastructure can be created
without a circular image dependency. After the internal build pipeline publishes the image by
immutable digest, the second stage supplies that digest and sets `deployApi=true`. The API resource
also waits for its identity roles and private DNS resources to be created; the deployment pipeline
must still retry safely while Azure role assignments converge.

Before enabling production traffic, a database administrator must create the contained database
users for the emitted `apiManagedIdentityObjectId` (runtime DML only) and
`sqlMigrationIdentityObjectId` (schema migration only) following
`infra/desktop-support/sql-grants.md`. That data-plane grant is intentionally not hidden in an
ARM deployment script.

Identity split: image pull uses its own identity (`imagePullIdentityObjectId`), the SQL
migration identity is never attached to the API, and data/config and signing share the runtime
identity attached to one process. The module seams (`modules/`) are preserved so signing can be
moved to an independently deployed workload if the threat review requires hard isolation.
Foundry model access is **not** a managed identity and holds **no role assignment**: it
authenticates with a backend-only API key carried through a Container Apps secret
(`foundry-api-key`) and injected as `Sapphirus__FoundryApiKey` via `secretRef`. There is no
Foundry model-broker identity anywhere in this stack.

## Foundry project (external dependency)

`foundryProjectEndpoint` must be the complete project endpoint, not the account host:

```
https://<account>.services.ai.azure.com/api/projects/<project>
```

`https` on the default port, host ending in the exact suffix `.services.ai.azure.com`, path
exactly `/api/projects/<project>` with no trailing slash, and no user info, query, or
fragment. Bicep has no regular-expression parameter constraint, so the shape is enforced by
named assertions in `main.bicep` that run during `validate` / `what-if` / `create`, and by
`tools/foundry-smoke/check-prerequisites.ps1` and `tools/foundry-smoke/network-smoke.ps1`,
which apply the identical rule. `_project` is accepted as the documented default-project
alias and must then be paired with an account-level `foundryProjectResourceId`.

`foundryProjectResourceId` is kept only as an account-identity check: its account must match
the account label in `foundryProjectEndpoint`, and the template asserts that. It is no longer a
role-assignment scope -- authentication is a backend-only API key, so this stack never creates,
reads, or depends on any Foundry role assignment.

### Foundry API key: secret, injection, and rotation

`foundryApiKey` is an `@secure()` parameter. **Never** put a real key in a `.bicepparam` file,
an output, a tag, or a plain (non-`secretRef`) environment variable; the committed
`*.example` parameter files carry an obvious placeholder string only. Source the real value
from the deployment pipeline's own secret store (for example a GitHub Actions/Azure DevOps
protected secret) and pass it at `deploy`/`what-if` time.

The template writes the key into a Container Apps secret named `foundry-api-key` and injects it
into the container only as `Sapphirus__FoundryApiKey` via `secretRef` -- it is never a literal
`value` on the container env, never logged, and never emitted as a template output.

Two other, non-secret settings travel with it:

- `Sapphirus__FoundryInventoryMode` (`ConfiguredAndProbed` | `OpenAiModels` |
  `ProjectDeploymentsApiKey`) -- the one explicit, live-qualified inventory mode the broker is
  allowed to use. There is no automatic fallback between modes.
- `Sapphirus__FoundryPrerequisiteEvidenceHash` (`sha256:<hex>`) -- the canonical hash of the
  qualification evidence for the selected mode, produced by
  `tools/foundry-smoke/network-smoke.ps1` / FND-002.

**Rotation is a staged revision change, never a hot-swap.** Update the `foundry-api-key`
secret value, deploy a new (inactive) Container App revision so the old revision keeps running
on the old key, run `tools/foundry-smoke/network-smoke.ps1` against the new revision, shift
traffic, then revoke the old key at the Foundry account. If the new key fails validation, the
old revision is still serving traffic on the still-valid old key, so rollback is simply
declining to shift traffic (see Rollback below).

## Network posture: pilot public egress vs production private egress

The Foundry account is external, so its private endpoint and private DNS zone are owned by
whoever created the Foundry project. Duplicating either here would create a second, competing
resolution for the same host, so this stack creates neither, and no Foundry private-link zone
name or subresource (`groupId`) is guessed anywhere in this repository.

**Pilot: public egress (default).** `foundryPrivateDnsZoneName` and
`foundryPrivateEndpointGroupId` are empty, `foundryEgressPosture` reports `public-egress`, and
the Container App reaches the project over public TLS. Authentication is the backend-only
`Sapphirus__FoundryApiKey` secret described above; there is no identity in this path at all.
This is acceptable only while the pilot is explicitly accepted as such.

**Production: private egress (operator-managed).** Inspect the existing Foundry account and
reference what is actually there:

```powershell
az network private-endpoint list --query "[?contains(to_string(privateLinkServiceConnections[].privateLinkServiceId), '<account>')].{name:name,group:privateLinkServiceConnections[0].groupIds[0]}"
az network private-dns zone list --query "[].name"
```

Supply the two facts you read back as `foundryPrivateDnsZoneName` and
`foundryPrivateEndpointGroupId` (both, or neither — the template asserts they are declared
together), then have the network owner link that existing zone to this VNet and, if the
Foundry account has no endpoint in a reachable VNet yet, create one in the Foundry resource
group. Both actions belong to the resource group that owns the Foundry account, which a
resource-group-scoped template cannot reach.

Verify the result rather than assuming it:

```powershell
pwsh -File tools/foundry-smoke/network-smoke.ps1 `
  -ProjectEndpoint "$env:FOUNDRY_PROJECT_ENDPOINT" `
  -ApiKey "$env:Sapphirus__FoundryApiKey"
```

It writes `artifacts/foundry/infra/network-smoke.json` and exits nonzero unless every stage
passed. `egress.posture` is `private-address-egress` only when every resolved address is
private, which is the observable proof that private egress is actually in effect. The script
proves **API-key reachability**, not identity/role-assignment reachability: it sends the key
only as an `api-key` header to the project's own deployment route from inside the running
revision, reading the key from the container's secret-backed environment variable. The report
carries counts, status classes, and hashes only: never the key, a response body, a deployment
name, or a resolved address.

## Operations and deployment gates

Operational modules: `modules/monitor-alerts.bicep` (seven scheduled-query alerts including a
sev-0 privacy canary rule) and `modules/budget.bicep` (monthly budget with action-group
notifications) deploy when `deployAlerts=true` with an `actionGroupId`. Diagnostic settings for
Key Vault audit and SQL metrics flow into the workspace. Container probes target
`/healthz/live` and `/healthz/ready`, which disclose only status and safe dependency classes.

Azure policy assignments for the target subscription must be inspected before any actual
deployment, and `what-if` output must show no local-auth enablement, public data-plane
endpoint, plaintext secret, mutable image tag, overly broad role, or caller-controlled model
endpoint. The actual deployment (validate/what-if/apply) is an operator step and is
deliberately not run from this repository's local gates.

Validate without deploying:

```powershell
az bicep build --file infra/desktop-support/main.bicep
az deployment group validate --resource-group <resource-group> --parameters infra/desktop-support/main.example.bicepparam
```

Never commit filled parameter files, tenant identifiers, image registry credentials, signing
material, the Foundry API key, or SQL credentials. Managed identity remains the runtime
credential for App Configuration, Key Vault, and SQL; Foundry model access uses only the
backend-only `Sapphirus__FoundryApiKey` secret. The Foundry project endpoint and its ARM
resource ID are operator inputs for the same reason: the `*.example` files carry placeholder
shapes only, and `foundryApiKey`'s placeholder is obviously not a usable key.

## Rollback

1. **Model path only.** Redeploy with the previous parameter file: keep
   `retireLegacyOpenAiAccount=false` (the default) so the legacy Azure OpenAI account and its
   `desktop-planning` deployment are still described, and roll the API back to a digest whose
   image still reads `Sapphirus__ModelEndpoint` / `Sapphirus__ModelDeployment`. The legacy
   account is never deleted by a deployment of this template, so this is a redeploy, not a
   restore.
2. **API key rotation.** Foundry key rotation is a staged revision change (see above): if a
   rotated key fails validation, decline to shift traffic -- the previous revision keeps running
   against the still-valid old key. Only revoke the old key at the Foundry account after the new
   revision is confirmed healthy and traffic has shifted; revoking early breaks the still-serving
   old revision.
3. **Network posture.** To fall back from private to public egress, clear
   `foundryPrivateDnsZoneName` and `foundryPrivateEndpointGroupId` and redeploy; then ask the
   network owner to unlink the private DNS zone from this VNet. Unlink after the redeploy, not
   before, or in-flight requests resolve to an address that is no longer reachable.
4. **Retirement is one-way by intent.** After `retireLegacyOpenAiAccount=true`, the account is
   no longer described by this stack; step 1 would then create a new, empty account and
   deployment, which is not the same resource. Delete the legacy account only after the
   rollback window has passed.
