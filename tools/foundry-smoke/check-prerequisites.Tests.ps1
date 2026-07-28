$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

. "$PSScriptRoot/check-prerequisites.ps1"

$script:failures = [System.Collections.Generic.List[string]]::new()
$script:passes = 0

function Assert-Equal {
    param(
        [Parameter(Mandatory)]
        [string] $Name,

        [AllowNull()]
        [object] $Actual,

        [AllowNull()]
        [object] $Expected
    )

    if ($Actual -ne $Expected) {
        $script:failures.Add("$Name expected '$Expected' but received '$Actual'")
        return
    }
    $script:passes += 1
}

function Assert-True {
    param(
        [Parameter(Mandatory)]
        [string] $Name,

        [Parameter(Mandatory)]
        [bool] $Condition
    )

    if (-not $Condition) {
        $script:failures.Add("$Name expected true")
        return
    }
    $script:passes += 1
}

function Assert-ThrowsCode {
    param(
        [Parameter(Mandatory)]
        [string] $Name,

        [Parameter(Mandatory)]
        [scriptblock] $Action,

        [Parameter(Mandatory)]
        [string] $Code
    )

    try {
        & $Action | Out-Null
        $script:failures.Add("$Name expected '$Code' but did not throw")
    }
    catch {
        $expectedMessage = "foundry_prerequisite:$Code"
        if ($_.Exception.Message -ne $expectedMessage) {
            $script:failures.Add(
                "$Name expected '$expectedMessage' but received '$($_.Exception.Message)'"
            )
            return
        }
        $script:passes += 1
    }
}

function Invoke-TestCase {
    param(
        [Parameter(Mandatory)]
        [string] $Name,

        [Parameter(Mandatory)]
        [scriptblock] $Action
    )

    try {
        & $Action
    }
    catch {
        $script:failures.Add("$Name failed with '$($_.Exception.Message)'")
    }
}

$namedEndpoint = Resolve-FoundryProjectEndpoint `
    -Value 'https://forge-east.services.ai.azure.com/api/projects/forge-prod'
Assert-Equal -Name 'named endpoint canonical form' `
    -Actual $namedEndpoint.AbsoluteUri `
    -Expected 'https://forge-east.services.ai.azure.com/api/projects/forge-prod'

$defaultEndpoint = Resolve-FoundryProjectEndpoint `
    -Value 'https://forge-east.services.ai.azure.com/api/projects/_project'
Assert-Equal -Name 'default project endpoint canonical form' `
    -Actual $defaultEndpoint.AbsoluteUri `
    -Expected 'https://forge-east.services.ai.azure.com/api/projects/_project'

$invalidEndpoints = [ordered]@{
    'http is rejected' = @(
        'http://forge-east.services.ai.azure.com/api/projects/forge-prod',
        'project_endpoint_scheme_invalid'
    )
    'nondefault port is rejected' = @(
        'https://forge-east.services.ai.azure.com:444/api/projects/forge-prod',
        'project_endpoint_port_invalid'
    )
    'userinfo is rejected' = @(
        'https://user@forge-east.services.ai.azure.com/api/projects/forge-prod',
        'project_endpoint_userinfo_invalid'
    )
    'lookalike host is rejected' = @(
        'https://forge-east.services.ai.azure.com.evil.example/api/projects/forge-prod',
        'project_endpoint_host_invalid'
    )
    'query is rejected' = @(
        'https://forge-east.services.ai.azure.com/api/projects/forge-prod?x=1',
        'project_endpoint_query_invalid'
    )
    'fragment is rejected' = @(
        'https://forge-east.services.ai.azure.com/api/projects/forge-prod#x',
        'project_endpoint_fragment_invalid'
    )
    'extra path is rejected' = @(
        'https://forge-east.services.ai.azure.com/api/projects/forge-prod/extra',
        'project_endpoint_path_invalid'
    )
    'encoded slash is rejected' = @(
        'https://forge-east.services.ai.azure.com/api/projects/forge%2Fprod',
        'project_endpoint_path_invalid'
    )
}
foreach ($case in $invalidEndpoints.GetEnumerator()) {
    $value = $case.Value[0]
    $code = $case.Value[1]
    Assert-ThrowsCode -Name $case.Key -Code $code -Action {
        Resolve-FoundryProjectEndpoint -Value $value
    }
}

