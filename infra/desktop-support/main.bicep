targetScope = 'resourceGroup'

@description('Short, lowercase deployment name used to build globally unique resource names.')
@minLength(3)
@maxLength(18)
param namePrefix string

@description('Azure region in which the desktop support plane is deployed.')
param location string = resourceGroup().location

@description('Deploy the support API after the infrastructure image has been published.')
param deployApi bool = false

@description('Immutable support API container image including its sha256 digest. Required when deployApi is true.')
param containerImage string = ''

@description('Microsoft Entra tenant-specific API authority. The common authority is not accepted.')
param entraAuthority string

@description('Microsoft Entra application ID URI expected in desktop access tokens.')
param entraAudience string

@description('Application (client) ID of the reviewed desktop public client allowed in delegated access tokens.')
@minLength(36)
@maxLength(36)
param approvedDesktopClientId string

@description('Object ID of the Entra group that administers the Azure SQL logical server.')
param sqlAdministratorObjectId string

@description('Display name of the Entra group that administers the Azure SQL logical server.')
param sqlAdministratorLogin string

@description('Legacy Azure OpenAI model name. Required only while the legacy account is retained.')
param modelName string = ''

@description('Pinned legacy Azure OpenAI model version. Required only while the legacy account is retained.')
param modelVersion string = ''

// The Foundry project is an EXTERNAL, pre-existing resource. This stack never
// creates a Foundry account, project, or model deployment, and it never invents
// a deployment name: the model broker reads the project's own deployment
// catalog at runtime.
@description('''Complete Microsoft Foundry project endpoint. Required shape:
https://<account>.services.ai.azure.com/api/projects/<project>
It must be https on the default port, the host must end with the exact suffix
`.services.ai.azure.com`, the path must be exactly `/api/projects/<project>`
with no trailing slash, and it must carry no user info, query, or fragment.
The account host label and the account named by foundryProjectResourceId must
agree. Bicep has no regular-expression parameter constraint, so the shape is
enforced by the deployment-time assertions below.''')
@minLength(46)
@maxLength(512)
param foundryProjectEndpoint string

@description('''ARM resource ID used as the Foundry role-assignment scope. Either
/subscriptions/<sub>/resourceGroups/<rg>/providers/Microsoft.CognitiveServices/accounts/<account>
or the narrower
.../accounts/<account>/projects/<project>. It must name the same account as
foundryProjectEndpoint.''')
@minLength(64)
@maxLength(512)
param foundryProjectResourceId string

@description('Name of the dedicated user-assigned identity used only for Foundry model access.')
@minLength(3)
@maxLength(60)
param modelBrokerIdentityName string = '${namePrefix}-foundry-id'

// Must stay equal to ProductionOptions.ModelProfileConfigurationPrefix in
// services/desktop-support-api, which accepts only lowercase colon-separated
// segments ending in a separator.
@description('App Configuration key prefix under which reviewed model profiles are published. Lowercase, colon-separated, ending with the separator, and never a wildcard.')
@minLength(3)
@maxLength(128)
param modelProfileConfigurationPrefix string = 'desktop-support:model-profiles:'

@description('Seconds between Foundry deployment-catalog refreshes.')
@minValue(30)
@maxValue(3600)
param foundryDeploymentRefreshSeconds int = 300

@description('Maximum age of a cached Foundry deployment catalog before the broker must fail closed. Must be at least the refresh interval.')
@minValue(30)
@maxValue(86400)
param foundryCatalogMaximumStaleSeconds int = 1800

@description('Pilot-only escape hatch: reuse the shared runtime identity for Foundry model access instead of creating a dedicated model-broker identity. Leave false outside a reviewed pilot.')
param allowSharedPilotIdentity bool = false

@description('Retire the legacy fixed Azure OpenAI account and its desktop-planning deployment. Left false so that migrating an existing deployment never silently deletes the legacy account; flipping it to true removes the resource from this stack and the operator then deletes it deliberately.')
param retireLegacyOpenAiAccount bool = false

@description('Name of the EXISTING private DNS zone that already fronts the Foundry account, for example the zone reported by the operator network team. Empty means public egress or externally managed egress. This stack never creates or duplicates a Foundry zone or private endpoint.')
@maxLength(253)
param foundryPrivateDnsZoneName string = ''

@description('Private endpoint group ID (subresource) of the EXISTING Foundry private endpoint, read from the operator-owned endpoint. Empty means public egress or externally managed egress.')
@maxLength(64)
param foundryPrivateEndpointGroupId string = ''

@allowed([
  'beta'
  'stable'
])
param releaseChannel string = 'beta'

@description('Deployment-wide resource tags.')
param tags object = {}

