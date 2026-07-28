using './main.bicep'

param namePrefix = 'sapphirus-int'
param deployApi = false
param entraAuthority = 'https://login.microsoftonline.com/00000000-0000-0000-0000-000000000000/v2.0'
param entraAudience = 'api://00000000-0000-0000-0000-000000000000'
param approvedDesktopClientId = '33333333-3333-3333-3333-333333333333'
param sqlAdministratorObjectId = '00000000-0000-0000-0000-000000000000'
param sqlAdministratorLogin = 'Sapphirus SQL Administrators'
// Legacy Azure OpenAI pin. Still required while retireLegacyOpenAiAccount is
// false; drop both once the legacy account is retired.
param modelName = 'gpt-5'
param modelVersion = 'replace-with-reviewed-version'
param retireLegacyOpenAiAccount = false
param releaseChannel = 'beta'
// External Microsoft Foundry project. Placeholder shape only: substitute the
// real account and project at deployment time and never commit them. The
// project name is a project name, not a deployment name; this repository knows
// no Foundry deployment name.
param foundryProjectEndpoint = 'https://replace-with-account.services.ai.azure.com/api/projects/replace-with-project'
param foundryProjectResourceId = '/subscriptions/00000000-0000-0000-0000-000000000000/resourceGroups/replace-with-foundry-resource-group/providers/Microsoft.CognitiveServices/accounts/replace-with-account/projects/replace-with-project'
param modelProfileConfigurationPrefix = 'desktop-support:model-profiles:'
param foundryDeploymentRefreshSeconds = 300
param foundryCatalogMaximumStaleSeconds = 1800
param allowSharedPilotIdentity = false
// Pilot posture: public egress. For private egress, inspect the EXISTING
// Foundry private endpoint and DNS zone and paste their facts here; this stack
// never creates or duplicates them. See README.md.
param foundryPrivateDnsZoneName = ''
param foundryPrivateEndpointGroupId = ''
param providerProfileHash = 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
param modelProfileHash = 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
param modelCapabilityHash = 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
param deployAlerts = false
param tags = {
  application: 'sapphirus'
  distribution: 'internal'
}