$nextLink = Resolve-FoundryNextLink `
    -ProjectEndpoint $namedEndpoint `
    -Value (
        'https://forge-east.services.ai.azure.com/api/projects/forge-prod/' +
        'deployments?api-version=v1&deploymentType=ModelDeployment&continuationToken=abc'
    )
Assert-Equal -Name 'same-origin nextLink is accepted' `
    -Actual $nextLink.Host `
    -Expected 'forge-east.services.ai.azure.com'

Assert-ThrowsCode -Name 'cross-origin nextLink is rejected' `
    -Code 'pagination_origin_invalid' `
    -Action {
        Resolve-FoundryNextLink -ProjectEndpoint $namedEndpoint -Value (
            'https://attacker.example/api/projects/forge-prod/' +
            'deployments?api-version=v1'
        )
    }

Assert-ThrowsCode -Name 'wrong pagination path is rejected' `
    -Code 'pagination_path_invalid' `
    -Action {
        Resolve-FoundryNextLink -ProjectEndpoint $namedEndpoint -Value (
            'https://forge-east.services.ai.azure.com/api/projects/other/' +
            'deployments?api-version=v1'
        )
    }

Assert-ThrowsCode -Name 'wrong pagination API version is rejected' `
    -Code 'pagination_api_version_invalid' `
    -Action {
        Resolve-FoundryNextLink -ProjectEndpoint $namedEndpoint -Value (
            'https://forge-east.services.ai.azure.com/api/projects/forge-prod/' +
            'deployments?api-version=v2'
        )
    }

Assert-ThrowsCode -Name 'pagination fragment is rejected' `
    -Code 'pagination_fragment_invalid' `
    -Action {
        Resolve-FoundryNextLink -ProjectEndpoint $namedEndpoint -Value (
            'https://forge-east.services.ai.azure.com/api/projects/forge-prod/' +
            'deployments?api-version=v1#fragment'
        )
    }

$rawDeployment = [pscustomobject]@{
    name = 'desktop-planning'
    type = 'ModelDeployment'
    modelPublisher = 'OpenAI'
    modelName = 'gpt-5-mini'
    modelVersion = '2026-06-01'
    connectionName = 'must-not-leak'
    capabilities = [pscustomobject]@{
        tool_calling = $true
        json_schema = 'must-not-leak'
    }
    sku = [pscustomobject]@{
        name = 'GlobalStandard'
        tier = 'Global'
        size = 'S'
        family = 'OpenAI'
        capacity = 10
        internal = 'must-not-leak'
    }
}
$redacted = ConvertTo-RedactedFoundryDeployment -Value $rawDeployment
Assert-Equal -Name 'deployment projection has exact fields' `
    -Actual ($redacted.PSObject.Properties.Name -join ',') `
    -Expected 'deploymentName,modelPublisher,modelName,modelVersion,sku,type,capabilityKeys'
Assert-Equal -Name 'capability keys are sorted' `
    -Actual ($redacted.capabilityKeys -join ',') `
    -Expected 'json_schema,tool_calling'
Assert-Equal -Name 'SKU projection has exact fields' `
    -Actual ($redacted.sku.PSObject.Properties.Name -join ',') `
    -Expected 'name,tier,size,family,capacity'