@description('Canonical provider profile hash (sha256:<hex>).')
param providerProfileHash string

@description('Canonical model profile hash (sha256:<hex>).')
param modelProfileHash string

@description('Canonical model capability hash (sha256:<hex>).')
param modelCapabilityHash string

@description('Deploy scheduled-query alerts and the monthly budget.')
param deployAlerts bool = false

@description('Action group resource id for alerts; required when deployAlerts is true.')
param actionGroupId string = ''

@description('Monthly budget amount in the subscription currency.')
param monthlyBudgetAmount int = 200

@description('Environment tag value used by alerting and budgets.')
param environmentTag string = 'staging'

assert immutableApiImage = !deployApi || (contains(containerImage, '@sha256:') && length(containerImage) >= 80)

// Foundry endpoint shape. Bicep offers no @pattern constraint, so the exact
// shape documented on foundryProjectEndpoint is proved by assertions that run
// at deployment time (validate / what-if / create). The segment arrays are
// padded so that a malformed value fails an assertion instead of failing an
// out-of-range index first.
// The scheme is removed by fixed offset rather than by replace(), so that a
// second 'https://' inside the value cannot be silently folded away. The
// @minLength constraint above is validated before this expression runs.
var foundryEndpointAuthorityAndPath = substring(foundryProjectEndpoint, 8, length(foundryProjectEndpoint) - 8)
var foundryEndpointSegments = split(foundryEndpointAuthorityAndPath, '/')
var foundryEndpointPaddedSegments = concat(foundryEndpointSegments, ['', '', '', ''])
var foundryEndpointAuthority = foundryEndpointPaddedSegments[0]
var foundryEndpointProjectName = foundryEndpointPaddedSegments[3]
var foundryEndpointAccountLabel = split(foundryEndpointAuthority, '.')[0]
var foundryHostSuffix = '.services.ai.azure.com'

assert foundryEndpointUsesHttps = startsWith(foundryProjectEndpoint, 'https://')
assert foundryEndpointHasNoUserInfoOrPort = !contains(foundryEndpointAuthority, '@') && !contains(foundryEndpointAuthority, ':')
assert foundryEndpointHasNoQueryOrFragment = !contains(foundryProjectEndpoint, '?') && !contains(foundryProjectEndpoint, '#')
assert foundryEndpointHostIsFoundryAccount = endsWith(foundryEndpointAuthority, foundryHostSuffix) && length(foundryEndpointAuthority) > length(foundryHostSuffix)
// The service accepts only a lowercase ASCII host, so that a homograph or
// mixed-case spelling can never satisfy the trusted suffix. Reject it here
// rather than deploying a container that cannot bind its own configuration.
assert foundryEndpointHostIsLowercase = foundryEndpointAuthority == toLower(foundryEndpointAuthority)
assert foundryEndpointPathIsCompleteProject = length(foundryEndpointSegments) == 4 && foundryEndpointPaddedSegments[1] == 'api' && foundryEndpointPaddedSegments[2] == 'projects' && length(foundryEndpointProjectName) > 0

// Foundry role-assignment scope. The account named by the scope and the account
// label in the data-plane endpoint must be the same account, otherwise the
// grant proves nothing about the endpoint the broker actually calls.
var foundryScopeSegments = split(foundryProjectResourceId, '/')
var foundryScopePaddedSegments = concat(foundryScopeSegments, ['', '', '', '', '', '', '', '', '', '', ''])
var foundryScopeSubscriptionId = foundryScopePaddedSegments[2]
var foundryScopeResourceGroupName = foundryScopePaddedSegments[4]
var foundryScopeAccountName = foundryScopePaddedSegments[8]
var foundryScopeProjectName = foundryScopePaddedSegments[10]

assert foundryScopeIsCognitiveServicesAccount = startsWith(foundryProjectResourceId, '/subscriptions/') && foundryScopePaddedSegments[3] == 'resourceGroups' && foundryScopePaddedSegments[5] == 'providers' && toLower(foundryScopePaddedSegments[6]) == 'microsoft.cognitiveservices' && toLower(foundryScopePaddedSegments[7]) == 'accounts' && length(foundryScopeAccountName) > 0
assert foundryScopeIsAccountOrProject = length(foundryScopeSegments) == 9 || (length(foundryScopeSegments) == 11 && toLower(foundryScopePaddedSegments[9]) == 'projects' && length(foundryScopeProjectName) > 0)
assert foundryScopeMatchesEndpointAccount = toLower(foundryScopeAccountName) == toLower(foundryEndpointAccountLabel)
// The `_project` alias resolves to the account's default project, so it cannot
// be bound to a named project scope. This mirrors the checker in
// tools/foundry-smoke/check-prerequisites.ps1.
assert foundryScopeMatchesEndpointProject = foundryEndpointProjectName == '_project' ? empty(foundryScopeProjectName) : (empty(foundryScopeProjectName) || foundryScopeProjectName == foundryEndpointProjectName)

