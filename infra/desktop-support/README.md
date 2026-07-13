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
user for the emitted `apiManagedIdentityObjectId` and grant only the migration/runtime roles
defined by the reviewed SQL migration. That data-plane grant is intentionally not hidden in an
ARM deployment script. The production API adapters remain a D2 work packet; the repository's
in-memory adapters refuse production signing and model access.

Validate without deploying:

```powershell
az bicep build --file infra/desktop-support/main.bicep
az deployment group validate --resource-group <resource-group> --parameters infra/desktop-support/main.example.bicepparam
```

Never commit filled parameter files, tenant identifiers, image registry credentials, signing
material, model credentials, or SQL credentials. Managed identity is the only runtime credential.