$redactedJson = $redacted | ConvertTo-Json -Depth 8 -Compress
Assert-True -Name 'connection name is omitted' `
    -Condition (-not $redactedJson.Contains('must-not-leak'))

Assert-ThrowsCode -Name 'nested deployment metadata is rejected' `
    -Code 'deployment_record_invalid' `
    -Action {
        ConvertTo-RedactedFoundryDeployment -Value ([pscustomobject]@{
            name = 'desktop-planning'
            type = 'ModelDeployment'
            modelPublisher = 'OpenAI'
            modelName = 'gpt-5-mini'
            modelVersion = '2026-06-01'
            capabilities = [pscustomobject]@{
                tool_calling = $true
            }
            sku = [pscustomobject]@{
                name = [pscustomobject]@{
                    nested = 'must-not-be-persisted'
                }
                tier = 'Global'
                size = 'S'
                family = 'OpenAI'
                capacity = 10
            }
        })
    }

Assert-ThrowsCode -Name 'invalid capability key is rejected' `
    -Code 'deployment_record_invalid' `
    -Action {
        ConvertTo-RedactedFoundryDeployment -Value ([pscustomobject]@{
            name = 'desktop-planning'
            type = 'ModelDeployment'
            modelPublisher = 'OpenAI'
            modelName = 'gpt-5-mini'
            modelVersion = '2026-06-01'
            capabilities = [pscustomobject]@{
                'invalid key' = $true
            }
            sku = [pscustomobject]@{
                name = 'GlobalStandard'
                tier = 'Global'
                size = 'S'
                family = 'OpenAI'
                capacity = 10
            }
        })
    }

Assert-ThrowsCode -Name 'missing model metadata is rejected' `
    -Code 'deployment_record_invalid' `
    -Action {
        ConvertTo-RedactedFoundryDeployment -Value ([pscustomobject]@{
            name = 'desktop-planning'
            type = 'ModelDeployment'
            modelPublisher = 'OpenAI'
            modelVersion = '2026-06-01'
            capabilities = [pscustomobject]@{}
            sku = [pscustomobject]@{
                name = 'GlobalStandard'
                tier = 'Global'
                size = 'S'
                family = 'OpenAI'
                capacity = 10
            }
        })
    }

$selection = Test-FoundryDeploymentSelection `
    -Deployments @($redacted) `
    -IntendedDeploymentNames @('desktop-planning')
Assert-True -Name 'explicit deployment selection is ready' -Condition $selection.ready
Assert-Equal -Name 'selected deployment is exact' `
    -Actual ($selection.selectedDeploymentNames -join ',') `
    -Expected 'desktop-planning'

$missingSelection = Test-FoundryDeploymentSelection `
    -Deployments @($redacted) `
    -IntendedDeploymentNames @('missing-deployment')
Assert-True -Name 'missing intended deployment blocks readiness' `
    -Condition (-not $missingSelection.ready)
Assert-Equal -Name 'missing intended deployment has stable blocker' `
    -Actual ($missingSelection.blockers -join ',') `
    -Expected 'intended_deployment_missing'

$identityResourceId = (
    '/subscriptions/00000000-0000-0000-0000-000000000001/' +
    'resourceGroups/forge/providers/Microsoft.ManagedIdentity/' +
    'userAssignedIdentities/forge-model-broker'
)
$identityClientId = '00000000-0000-0000-0000-000000000002'
$identityPrincipalId = '00000000-0000-0000-0000-000000000003'
$managedIdentity = [pscustomobject]@{
    id = $identityResourceId
    clientId = $identityClientId
    principalId = $identityPrincipalId
}
$containerIdentity = [pscustomobject]@{
    userAssignedIdentities = [pscustomobject]@{
        $identityResourceId = [pscustomobject]@{
            clientId = $identityClientId
            principalId = $identityPrincipalId
        }
    }
}
$identityCheck = Test-FoundryManagedIdentityAttachment `
    -ManagedIdentityClientId $identityClientId `
    -ManagedIdentity $managedIdentity `
    -ContainerAppIdentity $containerIdentity
Assert-True -Name 'matching identity is attached' -Condition $identityCheck.attached
Assert-True -Name 'identity report contains only hashes' `
    -Condition (
        $identityCheck.clientIdHash.StartsWith('sha256:') -and
        $identityCheck.resourceIdHash.StartsWith('sha256:') -and
        -not (($identityCheck | ConvertTo-Json -Compress).Contains($identityClientId))
    )