// Catalog freshness. A cache that may be served older than it is refreshed can
// never fail closed on a stale catalog. Same rule as ProductionOptions.
assert foundryCatalogStalenessCoversRefresh = foundryCatalogMaximumStaleSeconds >= foundryDeploymentRefreshSeconds

// The profile prefix becomes an App Configuration key filter. A wildcard or a
// missing separator would widen the read beyond the reviewed profile keys.
assert modelProfilePrefixIsBounded = endsWith(modelProfileConfigurationPrefix, ':') && !contains(modelProfileConfigurationPrefix, '*') && !contains(modelProfileConfigurationPrefix, ' ') && modelProfileConfigurationPrefix == toLower(modelProfileConfigurationPrefix)

// The shared-identity escape hatch is a pilot concession, and the service
// rejects it outside the pilot release channel. Fail at deployment time
// instead of at container start.
assert sharedPilotIdentityIsPilotOnly = !allowSharedPilotIdentity || releaseChannel == 'beta'

// Private egress is declared as a whole or not at all: half of the operator's
// existing private-link facts cannot be acted on or verified.
assert foundryPrivateEgressIsCompleteOrAbsent = empty(foundryPrivateDnsZoneName) == empty(foundryPrivateEndpointGroupId)
assert foundryPrivateDnsZoneIsPrivateLinkZone = empty(foundryPrivateDnsZoneName) || startsWith(foundryPrivateDnsZoneName, 'privatelink.')

// The legacy model pin is only meaningful while the legacy account is retained.
assert legacyModelPinPresentWhenRetained = retireLegacyOpenAiAccount || (!empty(modelName) && !empty(modelVersion))

var suffix = uniqueString(subscription().id, resourceGroup().id, namePrefix)
var compactPrefix = toLower(replace(namePrefix, '-', ''))
var identityName = '${namePrefix}-api-id'
var environmentName = '${namePrefix}-env'
var apiName = '${namePrefix}-api'
var registryName = take('${compactPrefix}${suffix}cr', 50)
var vaultName = take('${compactPrefix}-${suffix}-kv', 24)
var configurationName = take('${compactPrefix}-${suffix}-cfg', 50)
var sqlServerName = take('${compactPrefix}-${suffix}-sql', 63)
var openAiName = take('${compactPrefix}-${suffix}-aoai', 64)
var workspaceName = '${namePrefix}-logs'
var insightsName = '${namePrefix}-insights'
var networkName = '${namePrefix}-vnet'
var signingKeyName = 'desktop-policy-es256'

var subnets = {
  containerApps: '10.42.0.0/23'
  privateEndpoints: '10.42.2.0/24'
}

var roleIds = {
  acrPull: '7f951dda-4ed3-4680-a7ca-43fe172d538d'
  appConfigurationDataReader: '516239f1-63e1-4d78-a4de-a74fb236a071'
  cognitiveServicesOpenAiUser: '5e0bd9bd-7b93-4f28-af87-19fc36ad61bd'
  keyVaultCryptoUser: '12338af0-0e69-4776-bea7-57ae8d297424'
}

// Foundry data-plane role. DEPLOYMENT-TIME VALIDATION REQUIRED: Foundry role
// naming is still evolving, so this ID is deliberately a named variable rather
// than an inline GUID. Before every deployment into a new tenant or region,
// confirm it still resolves to the intended data-plane role with
//   az role definition list --name <role> --query "[].{name:roleName,id:name}"
// and confirm the granted dataActions still cover only project inference. The
// same ID is the one tools/foundry-smoke/check-prerequisites.ps1 verifies, so
// the two must be changed together.
var foundryDataPlaneRoleDefinitionId = '53ca6127-db72-4b80-b1b0-d745d6d5456d'

// Narrowest working scope: the project child when the operator supplied one,
// otherwise the account. A resourceGroup-scoped template can only carry an
// extension resource whose scope lives in this resource group, so the grant is
// emitted here only when the Foundry resource is co-located. Otherwise the
// grant stays an explicit operator step, like the SQL contained-user grants,
// and foundryRoleAssignmentMode reports which path applies.
var foundryScopeIsInThisResourceGroup = toLower(foundryScopeSubscriptionId) == toLower(subscription().subscriptionId) && toLower(foundryScopeResourceGroupName) == toLower(resourceGroup().name)
var foundryScopeIsProject = !empty(foundryScopeProjectName)
var assignFoundryRoleHere = foundryScopeIsInThisResourceGroup
var foundryEgressPosture = empty(foundryPrivateDnsZoneName) ? 'public-egress' : 'operator-managed-private-egress'

