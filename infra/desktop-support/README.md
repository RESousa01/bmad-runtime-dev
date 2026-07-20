# Desktop support plane infrastructure

This resource-group-scoped Bicep stack describes the D2 Azure support plane. It creates a
VNet-integrated Azure Container Apps environment, a user-assigned managed identity, private
Azure Container Registry, Key Vault ES256 signing key, App Configuration, Azure SQL metadata
database, Azure OpenAI account and pinned deployment, and workspace-based Application Insights.
All data services have local authentication and public network access disabled.

This is an internal, single-tenant deployment. The API itself has public TLS ingress so managed
employee devices can reach it, but tokens are accepted only for the configured organization tenant
and audience; there is no public registration or multi-tenant onboarding path.

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

Identity split in this first deployment is logical least privilege: image pull uses its own
identity (`imagePullIdentityObjectId`), the SQL migration identity is never attached to the API,
and data/config, signing, and model access share the runtime identity attached to one process.
The module seams (`modules/`) are preserved so signing and model access can be moved to
independently deployed workloads if the threat review requires hard isolation.

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