$mismatchedContainerIdentity = [pscustomobject]@{
    userAssignedIdentities = [pscustomobject]@{
        $identityResourceId = [pscustomobject]@{
            clientId = '00000000-0000-0000-0000-000000000099'
            principalId = $identityPrincipalId
        }
    }
}
$mismatchedIdentityCheck = Test-FoundryManagedIdentityAttachment `
    -ManagedIdentityClientId $identityClientId `
    -ManagedIdentity $managedIdentity `
    -ContainerAppIdentity $mismatchedContainerIdentity
Assert-True -Name 'identity attachment requires matching projected IDs' `
    -Condition (-not $mismatchedIdentityCheck.attached)

Invoke-TestCase -Name 'malformed attached identity remains unattached' -Action {
    $malformedContainerIdentity = [pscustomobject]@{
        userAssignedIdentities = [pscustomobject]@{
            $identityResourceId = [pscustomobject]@{}
        }
    }
    $malformedIdentityCheck = Test-FoundryManagedIdentityAttachment `
        -ManagedIdentityClientId $identityClientId `
        -ManagedIdentity $managedIdentity `
        -ContainerAppIdentity $malformedContainerIdentity
    Assert-True -Name 'missing attachment IDs fail closed' `
        -Condition (-not $malformedIdentityCheck.attached)
}

$foundryScope = (
    '/subscriptions/00000000-0000-0000-0000-000000000001/' +
    'resourceGroups/forge/providers/Microsoft.CognitiveServices/' +
    'accounts/forge-east/projects/forge-prod'
)
$roleAssignments = @(
    [pscustomobject]@{
        principalId = $identityPrincipalId
        roleDefinitionId = (
            '/subscriptions/00000000-0000-0000-0000-000000000001/' +
            'providers/Microsoft.Authorization/roleDefinitions/' +
            '53ca6127-db72-4b80-b1b0-d745d6d5456d'
        )
        scope = (
            '/subscriptions/00000000-0000-0000-0000-000000000001/' +
            'resourceGroups/forge/providers/Microsoft.CognitiveServices/' +
            'accounts/forge-east'
        )
    }
)
$roleCheck = Test-FoundryRoleAssignment `
    -PrincipalId $identityPrincipalId `
    -FoundryScope $foundryScope `
    -Assignments $roleAssignments
Assert-True -Name 'inherited Foundry User assignment is accepted' `
    -Condition $roleCheck.assigned
Assert-Equal -Name 'role definition is pinned by ID' `
    -Actual $roleCheck.roleDefinitionId `
    -Expected '53ca6127-db72-4b80-b1b0-d745d6d5456d'

$emptyScopeRoleCheck = Test-FoundryRoleAssignment `
    -PrincipalId $identityPrincipalId `
    -FoundryScope $foundryScope `
    -Assignments @(
        [pscustomobject]@{
            principalId = $identityPrincipalId
            roleDefinitionId = (
                '/subscriptions/00000000-0000-0000-0000-000000000001/' +
                'providers/Microsoft.Authorization/roleDefinitions/' +
                '53ca6127-db72-4b80-b1b0-d745d6d5456d'
            )
            scope = ''
        }
    )
Assert-True -Name 'empty role scope fails closed' `
    -Condition (-not $emptyScopeRoleCheck.assigned)