resource network 'Microsoft.Network/virtualNetworks@2024-05-01' = {
  name: networkName
  location: location
  tags: tags
  properties: {
    addressSpace: {
      addressPrefixes: [
        '10.42.0.0/16'
      ]
    }
    subnets: [
      {
        name: 'container-apps'
        properties: {
          addressPrefix: subnets.containerApps
          delegations: [
            {
              name: 'Microsoft.App.environments'
              properties: {
                serviceName: 'Microsoft.App/environments'
              }
            }
          ]
        }
      }
      {
        name: 'private-endpoints'
        properties: {
          addressPrefix: subnets.privateEndpoints
          privateEndpointNetworkPolicies: 'Disabled'
        }
      }
    ]
  }
}

resource containerAppsSubnet 'Microsoft.Network/virtualNetworks/subnets@2024-05-01' existing = {
  parent: network
  name: 'container-apps'
}

resource privateEndpointsSubnet 'Microsoft.Network/virtualNetworks/subnets@2024-05-01' existing = {
  parent: network
  name: 'private-endpoints'
}

// Identity split (first deployment = logical least privilege): image pull
// is isolated from the runtime identity; data/config and signing share the
// runtime identity attached to one process, with module seams preserved for
// an independently deployed signing workload if the threat review requires
// hard isolation later. The migration identity holds schema (DDL) rights only
// and is never attached to the API. Foundry model access has its own dedicated
// identity so that a model-plane grant never widens the runtime identity.
resource imagePullIdentity 'Microsoft.ManagedIdentity/userAssignedIdentities@2023-01-31' = {
  name: '${namePrefix}-pull-id'
  location: location
  tags: tags
}

resource migrationIdentity 'Microsoft.ManagedIdentity/userAssignedIdentities@2023-01-31' = {
  name: '${namePrefix}-sqlmigrate-id'
  location: location
  tags: tags
}

resource identity 'Microsoft.ManagedIdentity/userAssignedIdentities@2023-01-31' = {
  name: identityName
  location: location
  tags: tags
}

// Dedicated model-broker identity. It is the only principal that ever receives
// the Foundry data-plane role, and it is attached to the API container in
// addition to (never instead of) the runtime and image-pull identities.
resource modelBrokerIdentity 'Microsoft.ManagedIdentity/userAssignedIdentities@2023-01-31' = if (!allowSharedPilotIdentity) {
  name: modelBrokerIdentityName
  location: location
  tags: tags
}

var foundryIdentityResourceId = allowSharedPilotIdentity ? identity.id : modelBrokerIdentity!.id
var foundryIdentityClientId = allowSharedPilotIdentity ? identity.properties.clientId : modelBrokerIdentity!.properties.clientId
var foundryIdentityPrincipalId = allowSharedPilotIdentity ? identity.properties.principalId : modelBrokerIdentity!.properties.principalId
var foundryIdentityName = allowSharedPilotIdentity ? identity.name : modelBrokerIdentityName

resource logWorkspace 'Microsoft.OperationalInsights/workspaces@2023-09-01' = {
  name: workspaceName
  location: location
  tags: tags
  properties: {
    retentionInDays: 30
    publicNetworkAccessForIngestion: 'Enabled'
    publicNetworkAccessForQuery: 'Enabled'
    sku: {
      name: 'PerGB2018'
    }
  }
}

resource applicationInsights 'Microsoft.Insights/components@2020-02-02' = {
  name: insightsName
  location: location
  kind: 'web'
  tags: tags
  properties: {
    Application_Type: 'web'
    WorkspaceResourceId: logWorkspace.id
    DisableLocalAuth: true
    IngestionMode: 'LogAnalytics'
    publicNetworkAccessForIngestion: 'Enabled'
    publicNetworkAccessForQuery: 'Enabled'
  }
}

resource registry 'Microsoft.ContainerRegistry/registries@2025-04-01' = {
  name: registryName
  location: location
  tags: tags
  sku: {
    name: 'Premium'
  }
  properties: {
    adminUserEnabled: false
    anonymousPullEnabled: false
    dataEndpointEnabled: false
    publicNetworkAccess: 'Disabled'
    zoneRedundancy: 'Disabled'
  }
}

resource vault 'Microsoft.KeyVault/vaults@2024-11-01' = {
  name: vaultName
  location: location
  tags: tags
  properties: {
    tenantId: subscription().tenantId
    enableRbacAuthorization: true
    enablePurgeProtection: true
    enableSoftDelete: true
    softDeleteRetentionInDays: 90
    publicNetworkAccess: 'Disabled'
    sku: {
      family: 'A'
      name: 'standard'
    }
  }
}

