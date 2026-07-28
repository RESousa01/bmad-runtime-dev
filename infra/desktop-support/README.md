# Desktop support plane infrastructure

This resource-group-scoped Bicep stack describes the D2 Azure support plane. It creates a
VNet-integrated Azure Container Apps environment, user-assigned managed identities, private
Azure Container Registry, Key Vault ES256 signing key, App Configuration, Azure SQL metadata
database, and workspace-based Application Insights. All data services have local
authentication and public network access disabled.

The model plane is **not** created here. It is an external, pre-existing Microsoft Foundry
project reached at `https://<account>.services.ai.azure.com/api/projects/<project>`. This
stack references that project, grants a dedicated identity access to it, and injects the
complete project endpoint into the API. It never creates a Foundry account, a Foundry
project, or a model deployment, and it never names a deployment: the model broker reads the
project's own deployment catalog at runtime.

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
migration identity is never attached to the API, Foundry model access uses a dedicated
model-broker identity (`foundryModelIdentityObjectId`), and data/config and signing share the
runtime identity attached to one process. The module seams (`modules/`) are preserved so
signing can be moved to an independently deployed workload if the threat review requires hard
isolation. `allowSharedPilotIdentity=true` collapses the model-broker identity back onto the
runtime identity; it exists only for a reviewed pilot where a second identity cannot be
created, and it widens the runtime identity's blast radius to include the model plane.

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

`foundryProjectResourceId` is the role-assignment scope: either
`.../providers/Microsoft.CognitiveServices/accounts/<account>` or the narrower
`.../accounts/<account>/projects/<project>`. Its account must match the account label in the
endpoint; the template asserts that.

The data-plane role ID lives in the named variable `foundryDataPlaneRoleDefinitionId` and
carries a deployment-time validation comment, because Foundry role naming is still evolving.
Confirm it still resolves to the intended role before deploying into a new tenant or region,
and change it together with the same ID in `tools/foundry-smoke/check-prerequisites.ps1`.

A resource-group-scoped template can only carry a role assignment whose scope lives in its own
resource group. The `foundryRoleAssignmentMode` output reports which path applied:

- `in-template` — the Foundry resource is in this subscription and resource group, so the
  grant was created here at the narrowest supplied scope (project child when a project was
  supplied, otherwise the account).
- `operator-managed` — the Foundry resource is elsewhere. The grant is then an explicit
  operator step, like the SQL contained-user grants, and is deliberately not hidden in an ARM
  deployment script:

  ```powershell
  az role assignment create `
    --assignee-object-id <foundryModelIdentityObjectId> `
    --assignee-principal-type ServicePrincipal `
    --role <foundryRoleDefinitionId> `
    --scope <foundryRoleAssignmentScope>
  ```

Either way, `tools/foundry-smoke/check-prerequisites.ps1` is what proves the grant exists; the
template's output is not evidence.

## Network posture: pilot public egress vs production private egress

The Foundry account is external, so its private endpoint and private DNS zone are owned by
whoever created the Foundry project. Duplicating either here would create a second, competing
resolution for the same host, so this stack creates neither, and no Foundry private-link zone
name or subresource (`groupId`) is guessed anywhere in this repository.

**Pilot: public egress (default).** `foundryPrivateDnsZoneName` and
`foundryPrivateEndpointGroupId` are empty, `foundryEgressPosture` reports `public-egress`, and
the Container App reaches the project over public TLS. Authentication is still managed
identity only. This is acceptable only while the pilot is explicitly accepted as such.

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
pwsh -File tools/foundry-smoke/network-smoke.ps1 -ProjectEndpoint "$env:FOUNDRY_PROJECT_ENDPOINT"
```

It writes `artifacts/foundry/infra/network-smoke.json` and exits nonzero unless every stage
passed. `egress.posture` is `private-address-egress` only when every resolved address is
private, which is the observable proof that private egress is actually in effect. The report
carries counts, status classes, and hashes only: never a token, a response body, a deployment
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
material, model credentials, or SQL credentials. Managed identity is the only runtime credential.
The Foundry project endpoint and its ARM resource ID are operator inputs for the same reason:
the `*.example` files carry placeholder shapes only.

## Rollback

1. **Model path only.** Redeploy with the previous parameter file: keep
   `retireLegacyOpenAiAccount=false` (the default) so the legacy Azure OpenAI account and its
   `desktop-planning` deployment are still described, and roll the API back to a digest whose
   image still reads `Sapphirus__ModelEndpoint` / `Sapphirus__ModelDeployment`. The legacy
   account is never deleted by a deployment of this template, so this is a redeploy, not a
   restore.
2. **Identity.** Leave the model-broker identity and its role assignment in place during
   rollback. They grant nothing to the legacy path, and removing them costs a second
   role-assignment convergence when you roll forward again. Remove them only when the
   migration is abandoned: delete the role assignment first, the identity second.
3. **Network posture.** To fall back from private to public egress, clear
   `foundryPrivateDnsZoneName` and `foundryPrivateEndpointGroupId` and redeploy; then ask the
   network owner to unlink the private DNS zone from this VNet. Unlink after the redeploy, not
   before, or in-flight requests resolve to an address that is no longer reachable.
4. **Retirement is one-way by intent.** After `retireLegacyOpenAiAccount=true`, the account is
   no longer described by this stack; step 1 would then create a new, empty account and
   deployment, which is not the same resource. Delete the legacy account only after the
   rollback window has passed.