Invoke-TestCase -Name 'two-page deployment inventory' -Action {
    $firstPageUri = (
        'https://forge-east.services.ai.azure.com/api/projects/forge-prod/' +
        'deployments?api-version=v1&deploymentType=ModelDeployment'
    )
    $secondPageUri = (
        'https://forge-east.services.ai.azure.com/api/projects/forge-prod/' +
        'deployments?api-version=v1&deploymentType=ModelDeployment&continuationToken=page-2'
    )
    $pages = @{
        $firstPageUri = @'
{
  "value": [
    {
      "name": "alpha",
      "type": "ModelDeployment",
      "modelPublisher": "OpenAI",
      "modelName": "gpt-5-mini",
      "modelVersion": "2026-06-01",
      "sku": {
        "name": "GlobalStandard",
        "tier": "Global",
        "size": "S",
        "family": "OpenAI",
        "capacity": 10
      },
      "capabilities": {
        "json_schema": true
      }
    }
  ],
  "nextLink": "https://forge-east.services.ai.azure.com/api/projects/forge-prod/deployments?api-version=v1&deploymentType=ModelDeployment&continuationToken=page-2"
}
'@
        $secondPageUri = @'
{
  "value": [
    {
      "name": "beta",
      "type": "ModelDeployment",
      "modelPublisher": "Microsoft",
      "modelName": "phi-4",
      "modelVersion": "2026-05-01",
      "sku": {
        "name": "Standard",
        "tier": "Regional",
        "size": "S",
        "family": "Phi",
        "capacity": 5
      },
      "capabilities": {
        "tool_calling": true
      }
    }
  ]
}
'@
    }
    $received = [System.Collections.Generic.List[string]]::new()
    $inventory = Get-FoundryDeploymentInventory `
        -ProjectEndpoint $namedEndpoint `
        -PageFetcher {
            param([uri] $Uri)
            $received.Add($Uri.AbsoluteUri)
            return $pages[$Uri.AbsoluteUri]
        }
    Assert-Equal -Name 'deployment inventory follows two pages' `
        -Actual $inventory.pageCount `
        -Expected 2
    Assert-Equal -Name 'deployment inventory is sorted' `
        -Actual (($inventory.deployments | ForEach-Object deploymentName) -join ',') `
        -Expected 'alpha,beta'
    Assert-Equal -Name 'page fetcher receives validated URLs' `
        -Actual ($received -join ',') `
        -Expected "$firstPageUri,$secondPageUri"
    Assert-True -Name 'deployment inventory has a stable hash' `
        -Condition (
            $inventory.inventoryHash -match '^sha256:[0-9a-f]{64}$'
        )
}

Invoke-TestCase -Name 'cross-origin page is rejected before fetch' -Action {
    Assert-ThrowsCode -Name 'cross-origin response nextLink is rejected' `
        -Code 'pagination_origin_invalid' `
        -Action {
            Get-FoundryDeploymentInventory `
                -ProjectEndpoint $namedEndpoint `
                -PageFetcher {
                    param([uri] $Uri)
                    return @'
{
  "value": [],
  "nextLink": "https://attacker.example/api/projects/forge-prod/deployments?api-version=v1"
}
'@
                }
    }
}

Invoke-TestCase -Name 'empty deployment inventory remains explicit' -Action {
    $emptyInventory = Get-FoundryDeploymentInventory `
        -ProjectEndpoint $namedEndpoint `
        -PageFetcher {
            param([uri] $Uri)
            return '{"value":[]}'
        }
    Assert-Equal -Name 'empty inventory records one page' `
        -Actual $emptyInventory.pageCount `
        -Expected 1
    Assert-Equal -Name 'empty inventory records no deployments' `
        -Actual @($emptyInventory.deployments).Count `
        -Expected 0
    Assert-Equal -Name 'empty inventory hash is deterministic' `
        -Actual $emptyInventory.inventoryHash `
        -Expected (
            'sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5' +
            'ed12ab4d8e11ba873c2f11161202b945'
        )
    $emptySelection = Test-FoundryDeploymentSelection `
        -Deployments @($emptyInventory.deployments) `
        -IntendedDeploymentNames @('desktop-planning')
    Assert-True -Name 'empty inventory blocks readiness' `
        -Condition (-not $emptySelection.ready)
    Assert-Equal -Name 'empty inventory exposes both selection blockers' `
        -Actual ($emptySelection.blockers -join ',') `
        -Expected 'deployment_inventory_empty,intended_deployment_missing'
}