resource signingKey 'Microsoft.KeyVault/vaults/keys@2024-11-01' = {
  parent: vault
  name: signingKeyName
  properties: {
    attributes: {
      enabled: true
      exportable: false
    }
    curveName: 'P-256'
    keyOps: [
      'sign'
      'verify'
    ]
    kty: 'EC'
  }
}

resource configuration 'Microsoft.AppConfiguration/configurationStores@2024-06-01' = {
  name: configurationName
  location: location
  tags: tags
  sku: {
    name: 'standard'
  }
  properties: {
    disableLocalAuth: true
    enablePurgeProtection: true
    publicNetworkAccess: 'Disabled'
    softDeleteRetentionInDays: 7
  }
}

resource sqlServer 'Microsoft.Sql/servers@2023-08-01-preview' = {
  name: sqlServerName
  location: location
  tags: tags
  properties: {
    administrators: {
      administratorType: 'ActiveDirectory'
      azureADOnlyAuthentication: true
      login: sqlAdministratorLogin
      principalType: 'Group'
      sid: sqlAdministratorObjectId
      tenantId: subscription().tenantId
    }
    minimalTlsVersion: '1.2'
    publicNetworkAccess: 'Disabled'
    restrictOutboundNetworkAccess: 'Enabled'
    version: '12.0'
  }
}

resource sqlDatabase 'Microsoft.Sql/servers/databases@2023-08-01-preview' = {
  parent: sqlServer
  name: 'desktop-support'
  location: location
  tags: tags
  sku: {
    name: 'GP_S_Gen5_1'
    tier: 'GeneralPurpose'
    family: 'Gen5'
    capacity: 1
  }
  properties: {
    autoPauseDelay: 60
    minCapacity: json('0.5')
    readScale: 'Disabled'
    requestedBackupStorageRedundancy: 'Geo'
    zoneRedundant: false
  }
}

// LEGACY. The model authority moved to the external Foundry project. This
// account and its fixed desktop-planning deployment are kept only so that
// migrating an existing deployment does not delete a live resource as a side
// effect. Set retireLegacyOpenAiAccount=true once traffic has moved and the
// operator has decided to remove it; the account is then no longer described by
// this stack and is deleted deliberately, not implicitly.
resource openAi 'Microsoft.CognitiveServices/accounts@2025-12-01' = if (!retireLegacyOpenAiAccount) {
  name: openAiName
  location: location
  tags: tags
  kind: 'OpenAI'
  sku: {
    name: 'S0'
  }
  identity: {
    type: 'SystemAssigned'
  }
  properties: {
    customSubDomainName: openAiName
    disableLocalAuth: true
    dynamicThrottlingEnabled: true
    publicNetworkAccess: 'Disabled'
    networkAcls: {
      defaultAction: 'Deny'
    }
  }
}

resource modelDeployment 'Microsoft.CognitiveServices/accounts/deployments@2024-10-01' = if (!retireLegacyOpenAiAccount) {
  parent: openAi
  name: 'desktop-planning'
  sku: {
    name: 'Standard'
    capacity: 10
  }
  properties: {
    model: {
      format: 'OpenAI'
      name: modelName
      version: modelVersion
    }
    raiPolicyName: 'Microsoft.DefaultV2'
    versionUpgradeOption: 'NoAutoUpgrade'
  }
}

resource environment 'Microsoft.App/managedEnvironments@2026-01-01' = {
  name: environmentName
  location: location
  tags: tags
  properties: {
    appLogsConfiguration: {
      destination: 'azure-monitor'
    }
    vnetConfiguration: {
      infrastructureSubnetId: containerAppsSubnet.id
    }
    publicNetworkAccess: 'Enabled'
    zoneRedundant: false
  }
}

