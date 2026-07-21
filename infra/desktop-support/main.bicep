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

@description('Object ID of the Entra group that administers the Azure SQL logical server.')
param sqlAdministratorObjectId string

@description('Display name of the Entra group that administers the Azure SQL logical server.')
param sqlAdministratorLogin string

@description('Azure OpenAI model name made available to the transient model broker.')
param modelName string

@description('Pinned Azure OpenAI model version.')
param modelVersion string

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

@description('Canonical deployment hash (sha256:<hex>).')
param deploymentHash string

@description('Deploy scheduled-query alerts and the monthly budget.')
param deployAlerts bool = false

@description('Action group resource id for alerts; required when deployAlerts is true.')
param actionGroupId string = ''

@description('Monthly budget amount in the subscription currency.')
param monthlyBudgetAmount int = 200

@description('Environment tag value used by alerting and budgets.')
param environmentTag string = 'staging'

assert immutableApiImage = !deployApi || (contains(containerImage, '@sha256:') && length(containerImage) >= 80)

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
// is isolated from the runtime identity; data/config, signing, and model
// access share the runtime identity attached to one process, with module
// seams preserved for independently deployed signing/model workloads if the
// threat review requires hard isolation later. The migration identity holds
// schema (DDL) rights only and is never attached to the API.
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

resource openAi 'Microsoft.CognitiveServices/accounts@2025-12-01' = {
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

resource modelDeployment 'Microsoft.CognitiveServices/accounts/deployments@2024-10-01' = {
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
    userAssignedIdentities: {
      '${identity.id}': {}
      '${imagePullIdentity.id}': {}
    }
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
            { name: 'Sapphirus__Region', value: location }
            { name: 'Sapphirus__ReleaseChannel', value: releaseChannel }
            { name: 'Sapphirus__ManagedIdentityClientId', value: identity.properties.clientId }
            { name: 'Sapphirus__AppConfigurationEndpoint', value: configuration.properties.endpoint }
            { name: 'Sapphirus__KeyVaultUri', value: vault.properties.vaultUri }
            { name: 'Sapphirus__ReceiptSigningKeyName', value: signingKey.name }
            { name: 'Sapphirus__ProviderProfileHash', value: providerProfileHash }
            { name: 'Sapphirus__ModelProfileHash', value: modelProfileHash }
            { name: 'Sapphirus__ModelCapabilityHash', value: modelCapabilityHash }
            { name: 'Sapphirus__DeploymentHash', value: deploymentHash }
            { name: 'Sapphirus__SqlServer', value: sqlServer.properties.fullyQualifiedDomainName }
            { name: 'Sapphirus__SqlDatabase', value: sqlDatabase.name }
            { name: 'Sapphirus__ModelEndpoint', value: openAi.properties.endpoint }
            { name: 'Sapphirus__ModelDeployment', value: modelDeployment.name }
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

resource modelUser 'Microsoft.Authorization/roleAssignments@2022-04-01' = {
  name: guid(openAi.id, identity.id, roleIds.cognitiveServicesOpenAiUser)
  scope: openAi
  properties: {
    principalId: identity.properties.principalId
    principalType: 'ServicePrincipal'
    roleDefinitionId: subscriptionResourceId('Microsoft.Authorization/roleDefinitions', roleIds.cognitiveServicesOpenAiUser)
  }
}

var privateLinkTargets = [
  { name: 'registry', resourceId: registry.id, groupId: 'registry', zone: 'privatelink.azurecr.io' }
  { name: 'vault', resourceId: vault.id, groupId: 'vault', zone: 'privatelink.vaultcore.azure.net' }
  { name: 'configuration', resourceId: configuration.id, groupId: 'configurationStores', zone: 'privatelink.azconfig.io' }
  { name: 'sql', resourceId: sqlServer.id, groupId: 'sqlServer', zone: 'privatelink${az.environment().suffixes.sqlServerHostname}' }
  { name: 'openai', resourceId: openAi.id, groupId: 'account', zone: 'privatelink.openai.azure.com' }
]

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
output sqlServerFqdn string = sqlServer.properties.fullyQualifiedDomainName
output sqlDatabaseName string = sqlDatabase.name
output registryLoginServer string = registry.properties.loginServer
output imagePullIdentityObjectId string = imagePullIdentity.properties.principalId
output sqlMigrationIdentityClientId string = migrationIdentity.properties.clientId
output sqlMigrationIdentityObjectId string = migrationIdentity.properties.principalId
output signingKeyUri string = signingKey.properties.keyUri
output appConfigurationEndpoint string = configuration.properties.endpoint
output modelEndpoint string = openAi.properties.endpoint