Invoke-TestCase -Name 'deployment response stream is bounded while reading' -Action {
    $smallBytes = [System.Text.Encoding]::UTF8.GetBytes('{"value":[]}')
    $smallStream = [System.IO.MemoryStream]::new($smallBytes, $false)
    try {
        Assert-Equal -Name 'bounded stream returns UTF-8 content' `
            -Actual (
                Read-FoundryBoundedStream `
                    -Stream $smallStream `
                    -MaximumBytes 1024
            ) `
            -Expected '{"value":[]}'
    }
    finally {
        $smallStream.Dispose()
    }

    $largeStream = [System.IO.MemoryStream]::new(
        ([byte[]]::new(1025)),
        $false
    )
    try {
        Assert-ThrowsCode -Name 'oversized stream is rejected before buffering all content' `
            -Code 'deployment_inventory_body_too_large' `
            -Action {
                Read-FoundryBoundedStream `
                    -Stream $largeStream `
                    -MaximumBytes 1024
            }
    }
    finally {
        $largeStream.Dispose()
    }

    $cancelledStream = [System.IO.MemoryStream]::new($smallBytes, $false)
    $cancellation = [System.Threading.CancellationTokenSource]::new()
    try {
        $cancellation.Cancel()
        Assert-ThrowsCode -Name 'cancelled content read has a stable timeout code' `
            -Code 'deployment_inventory_timeout' `
            -Action {
                Read-FoundryBoundedStream `
                    -Stream $cancelledStream `
                    -MaximumBytes 1024 `
                    -CancellationToken $cancellation.Token
            }
    }
    finally {
        $cancellation.Dispose()
        $cancelledStream.Dispose()
    }
}

Invoke-TestCase -Name 'default endpoint rejects an unbindable project role scope' -Action {
    $reportPath = Join-Path `
        ([System.IO.Path]::GetTempPath()) `
        "forge-foundry-default-scope-$([guid]::NewGuid().ToString('N')).json"
    try {
        $exitCode = Invoke-FoundryPrerequisiteCheck `
            -ProjectEndpointValue (
                'https://forge-east.services.ai.azure.com/api/projects/_project'
            ) `
            -ManagedIdentityClientIdValue $identityClientId `
            -ContainerAppResourceIdValue (
                '/subscriptions/00000000-0000-0000-0000-000000000001/' +
                'resourceGroups/forge/providers/Microsoft.App/' +
                'containerApps/forge-support'
            ) `
            -FoundryRoleScopeValue $foundryScope `
            -IntendedDeploymentNames @('desktop-planning') `
            -ReportPath $reportPath `
            -AzInvoker {
                param([string[]] $Arguments)
                throw 'synthetic unauthenticated Azure CLI'
            }
        Assert-Equal -Name 'unbound default endpoint remains blocked' `
            -Actual $exitCode `
            -Expected 2
        $report = Get-Content -Raw -LiteralPath $reportPath |
            ConvertFrom-Json -Depth 32
        Assert-True -Name 'unbound project scope has a stable blocker' `
            -Condition (
                @($report.blockers).Contains(
                    'foundry_role_scope_endpoint_mismatch'
                )
            )
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

Invoke-TestCase -Name 'missing inputs produce a durable blocked report' -Action {
    $reportPath = Join-Path `
        ([System.IO.Path]::GetTempPath()) `
        "forge-foundry-prerequisites-$([guid]::NewGuid().ToString('N')).json"
    try {
        & pwsh -NoProfile -File "$PSScriptRoot/check-prerequisites.ps1" `
            -ProjectEndpoint '' `
            -ManagedIdentityClientId '' `
            -ContainerAppResourceId '' `
            -FoundryRoleScope '' `
            -IntendedDeploymentName '' `
            -OutputPath $reportPath
        $exitCode = $LASTEXITCODE
        Assert-Equal -Name 'blocked prerequisite command exit code' `
            -Actual $exitCode `
            -Expected 2
        Assert-True -Name 'blocked prerequisite report exists' `
            -Condition (Test-Path -LiteralPath $reportPath -PathType Leaf)
        $report = Get-Content -Raw -LiteralPath $reportPath |
            ConvertFrom-Json -Depth 32
        Assert-True -Name 'missing-input report is not ready' `
            -Condition (-not $report.ready)
        $requiredInputBlockers = @(
            'container_app_resource_id_missing',
            'foundry_role_scope_missing',
            'intended_deployments_missing',
            'managed_identity_client_id_missing',
            'project_endpoint_missing'
        )
        Assert-True -Name 'missing-input blockers are all explicit' `
            -Condition (
                @(
                    $requiredInputBlockers |
                        Where-Object { $_ -notin $report.blockers }
                ).Count -eq 0
            )
        $reportJson = $report | ConvertTo-Json -Depth 32 -Compress
        Assert-True -Name 'blocked report does not contain credentials' `
            -Condition (
                -not $reportJson.Contains('accessToken') -and
                -not $reportJson.Contains('Authorization')
            )
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

Invoke-TestCase -Name 'authentication is checked despite missing inputs' -Action {
    $reportPath = Join-Path `
        ([System.IO.Path]::GetTempPath()) `
        "forge-foundry-auth-blocked-$([guid]::NewGuid().ToString('N')).json"
    try {
        $exitCode = Invoke-FoundryPrerequisiteCheck `
            -ProjectEndpointValue '' `
            -ManagedIdentityClientIdValue '' `
            -ContainerAppResourceIdValue '' `
            -FoundryRoleScopeValue '' `
            -IntendedDeploymentNames @() `
            -ReportPath $reportPath `
            -AzInvoker {
                param([string[]] $Arguments)
                throw 'synthetic Azure login is unavailable'
            }
        Assert-Equal -Name 'missing authentication remains blocked' `
            -Actual $exitCode `
            -Expected 2
        $report = Get-Content -Raw -LiteralPath $reportPath |
            ConvertFrom-Json -Depth 32
        Assert-True -Name 'missing authentication has stable blocker' `
            -Condition (
                'azure_cli_authentication_missing' -in $report.blockers
            )
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

Invoke-TestCase -Name 'Azure resource IDs are parsed without guesswork' -Action {
    $containerResource = Resolve-AzureResourceId `
        -Value (
            '/subscriptions/00000000-0000-0000-0000-000000000001/' +
            'resourceGroups/forge/providers/Microsoft.App/' +
            'containerApps/forge-support'
        )
    Assert-Equal -Name 'resource ID subscription is exact' `
        -Actual $containerResource.subscriptionId `
        -Expected '00000000-0000-0000-0000-000000000001'
    Assert-Equal -Name 'resource ID provider is exact' `
        -Actual $containerResource.providerNamespace `
        -Expected 'Microsoft.App'
    Assert-Equal -Name 'resource ID type and name are paired' `
        -Actual (
            ($containerResource.typeNamePairs |
                ForEach-Object { "$($_.type):$($_.name)" }) -join ','
        ) `
        -Expected 'containerApps:forge-support'

    Assert-ThrowsCode -Name 'resource ID with an unpaired segment is rejected' `
        -Code 'azure_resource_id_invalid' `
        -Action {
            Resolve-AzureResourceId -Value (
                '/subscriptions/00000000-0000-0000-0000-000000000001/' +
                'resourceGroups/forge/providers/Microsoft.App/containerApps'
            )
        }
}

Invoke-TestCase -Name 'complete prerequisite proof becomes ready' -Action {
    $reportPath = Join-Path `
        ([System.IO.Path]::GetTempPath()) `
        "forge-foundry-ready-$([guid]::NewGuid().ToString('N')).json"
    $containerResourceId = (
        '/subscriptions/00000000-0000-0000-0000-000000000001/' +
        'resourceGroups/forge/providers/Microsoft.App/' +
        'containerApps/forge-support'
    )
    try {
        $exitCode = Invoke-FoundryPrerequisiteCheck `
            -ProjectEndpointValue (
                'https://forge-east.services.ai.azure.com/api/projects/forge-prod'
            ) `
            -ManagedIdentityClientIdValue $identityClientId `
            -ContainerAppResourceIdValue $containerResourceId `
            -FoundryRoleScopeValue $foundryScope `
            -IntendedDeploymentNames @('desktop-planning') `
            -ReportPath $reportPath `
            -AzInvoker {
                param([string[]] $Arguments)
                $command = $Arguments -join ' '
                if ($command.StartsWith('account show ')) {
                    return @'
{
  "id": "00000000-0000-0000-0000-000000000001",
  "tenantId": "00000000-0000-0000-0000-000000000004"
}
'@
                }
                if ($command.StartsWith('account get-access-token ')) {
                    return 'synthetic-secret-token-must-not-leak'
                }
                if ($command.StartsWith('identity list ')) {
                    return @"
[
  {
    "id": "$identityResourceId",
    "clientId": "$identityClientId",
    "principalId": "$identityPrincipalId"
  }
]
"@
                }
                if ($command.StartsWith('containerapp identity show ')) {
                    return @"
{
  "userAssignedIdentities": {
    "$identityResourceId": {
      "clientId": "$identityClientId",
      "principalId": "$identityPrincipalId"
    }
  }
}
"@
                }
                if ($command.StartsWith('role assignment list ')) {
                    if (
                        -not $command.Contains(
                            '--subscription 00000000-0000-0000-0000-000000000001'
                        ) -or
                        -not $command.Contains('--fill-principal-name false') -or
                        -not $command.Contains('--fill-role-definition-name false')
                    ) {
                        throw 'role lookup was not pinned to the intended subscription'
                    }
                    return ($roleAssignments | ConvertTo-Json -Depth 8)
                }
                throw "unexpected Azure CLI command: $command"
            } `
            -PageFetcher {
                param(
                    [uri] $Uri,
                    [string] $AccessToken
                )
                if ($AccessToken -ne 'synthetic-secret-token-must-not-leak') {
                    throw 'token was not kept at the HTTP boundary'
                }
                return @'
{
  "value": [
    {
      "name": "desktop-planning",
      "type": "ModelDeployment",
      "modelPublisher": "OpenAI",
      "modelName": "gpt-5-mini",
      "modelVersion": "2026-06-01",
      "connectionName": "must-not-leak",
      "sku": {
        "name": "GlobalStandard",
        "tier": "Global",
        "size": "S",
        "family": "OpenAI",
        "capacity": 10
      },
      "capabilities": {
        "json_schema": true,
        "tool_calling": true
      }
    }
  ]
}
'@
            }
        $report = Get-Content -Raw -LiteralPath $reportPath |
            ConvertFrom-Json -Depth 32
        Assert-Equal -Name (
            'ready prerequisite command exit code; blockers=' +
            (@($report.blockers) -join ',')
        ) `
            -Actual $exitCode `
            -Expected 0
        Assert-True -Name 'complete prerequisite report is ready' `
            -Condition $report.ready
        Assert-Equal -Name 'complete prerequisite report has no blockers' `
            -Actual @($report.blockers).Count `
            -Expected 0
        Assert-Equal -Name 'complete prerequisite report selects exact deployment' `
            -Actual ($report.selectedDeploymentNames -join ',') `
            -Expected 'desktop-planning'
        Assert-True -Name 'complete report records every proof' `
            -Condition (
                $report.checks.endpointValidated -and
                $report.checks.entraTokenAcquired -and
                $report.checks.deploymentInventoryCaptured -and
                $report.checks.managedIdentityAttached -and
                $report.checks.foundryRoleAssigned
            )
        $reportJson = $report | ConvertTo-Json -Depth 32 -Compress
        Assert-True -Name 'ready report redacts all identity and token material' `
            -Condition (
                -not $reportJson.Contains('synthetic-secret-token-must-not-leak') -and
                -not $reportJson.Contains($identityClientId) -and
                -not $reportJson.Contains($identityPrincipalId) -and
                -not $reportJson.Contains($identityResourceId) -and
                -not $reportJson.Contains('must-not-leak')
            )
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

if ($script:failures.Count -gt 0) {
    foreach ($failure in $script:failures) {
        Write-Output "FAIL: $failure"
    }
    throw "Foundry prerequisite tests failed: $($script:failures.Count)"
}

Write-Output "Foundry prerequisite tests passed: $script:passes"