resource api 'Microsoft.App/containerApps@2026-01-01' = if (deployApi) {
  name: apiName
  location: location
  tags: tags
  identity: {
    type: 'UserAssigned'
    userAssignedIdentities: union(
      {
        '${identity.id}': {}
        '${imagePullIdentity.id}': {}
      },
      allowSharedPilotIdentity ? {} : {
        '${modelBrokerIdentity!.id}': {}
      }
    )
  }
  properties: {
    environmentId: environment.id
    configuration: {
      activeRevisionsMode: 'Single'
      ingress: {
        allowInsecure: false
        external: true
        targetPort: 8080
        transport: 'http2'
      }
      registries: [
        {
          identity: imagePullIdentity.id
          server: registry.properties.loginServer
        }
      ]
    }
    template: {
      containers: [
        {
          name: 'desktop-support-api'
          image: containerImage
          env: [
            { name: 'ASPNETCORE_HTTP_PORTS', value: '8080' }
            { name: 'ASPNETCORE_ENVIRONMENT', value: 'Production' }
            { name: 'Sapphirus__Authority', value: entraAuthority }
            { name: 'Sapphirus__Audience', value: entraAudience }
            { name: 'Sapphirus__ApprovedDesktopClientId', value: approvedDesktopClientId }
            { name: 'Sapphirus__Region', value: location }
            { name: 'Sapphirus__ReleaseChannel', value: releaseChannel }
            { name: 'Sapphirus__ManagedIdentityClientId', value: identity.properties.clientId }
            { name: 'Sapphirus__AppConfigurationEndpoint', value: configuration.properties.endpoint }
            { name: 'Sapphirus__KeyVaultUri', value: vault.properties.vaultUri }
            { name: 'Sapphirus__ReceiptSigningKeyName', value: signingKey.name }
            { name: 'Sapphirus__ProviderProfileHash', value: providerProfileHash }
            { name: 'Sapphirus__ModelProfileHash', value: modelProfileHash }
            { name: 'Sapphirus__ModelCapabilityHash', value: modelCapabilityHash }
            { name: 'Sapphirus__SqlServer', value: sqlServer.properties.fullyQualifiedDomainName }
            { name: 'Sapphirus__SqlDatabase', value: sqlDatabase.name }
            // Model access is the complete external project endpoint plus the
            // identity allowed to call it. No endpoint host, deployment name, or
            // deployment hash is pinned here: the broker reads the project's own
            // catalog, and no deployment name is known to this repository.
            { name: 'Sapphirus__FoundryProjectEndpoint', value: foundryProjectEndpoint }
            { name: 'Sapphirus__FoundryModelIdentityClientId', value: foundryIdentityClientId }
            // Required for the shared-identity pilot to be accepted at all: the
            // service refuses a Foundry identity equal to the runtime identity
            // unless this opt-in is present on the pilot channel.
            { name: 'Sapphirus__AllowSharedPilotIdentity', value: string(allowSharedPilotIdentity) }
            { name: 'Sapphirus__FoundryDeploymentRefreshSeconds', value: string(foundryDeploymentRefreshSeconds) }
            { name: 'Sapphirus__FoundryCatalogMaximumStaleSeconds', value: string(foundryCatalogMaximumStaleSeconds) }
            { name: 'Sapphirus__ModelProfileConfigurationPrefix', value: modelProfileConfigurationPrefix }
            { name: 'APPLICATIONINSIGHTS_CONNECTION_STRING', value: applicationInsights.properties.ConnectionString }
          ]
          resources: {
            cpu: json('0.5')
            memory: '1Gi'
          }
          probes: [
            {
              type: 'Startup'
              httpGet: { path: '/healthz/live', port: 8080 }
              initialDelaySeconds: 5
              periodSeconds: 5
              failureThreshold: 12
            }
            {
              type: 'Liveness'
              httpGet: { path: '/healthz/live', port: 8080 }
              periodSeconds: 15
              failureThreshold: 3
            }
            {
              type: 'Readiness'
              httpGet: { path: '/healthz/ready', port: 8080 }
              periodSeconds: 30
              failureThreshold: 3
            }
          ]
        }
      ]
      scale: {
        minReplicas: 1
        maxReplicas: 10
        rules: [
          {
            name: 'http-concurrency'
            http: {
              metadata: {
                concurrentRequests: '50'
              }
            }
          }
        ]
      }
    }
  }
  dependsOn: [
    acrPull
    appConfigurationReader
    signingRole
    modelUser
    foundryAccountModelUser
    foundryProjectModelUser
    privateDnsZoneGroups
  ]
}

resource acrPull 'Microsoft.Authorization/roleAssignments@2022-04-01' = {
  name: guid(registry.id, imagePullIdentity.id, roleIds.acrPull)
  scope: registry
  properties: {
    principalId: imagePullIdentity.properties.principalId
    principalType: 'ServicePrincipal'
    roleDefinitionId: subscriptionResourceId('Microsoft.Authorization/roleDefinitions', roleIds.acrPull)
  }
}

resource appConfigurationReader 'Microsoft.Authorization/roleAssignments@2022-04-01' = {
  name: guid(configuration.id, identity.id, roleIds.appConfigurationDataReader)
  scope: configuration
  properties: {
    principalId: identity.properties.principalId
    principalType: 'ServicePrincipal'
    roleDefinitionId: subscriptionResourceId('Microsoft.Authorization/roleDefinitions', roleIds.appConfigurationDataReader)
  }
}

resource signingRole 'Microsoft.Authorization/roleAssignments@2022-04-01' = {
  name: guid(vault.id, identity.id, roleIds.keyVaultCryptoUser)
  scope: vault
  properties: {
    principalId: identity.properties.principalId
    principalType: 'ServicePrincipal'
    roleDefinitionId: subscriptionResourceId('Microsoft.Authorization/roleDefinitions', roleIds.keyVaultCryptoUser)
  }
}

// LEGACY grant on the retiring account. It disappears with the account.
resource modelUser 'Microsoft.Authorization/roleAssignments@2022-04-01' = if (!retireLegacyOpenAiAccount) {
  name: guid(openAi!.id, identity.id, roleIds.cognitiveServicesOpenAiUser)
  scope: openAi
  properties: {
    principalId: identity.properties.principalId
    principalType: 'ServicePrincipal'
    roleDefinitionId: subscriptionResourceId('Microsoft.Authorization/roleDefinitions', roleIds.cognitiveServicesOpenAiUser)
  }
}

// The external Foundry account/project referenced only as a role scope. These
// are `existing` declarations: this stack never creates, configures, or deletes
// the Foundry resource.
resource foundryAccount 'Microsoft.CognitiveServices/accounts@2025-12-01' existing = {
  name: foundryScopeAccountName
}

resource foundryProject 'Microsoft.CognitiveServices/accounts/projects@2025-12-01' existing = {
  parent: foundryAccount
  name: foundryScopeIsProject ? foundryScopeProjectName : 'unbound'
}

resource foundryProjectModelUser 'Microsoft.Authorization/roleAssignments@2022-04-01' = if (assignFoundryRoleHere && foundryScopeIsProject) {
  name: guid(foundryProject.id, foundryIdentityResourceId, foundryDataPlaneRoleDefinitionId)
  scope: foundryProject
  properties: {
    principalId: foundryIdentityPrincipalId
    principalType: 'ServicePrincipal'
    roleDefinitionId: subscriptionResourceId('Microsoft.Authorization/roleDefinitions', foundryDataPlaneRoleDefinitionId)
  }
}

resource foundryAccountModelUser 'Microsoft.Authorization/roleAssignments@2022-04-01' = if (assignFoundryRoleHere && !foundryScopeIsProject) {
  name: guid(foundryAccount.id, foundryIdentityResourceId, foundryDataPlaneRoleDefinitionId)
  scope: foundryAccount
  properties: {
    principalId: foundryIdentityPrincipalId
    principalType: 'ServicePrincipal'
    roleDefinitionId: subscriptionResourceId('Microsoft.Authorization/roleDefinitions', foundryDataPlaneRoleDefinitionId)
  }
}

// Private link is described only for resources this stack owns. The Foundry
// account is external: its private endpoint and private DNS zone are owned by
// whoever created the Foundry project, and duplicating either here would
// create a second, competing name resolution for the same host. The Foundry
// zone name and group ID are therefore inputs to be inspected and referenced
// (see README.md), not resources to be invented. No Foundry private-link zone
// or subresource name is guessed anywhere in this stack.
var ownedPrivateLinkTargets = [
  { name: 'registry', resourceId: registry.id, groupId: 'registry', zone: 'privatelink.azurecr.io' }
  { name: 'vault', resourceId: vault.id, groupId: 'vault', zone: 'privatelink.vaultcore.azure.net' }
  { name: 'configuration', resourceId: configuration.id, groupId: 'configurationStores', zone: 'privatelink.azconfig.io' }
  { name: 'sql', resourceId: sqlServer.id, groupId: 'sqlServer', zone: 'privatelink${az.environment().suffixes.sqlServerHostname}' }
]

var legacyPrivateLinkTargets = retireLegacyOpenAiAccount ? [] : [
  { name: 'openai', resourceId: openAi!.id, groupId: 'account', zone: 'privatelink.openai.azure.com' }
]

var privateLinkTargets = concat(ownedPrivateLinkTargets, legacyPrivateLinkTargets)

resource privateDnsZones 'Microsoft.Network/privateDnsZones@2024-06-01' = [for target in privateLinkTargets: {
  name: target.zone
  location: 'global'
  tags: tags
}]

resource privateDnsLinks 'Microsoft.Network/privateDnsZones/virtualNetworkLinks@2024-06-01' = [for (target, index) in privateLinkTargets: {
  parent: privateDnsZones[index]
  name: '${namePrefix}-${target.name}-link'
  location: 'global'
  properties: {
    registrationEnabled: false
    virtualNetwork: {
      id: network.id
    }
  }
}]

resource privateEndpoints 'Microsoft.Network/privateEndpoints@2024-05-01' = [for target in privateLinkTargets: {
  name: '${namePrefix}-${target.name}-pe'
  location: location
  tags: tags
  properties: {
    subnet: {
      id: privateEndpointsSubnet.id
    }
    privateLinkServiceConnections: [
      {
        name: '${target.name}-connection'
        properties: {
          groupIds: [
            target.groupId
          ]
          privateLinkServiceId: target.resourceId
        }
      }
    ]
  }
}]

resource privateDnsZoneGroups 'Microsoft.Network/privateEndpoints/privateDnsZoneGroups@2024-05-01' = [for (target, index) in privateLinkTargets: {
  parent: privateEndpoints[index]
  name: 'default'
  properties: {
    privateDnsZoneConfigs: [
      {
        name: target.name
        properties: {
          privateDnsZoneId: privateDnsZones[index].id
        }
      }
    ]
  }
  dependsOn: [
    privateDnsLinks[index]
  ]
}]

module monitorAlerts 'modules/monitor-alerts.bicep' = if (deployAlerts) {
  name: '${namePrefix}-monitor-alerts'
  params: {
    logAnalyticsWorkspaceId: logWorkspace.id
    actionGroupId: actionGroupId
    location: location
    environment: environmentTag
  }
}

module budget 'modules/budget.bicep' = if (deployAlerts) {
  name: '${namePrefix}-budget'
  params: {
    budgetName: '${namePrefix}-monthly-budget'
    amount: monthlyBudgetAmount
    actionGroupId: actionGroupId
  }
}

resource vaultDiagnostics 'Microsoft.Insights/diagnosticSettings@2021-05-01-preview' = {
  name: '${namePrefix}-vault-diagnostics'
  scope: vault
  properties: {
    workspaceId: logWorkspace.id
    logs: [
      { categoryGroup: 'audit', enabled: true }
    ]
  }
}

resource sqlDiagnostics 'Microsoft.Insights/diagnosticSettings@2021-05-01-preview' = {
  name: '${namePrefix}-sql-diagnostics'
  scope: sqlDatabase
  properties: {
    workspaceId: logWorkspace.id
    metrics: [
      { category: 'Basic', enabled: true }
    ]
  }
}

output apiFqdn string = api.?properties.configuration.ingress.fqdn ?? ''
output apiManagedIdentityObjectId string = identity.properties.principalId
output apiManagedIdentityClientId string = identity.properties.clientId
output apiManagedIdentityName string = identity.name
output sqlServerFqdn string = sqlServer.properties.fullyQualifiedDomainName
output sqlDatabaseName string = sqlDatabase.name
output registryLoginServer string = registry.properties.loginServer
output imagePullIdentityObjectId string = imagePullIdentity.properties.principalId
output imagePullIdentityResourceId string = imagePullIdentity.id
output sqlMigrationIdentityClientId string = migrationIdentity.properties.clientId
output sqlMigrationIdentityObjectId string = migrationIdentity.properties.principalId
output sqlMigrationIdentityResourceId string = migrationIdentity.id
output sqlMigrationIdentityName string = migrationIdentity.name
output signingKeyUri string = signingKey.properties.keyUri
output appConfigurationEndpoint string = configuration.properties.endpoint

// Foundry operational identifiers. These are the values deployment automation
// and tools/foundry-smoke/*.ps1 need to verify the model path. No token, no
// profile content, and no deployment name is emitted; this stack knows no
// Foundry deployment name.
output apiResourceId string = deployApi ? resourceId('Microsoft.App/containerApps', apiName) : ''
output foundryProjectEndpoint string = foundryProjectEndpoint
output foundryModelIdentityName string = foundryIdentityName
output foundryModelIdentityClientId string = foundryIdentityClientId
output foundryModelIdentityObjectId string = foundryIdentityPrincipalId
output foundryModelIdentityResourceId string = foundryIdentityResourceId
output foundryModelIdentityIsDedicated bool = !allowSharedPilotIdentity
output foundryRoleDefinitionId string = foundryDataPlaneRoleDefinitionId
output foundryRoleAssignmentScope string = foundryProjectResourceId
output foundryRoleAssignmentMode string = assignFoundryRoleHere ? 'in-template' : 'operator-managed'
output foundryModelProfileConfigurationPrefix string = modelProfileConfigurationPrefix
output foundryEgressPosture string = foundryEgressPosture
output foundryPrivateDnsZoneName string = foundryPrivateDnsZoneName
output foundryPrivateEndpointGroupId string = foundryPrivateEndpointGroupId
output legacyOpenAiAccountRetired bool = retireLegacyOpenAiAccount
