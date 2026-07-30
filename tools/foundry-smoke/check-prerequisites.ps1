[CmdletBinding()]
param(
    [string] $ProjectEndpoint = $env:FOUNDRY_PROJECT_ENDPOINT,
    [string] $ApiKeyEnvironmentVariable = 'FOUNDRY_API_KEY',
    [string] $Profiles = 'infra/desktop-support/model-profiles.json',
    [ValidateRange(0, 1000000000)]
    [long] $BudgetMicrounits = 5000,
    [ValidateSet(
        'configured_and_probed',
        'openai_models',
        'project_deployments_api_key'
    )]
    [string] $InventoryMode = $(
        if ([string]::IsNullOrWhiteSpace($env:FOUNDRY_INVENTORY_MODE)) {
            'configured_and_probed'
        }
        else {
            $env:FOUNDRY_INVENTORY_MODE
        }
    ),
    [string] $CredentialVersion = $(
        if ([string]::IsNullOrWhiteSpace($env:FOUNDRY_API_KEY_VERSION)) {
            'unversioned'
        }
        else {
            $env:FOUNDRY_API_KEY_VERSION
        }
    ),
    [string] $ManagedIdentityClientId = $env:FOUNDRY_MODEL_IDENTITY_CLIENT_ID,
    [string] $ContainerAppResourceId = $env:FOUNDRY_CONTAINER_APP_RESOURCE_ID,
    [string] $FoundryRoleScope = $env:FOUNDRY_PROJECT_RESOURCE_ID,
    [string[]] $IntendedDeploymentName = @(
        $env:FOUNDRY_PROFILE_DEPLOYMENTS -split ',' |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    ),
    [string] $OutputPath = (
        Join-Path `
            $PSScriptRoot `
            '..\..\artifacts\foundry\prerequisites.json'
    )
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Throw-FoundryPrerequisite {
    param([Parameter(Mandatory)][string] $Code)
    throw "foundry_prerequisite:$Code"
}

. "$PSScriptRoot/check-api-key-prerequisites.ps1"

function Get-FoundrySha256Label {
    param([Parameter(Mandatory)][string] $Value)
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($Value)
    $hash = [System.Security.Cryptography.SHA256]::HashData($bytes)
    return "sha256:$([Convert]::ToHexString($hash).ToLowerInvariant())"
}

function Resolve-FoundryProjectEndpoint {
    param([Parameter(Mandatory)][string] $Value)

    $parsed = $null
    if (-not [uri]::TryCreate($Value, [System.UriKind]::Absolute, [ref] $parsed)) {
        Throw-FoundryPrerequisite -Code 'project_endpoint_invalid'
    }
    if ($parsed.Scheme -cne 'https') {
        Throw-FoundryPrerequisite -Code 'project_endpoint_scheme_invalid'
    }
    if (-not $parsed.IsDefaultPort -or $parsed.Port -ne 443) {
        Throw-FoundryPrerequisite -Code 'project_endpoint_port_invalid'
    }
    if (-not [string]::IsNullOrEmpty($parsed.UserInfo)) {
        Throw-FoundryPrerequisite -Code 'project_endpoint_userinfo_invalid'
    }
    if (-not [string]::IsNullOrEmpty($parsed.Query)) {
        Throw-FoundryPrerequisite -Code 'project_endpoint_query_invalid'
    }
    if (-not [string]::IsNullOrEmpty($parsed.Fragment)) {
        Throw-FoundryPrerequisite -Code 'project_endpoint_fragment_invalid'
    }

    $hostSuffix = '.services.ai.azure.com'
    if (
        -not $parsed.DnsSafeHost.EndsWith(
            $hostSuffix,
            [System.StringComparison]::OrdinalIgnoreCase
        ) -or
        $parsed.DnsSafeHost.Length -le $hostSuffix.Length
    ) {
        Throw-FoundryPrerequisite -Code 'project_endpoint_host_invalid'
    }

    $projectPrefix = '/api/projects/'
    if (
        -not $parsed.AbsolutePath.StartsWith(
            $projectPrefix,
            [System.StringComparison]::Ordinal
        )
    ) {
        Throw-FoundryPrerequisite -Code 'project_endpoint_path_invalid'
    }
    $rawProjectName = $parsed.AbsolutePath.Substring($projectPrefix.Length)
    $decodedProjectName = [uri]::UnescapeDataString($rawProjectName)
    if (
        [string]::IsNullOrWhiteSpace($decodedProjectName) -or
        $decodedProjectName -in @('.', '..') -or
        $decodedProjectName.Length -gt 128 -or
        $decodedProjectName.IndexOfAny([char[]] "/\?#") -ge 0 -or
        $decodedProjectName -match '[\x00-\x1f\x7f]'
    ) {
        Throw-FoundryPrerequisite -Code 'project_endpoint_path_invalid'
    }

    $builder = [System.UriBuilder]::new(
        'https',
        $parsed.DnsSafeHost.ToLowerInvariant()
    )
    $builder.Port = -1
    $builder.Path = $parsed.AbsolutePath
    $builder.Query = ''
    $builder.Fragment = ''
    return $builder.Uri
}

function Resolve-FoundryNextLink {
    param(
        [Parameter(Mandatory)][uri] $ProjectEndpoint,
        [Parameter(Mandatory)][string] $Value
    )

    $parsed = $null
    if (-not [uri]::TryCreate($Value, [System.UriKind]::Absolute, [ref] $parsed)) {
        Throw-FoundryPrerequisite -Code 'pagination_uri_invalid'
    }
    if (-not [string]::IsNullOrEmpty($parsed.Fragment)) {
        Throw-FoundryPrerequisite -Code 'pagination_fragment_invalid'
    }
    if (
        $parsed.Scheme -cne 'https' -or
        -not $parsed.IsDefaultPort -or
        $parsed.Port -ne 443 -or
        -not [string]::IsNullOrEmpty($parsed.UserInfo) -or
        -not $parsed.DnsSafeHost.Equals(
            $ProjectEndpoint.DnsSafeHost,
            [System.StringComparison]::OrdinalIgnoreCase
        )
    ) {
        Throw-FoundryPrerequisite -Code 'pagination_origin_invalid'
    }

    $expectedPath = "$($ProjectEndpoint.AbsolutePath)/deployments"
    if (
        -not $parsed.AbsolutePath.Equals(
            $expectedPath,
            [System.StringComparison]::Ordinal
        )
    ) {
        Throw-FoundryPrerequisite -Code 'pagination_path_invalid'
    }

    $apiVersions = [System.Collections.Generic.List[string]]::new()
    foreach ($part in $parsed.Query.TrimStart('?').Split(
        '&',
        [System.StringSplitOptions]::RemoveEmptyEntries
    )) {
        $pair = $part.Split('=', 2)
        $name = [uri]::UnescapeDataString($pair[0])
        $valuePart = if ($pair.Count -eq 2) {
            [uri]::UnescapeDataString($pair[1])
        }
        else {
            ''
        }
        if ($name.Equals('api-version', [System.StringComparison]::Ordinal)) {
            $apiVersions.Add($valuePart)
        }
    }
    if ($apiVersions.Count -ne 1 -or $apiVersions[0] -cne 'v1') {
        Throw-FoundryPrerequisite -Code 'pagination_api_version_invalid'
    }
    return $parsed
}

function Get-FoundryDeploymentString {
    param(
        [Parameter(Mandatory)][object] $Value,
        [Parameter(Mandatory)][string] $PropertyName,
        [switch] $Required,
        [ValidateRange(1, 1024)][int] $MaximumLength = 256
    )

    $property = $Value.PSObject.Properties[$PropertyName]
    if ($null -eq $property -or $null -eq $property.Value) {
        if ($Required) {
            Throw-FoundryPrerequisite -Code 'deployment_record_invalid'
        }
        return $null
    }
    if ($property.Value -isnot [string]) {
        Throw-FoundryPrerequisite -Code 'deployment_record_invalid'
    }
    $text = [string] $property.Value
    if (
        ($Required -and [string]::IsNullOrWhiteSpace($text)) -or
        $text.Length -gt $MaximumLength -or
        $text -match '[\x00-\x1f\x7f]'
    ) {
        Throw-FoundryPrerequisite -Code 'deployment_record_invalid'
    }
    return $text
}

function ConvertTo-RedactedFoundryDeployment {
    param([Parameter(Mandatory)][object] $Value)

    if ($Value -isnot [System.Management.Automation.PSCustomObject]) {
        Throw-FoundryPrerequisite -Code 'deployment_record_invalid'
    }

    $deploymentName = Get-FoundryDeploymentString `
        -Value $Value `
        -PropertyName 'name' `
        -Required
    $type = Get-FoundryDeploymentString `
        -Value $Value `
        -PropertyName 'type' `
        -Required `
        -MaximumLength 128
    if ($type -cne 'ModelDeployment') {
        Throw-FoundryPrerequisite -Code 'deployment_record_invalid'
    }

    $capabilitiesProperty = $Value.PSObject.Properties['capabilities']
    $capabilityKeys = @(
        if (
            $null -ne $capabilitiesProperty -and
            $null -ne $capabilitiesProperty.Value
        ) {
            if (
                $capabilitiesProperty.Value -isnot
                    [System.Management.Automation.PSCustomObject]
            ) {
                Throw-FoundryPrerequisite -Code 'deployment_record_invalid'
            }
            $names = @(
                $capabilitiesProperty.Value.PSObject.Properties |
                    ForEach-Object { $_.Name }
            )
            if ($names.Count -gt 128) {
                Throw-FoundryPrerequisite -Code 'deployment_record_invalid'
            }
            foreach ($name in $names) {
                if ($name -notmatch '^[A-Za-z0-9._-]{1,128}$') {
                    Throw-FoundryPrerequisite -Code 'deployment_record_invalid'
                }
            }
            $names | Sort-Object -Unique
        }
    )

    $skuProperty = $Value.PSObject.Properties['sku']
    if (
        $null -eq $skuProperty -or
        $skuProperty.Value -isnot
            [System.Management.Automation.PSCustomObject]
    ) {
        Throw-FoundryPrerequisite -Code 'deployment_record_invalid'
    }
    $skuValue = $skuProperty.Value
    $capacity = $null
    $capacityProperty = $skuValue.PSObject.Properties['capacity']
    if ($null -ne $capacityProperty -and $null -ne $capacityProperty.Value) {
        $capacityTypeCode = [System.Type]::GetTypeCode(
            $capacityProperty.Value.GetType()
        )
        if (
            $capacityTypeCode -notin @(
                [System.TypeCode]::Byte,
                [System.TypeCode]::SByte,
                [System.TypeCode]::Int16,
                [System.TypeCode]::UInt16,
                [System.TypeCode]::Int32,
                [System.TypeCode]::UInt32,
                [System.TypeCode]::Int64
            ) -or
            [long] $capacityProperty.Value -lt 0
        ) {
            Throw-FoundryPrerequisite -Code 'deployment_record_invalid'
        }
        $capacity = [long] $capacityProperty.Value
    }
    $sku = [pscustomobject][ordered]@{
        name = Get-FoundryDeploymentString `
            -Value $skuValue `
            -PropertyName 'name' `
            -Required `
            -MaximumLength 128
        tier = Get-FoundryDeploymentString `
            -Value $skuValue `
            -PropertyName 'tier' `
            -MaximumLength 128
        size = Get-FoundryDeploymentString `
            -Value $skuValue `
            -PropertyName 'size' `
            -MaximumLength 128
        family = Get-FoundryDeploymentString `
            -Value $skuValue `
            -PropertyName 'family' `
            -MaximumLength 128
        capacity = $capacity
    }
    return [pscustomobject][ordered]@{
        deploymentName = $deploymentName
        modelPublisher = Get-FoundryDeploymentString `
            -Value $Value `
            -PropertyName 'modelPublisher' `
            -Required
        modelName = Get-FoundryDeploymentString `
            -Value $Value `
            -PropertyName 'modelName' `
            -Required
        modelVersion = Get-FoundryDeploymentString `
            -Value $Value `
            -PropertyName 'modelVersion' `
            -Required
        sku = $sku
        type = $type
        capabilityKeys = $capabilityKeys
    }
}

function Get-FoundryDeploymentInventory {
    param(
        [Parameter(Mandatory)]
        [uri] $ProjectEndpoint,

        [Parameter(Mandatory)]
        [scriptblock] $PageFetcher,

        [ValidateRange(1, 64)]
        [int] $MaximumPages = 32,

        [ValidateRange(1024, 16777216)]
        [int] $MaximumPageBytes = 1048576
    )

    $currentUri = [uri] (
        "$($ProjectEndpoint.AbsoluteUri)/deployments" +
        '?api-version=v1&deploymentType=ModelDeployment'
    )
    $visited = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::OrdinalIgnoreCase
    )
    $deployments = [System.Collections.Generic.List[object]]::new()
    $pageCount = 0

    while ($null -ne $currentUri) {
        if ($pageCount -ge $MaximumPages) {
            Throw-FoundryPrerequisite -Code 'pagination_limit_exceeded'
        }
        if (-not $visited.Add($currentUri.AbsoluteUri)) {
            Throw-FoundryPrerequisite -Code 'pagination_cycle_detected'
        }

        $body = & $PageFetcher $currentUri
        if ($body -isnot [string] -or [string]::IsNullOrWhiteSpace($body)) {
            Throw-FoundryPrerequisite -Code 'deployment_inventory_body_invalid'
        }
        if ([System.Text.Encoding]::UTF8.GetByteCount($body) -gt $MaximumPageBytes) {
            Throw-FoundryPrerequisite -Code 'deployment_inventory_body_too_large'
        }
        try {
            $page = $body | ConvertFrom-Json -Depth 64 -NoEnumerate
        }
        catch {
            Throw-FoundryPrerequisite -Code 'deployment_inventory_json_invalid'
        }
        if (
            $null -eq $page -or
            $null -eq $page.PSObject.Properties['value']
        ) {
            Throw-FoundryPrerequisite -Code 'deployment_inventory_shape_invalid'
        }

        foreach ($deployment in @($page.value)) {
            $deployments.Add(
                (ConvertTo-RedactedFoundryDeployment -Value $deployment)
            )
        }
        $pageCount += 1

        $nextLinkProperty = $page.PSObject.Properties['nextLink']
        if (
            $null -eq $nextLinkProperty -or
            [string]::IsNullOrWhiteSpace([string] $nextLinkProperty.Value)
        ) {
            $currentUri = $null
        }
        else {
            $currentUri = Resolve-FoundryNextLink `
                -ProjectEndpoint $ProjectEndpoint `
                -Value ([string] $nextLinkProperty.Value)
        }
    }

    $orderedDeployments = @(
        $deployments |
            Sort-Object `
                deploymentName,
                modelPublisher,
                modelName,
                modelVersion
    )
    $canonicalJson = if ($orderedDeployments.Count -eq 0) {
        '[]'
    }
    else {
        ConvertTo-Json `
            -InputObject $orderedDeployments `
            -Depth 8 `
            -Compress
    }
    return [pscustomobject][ordered]@{
        pageCount = $pageCount
        inventoryHash = Get-FoundrySha256Label -Value $canonicalJson
        deployments = $orderedDeployments
    }
}

function Test-FoundryDeploymentSelection {
    param(
        [Parameter(Mandatory)]
        [AllowEmptyCollection()]
        [object[]] $Deployments,
        [Parameter(Mandatory)][string[]] $IntendedDeploymentNames
    )

    $blockers = [System.Collections.Generic.List[string]]::new()
    $deploymentNames = @($Deployments | ForEach-Object { $_.deploymentName })
    $duplicates = @(
        $deploymentNames |
            Group-Object |
            Where-Object Count -gt 1
    )
    if ($duplicates.Count -gt 0) {
        $blockers.Add('deployment_duplicate')
    }
    if ($Deployments.Count -eq 0) {
        $blockers.Add('deployment_inventory_empty')
    }

    $selected = [System.Collections.Generic.List[string]]::new()
    foreach ($name in @($IntendedDeploymentNames | Sort-Object -Unique)) {
        if (
            [string]::IsNullOrWhiteSpace($name) -or
            $deploymentNames -cnotcontains $name
        ) {
            if (-not $blockers.Contains('intended_deployment_missing')) {
                $blockers.Add('intended_deployment_missing')
            }
            continue
        }
        $selected.Add($name)
    }
    if ($IntendedDeploymentNames.Count -eq 0) {
        $blockers.Add('intended_deployment_missing')
    }

    return [pscustomobject]@{
        ready = $blockers.Count -eq 0
        selectedDeploymentNames = @($selected)
        blockers = @($blockers)
    }
}

function Test-FoundryManagedIdentityAttachment {
    param(
        [Parameter(Mandatory)][string] $ManagedIdentityClientId,
        [Parameter(Mandatory)][object] $ManagedIdentity,
        [Parameter(Mandatory)][object] $ContainerAppIdentity
    )

    $resourceIdProperty = $ManagedIdentity.PSObject.Properties['id']
    $clientIdProperty = $ManagedIdentity.PSObject.Properties['clientId']
    $principalIdProperty = $ManagedIdentity.PSObject.Properties['principalId']
    $resourceId = if ($null -ne $resourceIdProperty) {
        [string] $resourceIdProperty.Value
    }
    else {
        ''
    }
    $identityClientId = if ($null -ne $clientIdProperty) {
        [string] $clientIdProperty.Value
    }
    else {
        ''
    }
    $identityPrincipalId = if ($null -ne $principalIdProperty) {
        [string] $principalIdProperty.Value
    }
    else {
        ''
    }
    $attachmentProperty = $null
    $containerIdentitiesProperty =
        $ContainerAppIdentity.PSObject.Properties['userAssignedIdentities']
    if (
        $null -ne $containerIdentitiesProperty -and
        $null -ne $containerIdentitiesProperty.Value
    ) {
        foreach (
            $property in
                $containerIdentitiesProperty.Value.PSObject.Properties
        ) {
            if (
                $property.Name.Equals(
                    $resourceId,
                    [System.StringComparison]::OrdinalIgnoreCase
                )
            ) {
                $attachmentProperty = $property
                break
            }
        }
    }
    $attachedIdentity = if ($null -ne $attachmentProperty) {
        $attachmentProperty.Value
    }
    else {
        $null
    }
    $attachedClientIdProperty = if ($null -ne $attachedIdentity) {
        $attachedIdentity.PSObject.Properties['clientId']
    }
    else {
        $null
    }
    $attachedPrincipalIdProperty = if ($null -ne $attachedIdentity) {
        $attachedIdentity.PSObject.Properties['principalId']
    }
    else {
        $null
    }
    $attachedClientId = if ($null -ne $attachedClientIdProperty) {
        [string] $attachedClientIdProperty.Value
    }
    else {
        ''
    }
    $attachedPrincipalId = if ($null -ne $attachedPrincipalIdProperty) {
        [string] $attachedPrincipalIdProperty.Value
    }
    else {
        ''
    }
    $attached = (
        $identityClientId.Equals(
            $ManagedIdentityClientId,
            [System.StringComparison]::OrdinalIgnoreCase
        ) -and
        $null -ne $attachmentProperty -and
        $attachedClientId.Equals(
            $ManagedIdentityClientId,
            [System.StringComparison]::OrdinalIgnoreCase
        ) -and
        $attachedPrincipalId.Equals(
            $identityPrincipalId,
            [System.StringComparison]::OrdinalIgnoreCase
        )
    )

    return [pscustomobject]@{
        attached = $attached
        clientIdHash = Get-FoundrySha256Label `
            -Value $ManagedIdentityClientId.ToLowerInvariant()
        resourceIdHash = Get-FoundrySha256Label `
            -Value $resourceId.ToLowerInvariant()
        principalIdHash = Get-FoundrySha256Label `
            -Value $identityPrincipalId.ToLowerInvariant()
    }
}

function Test-FoundryRoleAssignment {
    param(
        [Parameter(Mandatory)][string] $PrincipalId,
        [Parameter(Mandatory)][string] $FoundryScope,
        [Parameter(Mandatory)][object[]] $Assignments
    )

    $roleDefinitionId = '53ca6127-db72-4b80-b1b0-d745d6d5456d'
    $normalizedTargetScope = $FoundryScope.TrimEnd('/')
    $match = $null
    foreach ($assignment in $Assignments) {
        $roleProperty = $assignment.PSObject.Properties['roleDefinitionId']
        $scopeProperty = $assignment.PSObject.Properties['scope']
        $principalProperty = $assignment.PSObject.Properties['principalId']
        $assignmentRoleId = if ($null -ne $roleProperty) {
            ([string] $roleProperty.Value).Split('/')[-1]
        }
        else {
            ''
        }
        $assignmentScope = if ($null -ne $scopeProperty) {
            ([string] $scopeProperty.Value).TrimEnd('/')
        }
        else {
            ''
        }
        $assignmentPrincipalId = if ($null -ne $principalProperty) {
            [string] $principalProperty.Value
        }
        else {
            ''
        }
        $principalMatches = $assignmentPrincipalId.Equals(
            $PrincipalId,
            [System.StringComparison]::OrdinalIgnoreCase
        )
        $roleMatches = $assignmentRoleId.Equals(
            $roleDefinitionId,
            [System.StringComparison]::OrdinalIgnoreCase
        )
        $scopeMatches = (
            -not [string]::IsNullOrWhiteSpace($assignmentScope) -and
            (
                $normalizedTargetScope.Equals(
                    $assignmentScope,
                    [System.StringComparison]::OrdinalIgnoreCase
                ) -or
                $normalizedTargetScope.StartsWith(
                    "$assignmentScope/",
                    [System.StringComparison]::OrdinalIgnoreCase
                )
            )
        )
        if ($principalMatches -and $roleMatches -and $scopeMatches) {
            $match = $assignment
            break
        }
    }

    return [pscustomobject]@{
        assigned = $null -ne $match
        roleDefinitionId = $roleDefinitionId
        scopeHash = if ($null -ne $match) {
            Get-FoundrySha256Label -Value (
                ([string] $match.scope).TrimEnd('/').ToLowerInvariant()
            )
        }
        else {
            $null
        }
    }
}

function Resolve-AzureResourceId {
    param([Parameter(Mandatory)][string] $Value)

    if (
        [string]::IsNullOrWhiteSpace($Value) -or
        -not $Value.StartsWith('/') -or
        $Value.Contains('?') -or
        $Value.Contains('#')
    ) {
        Throw-FoundryPrerequisite -Code 'azure_resource_id_invalid'
    }
    $segments = @(
        $Value.Split(
            '/',
            [System.StringSplitOptions]::RemoveEmptyEntries
        )
    )
    if (
        $segments.Count -lt 8 -or
        ($segments.Count - 6) % 2 -ne 0 -or
        -not $segments[0].Equals(
            'subscriptions',
            [System.StringComparison]::OrdinalIgnoreCase
        ) -or
        -not $segments[2].Equals(
            'resourceGroups',
            [System.StringComparison]::OrdinalIgnoreCase
        ) -or
        -not $segments[4].Equals(
            'providers',
            [System.StringComparison]::OrdinalIgnoreCase
        )
    ) {
        Throw-FoundryPrerequisite -Code 'azure_resource_id_invalid'
    }

    $subscriptionId = [guid]::Empty
    if (-not [guid]::TryParse($segments[1], [ref] $subscriptionId)) {
        Throw-FoundryPrerequisite -Code 'azure_resource_id_invalid'
    }
    foreach ($segment in $segments) {
        if (
            [string]::IsNullOrWhiteSpace($segment) -or
            $segment.Length -gt 260 -or
            $segment -match '[\x00-\x1f\x7f]'
        ) {
            Throw-FoundryPrerequisite -Code 'azure_resource_id_invalid'
        }
    }

    $pairs = [System.Collections.Generic.List[object]]::new()
    for ($index = 6; $index -lt $segments.Count; $index += 2) {
        $pairs.Add(
            [pscustomobject][ordered]@{
                type = $segments[$index]
                name = $segments[$index + 1]
            }
        )
    }
    return [pscustomobject][ordered]@{
        subscriptionId = $subscriptionId.ToString()
        resourceGroupName = $segments[3]
        providerNamespace = $segments[5]
        typeNamePairs = @($pairs)
        normalizedId = '/' + ($segments -join '/')
    }
}

function Invoke-FoundryAzCli {
    param([Parameter(Mandatory)][string[]] $Arguments)

    if ($null -eq (Get-Command az -ErrorAction SilentlyContinue)) {
        Throw-FoundryPrerequisite -Code 'azure_cli_unavailable'
    }
    $output = & az @Arguments 2>$null
    if ($LASTEXITCODE -ne 0) {
        Throw-FoundryPrerequisite -Code 'azure_cli_command_failed'
    }
    return ($output -join [Environment]::NewLine).Trim()
}

function Read-FoundryBoundedStream {
    param(
        [Parameter(Mandatory)][System.IO.Stream] $Stream,
        [ValidateRange(1, 16777216)][int] $MaximumBytes,
        [System.Threading.CancellationToken] $CancellationToken = (
            [System.Threading.CancellationToken]::None
        )
    )

    $buffer = [byte[]]::new([Math]::Min(8192, $MaximumBytes + 1))
    $memory = [System.IO.MemoryStream]::new()
    $totalBytes = 0
    try {
        while ($true) {
            $remainingBytes = $MaximumBytes - $totalBytes
            $readLimit = [Math]::Min($buffer.Length, $remainingBytes + 1)
            try {
                $bytesRead = $Stream.
                    ReadAsync(
                        $buffer,
                        0,
                        $readLimit,
                        $CancellationToken
                    ).
                    GetAwaiter().
                    GetResult()
            }
            catch [System.OperationCanceledException] {
                Throw-FoundryPrerequisite `
                    -Code 'deployment_inventory_timeout'
            }
            if ($bytesRead -eq 0) {
                break
            }
            if ($bytesRead -gt $remainingBytes) {
                Throw-FoundryPrerequisite `
                    -Code 'deployment_inventory_body_too_large'
            }
            $memory.Write($buffer, 0, $bytesRead)
            $totalBytes += $bytesRead
        }
        try {
            $utf8 = [System.Text.UTF8Encoding]::new($false, $true)
            return $utf8.GetString($memory.ToArray())
        }
        catch [System.Text.DecoderFallbackException] {
            Throw-FoundryPrerequisite `
                -Code 'deployment_inventory_body_invalid'
        }
    }
    finally {
        $memory.Dispose()
    }
}

function Invoke-FoundryDeploymentPage {
    param(
        [Parameter(Mandatory)][uri] $Uri,
        [Parameter(Mandatory)][string] $AccessToken,
        [ValidateRange(1024, 16777216)][int] $MaximumBytes = 1048576
    )

    $handler = [System.Net.Http.HttpClientHandler]::new()
    $handler.AllowAutoRedirect = $false
    $client = [System.Net.Http.HttpClient]::new($handler)
    $client.Timeout = [TimeSpan]::FromSeconds(30)
    $request = [System.Net.Http.HttpRequestMessage]::new(
        [System.Net.Http.HttpMethod]::Get,
        $Uri
    )
    $request.Headers.Authorization =
        [System.Net.Http.Headers.AuthenticationHeaderValue]::new(
            'Bearer',
            $AccessToken
        )
    $request.Headers.Accept.ParseAdd('application/json')

    try {
        try {
            $response = $client.Send(
                $request,
                [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead
            )
        }
        catch [System.Net.Http.HttpRequestException] {
            if (
                $null -ne $_.Exception.PSObject.Properties['HttpRequestError'] -and
                [string] $_.Exception.HttpRequestError -eq 'NameResolutionError'
            ) {
                Throw-FoundryPrerequisite -Code 'dns_unavailable'
            }
            Throw-FoundryPrerequisite -Code 'deployment_inventory_unavailable'
        }

        try {
            $statusCode = [int] $response.StatusCode
            if (-not $response.IsSuccessStatusCode) {
                switch ($statusCode) {
                    401 {
                        Throw-FoundryPrerequisite `
                            -Code 'tenant_access_unavailable'
                    }
                    403 {
                        Throw-FoundryPrerequisite `
                            -Code 'tenant_access_unavailable'
                    }
                    404 {
                        Throw-FoundryPrerequisite `
                            -Code 'project_endpoint_not_found'
                    }
                    429 {
                        Throw-FoundryPrerequisite `
                            -Code 'deployment_inventory_rate_limited'
                    }
                    default {
                        Throw-FoundryPrerequisite `
                            -Code "deployment_inventory_http_$statusCode"
                    }
                }
            }
            if (
                $response.Content.Headers.ContentLength.HasValue -and
                $response.Content.Headers.ContentLength.Value -gt $MaximumBytes
            ) {
                Throw-FoundryPrerequisite `
                    -Code 'deployment_inventory_body_too_large'
            }
            $stream = $null
            $contentDeadline =
                [System.Threading.CancellationTokenSource]::new()
            try {
                $contentDeadline.CancelAfter([TimeSpan]::FromSeconds(30))
                $stream = $response.Content.ReadAsStream()
                return Read-FoundryBoundedStream `
                    -Stream $stream `
                    -MaximumBytes $MaximumBytes `
                    -CancellationToken $contentDeadline.Token
            }
            finally {
                if ($null -ne $stream) {
                    $stream.Dispose()
                }
                $contentDeadline.Dispose()
            }
        }
        finally {
            if ($null -ne $response) {
                $response.Dispose()
            }
        }
    }
    finally {
        $request.Dispose()
        $client.Dispose()
        $handler.Dispose()
    }
}

function Write-FoundryPrerequisiteReport {
    param(
        [Parameter(Mandatory)]
        [object] $Report,

        [Parameter(Mandatory)]
        [string] $Path
    )

    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    $directory = [System.IO.Path]::GetDirectoryName($resolvedPath)
    if ([string]::IsNullOrWhiteSpace($directory)) {
        Throw-FoundryPrerequisite -Code 'report_path_invalid'
    }
    [System.IO.Directory]::CreateDirectory($directory) | Out-Null
    $json = $Report | ConvertTo-Json -Depth 32
    [System.IO.File]::WriteAllText(
        $resolvedPath,
        "$json`n",
        [System.Text.UTF8Encoding]::new($false)
    )
}

function Invoke-FoundryPrerequisiteCheck {
    param(
        [AllowEmptyString()]
        [string] $ProjectEndpointValue,

        [AllowEmptyString()]
        [string] $ManagedIdentityClientIdValue,

        [AllowEmptyString()]
        [string] $ContainerAppResourceIdValue,

        [AllowEmptyString()]
        [string] $FoundryRoleScopeValue,

        [AllowEmptyCollection()]
        [AllowEmptyString()]
        [string[]] $IntendedDeploymentNames,

        [Parameter(Mandatory)]
        [string] $ReportPath,

        [scriptblock] $AzInvoker,

        [scriptblock] $PageFetcher
    )

    $blockers = [System.Collections.Generic.List[string]]::new()
    $addBlocker = {
        param([string] $Code)
        if (-not $blockers.Contains($Code)) {
            $blockers.Add($Code)
        }
    }
    $readFailureCode = {
        param(
            [System.Exception] $Exception,
            [string] $Fallback
        )
        $prefix = 'foundry_prerequisite:'
        if ($Exception.Message.StartsWith($prefix)) {
            return $Exception.Message.Substring($prefix.Length)
        }
        return $Fallback
    }

    $validatedEndpoint = $null
    if ([string]::IsNullOrWhiteSpace($ProjectEndpointValue)) {
        & $addBlocker 'project_endpoint_missing'
    }
    else {
        try {
            $validatedEndpoint = Resolve-FoundryProjectEndpoint `
                -Value $ProjectEndpointValue
        }
        catch {
            & $addBlocker (
                & $readFailureCode `
                    $_.Exception `
                    'project_endpoint_invalid'
            )
        }
    }

    $managedIdentityGuid = [guid]::Empty
    if ([string]::IsNullOrWhiteSpace($ManagedIdentityClientIdValue)) {
        & $addBlocker 'managed_identity_client_id_missing'
    }
    elseif (
        -not [guid]::TryParse(
            $ManagedIdentityClientIdValue,
            [ref] $managedIdentityGuid
        )
    ) {
        & $addBlocker 'managed_identity_client_id_invalid'
    }

    $containerResource = $null
    if ([string]::IsNullOrWhiteSpace($ContainerAppResourceIdValue)) {
        & $addBlocker 'container_app_resource_id_missing'
    }
    else {
        try {
            $containerResource = Resolve-AzureResourceId `
                -Value $ContainerAppResourceIdValue
            if (
                -not $containerResource.providerNamespace.Equals(
                    'Microsoft.App',
                    [System.StringComparison]::OrdinalIgnoreCase
                ) -or
                $containerResource.typeNamePairs.Count -ne 1 -or
                -not $containerResource.typeNamePairs[0].type.Equals(
                    'containerApps',
                    [System.StringComparison]::OrdinalIgnoreCase
                )
            ) {
                & $addBlocker 'container_app_resource_id_invalid'
                $containerResource = $null
            }
        }
        catch {
            & $addBlocker 'container_app_resource_id_invalid'
        }
    }

    $foundryResource = $null
    if ([string]::IsNullOrWhiteSpace($FoundryRoleScopeValue)) {
        & $addBlocker 'foundry_role_scope_missing'
    }
    else {
        try {
            $foundryResource = Resolve-AzureResourceId `
                -Value $FoundryRoleScopeValue
            $pairs = @($foundryResource.typeNamePairs)
            $shapeIsValid = (
                $foundryResource.providerNamespace.Equals(
                    'Microsoft.CognitiveServices',
                    [System.StringComparison]::OrdinalIgnoreCase
                ) -and
                $pairs.Count -ge 1 -and
                $pairs.Count -le 2 -and
                $pairs[0].type.Equals(
                    'accounts',
                    [System.StringComparison]::OrdinalIgnoreCase
                ) -and
                (
                    $pairs.Count -eq 1 -or
                    $pairs[1].type.Equals(
                        'projects',
                        [System.StringComparison]::OrdinalIgnoreCase
                    )
                )
            )
            if (-not $shapeIsValid) {
                & $addBlocker 'foundry_role_scope_invalid'
                $foundryResource = $null
            }
        }
        catch {
            & $addBlocker 'foundry_role_scope_invalid'
        }
    }

    $intendedNames = @(
        $IntendedDeploymentNames |
            ForEach-Object { $_.Trim() } |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
            Sort-Object -Unique
    )
    if ($intendedNames.Count -eq 0) {
        & $addBlocker 'intended_deployments_missing'
    }

    if ($null -ne $validatedEndpoint -and $null -ne $foundryResource) {
        $endpointAccountName = $validatedEndpoint.DnsSafeHost.Split('.')[0]
        $scopeAccountName = $foundryResource.typeNamePairs[0].name
        if (
            -not $endpointAccountName.Equals(
                $scopeAccountName,
                [System.StringComparison]::OrdinalIgnoreCase
            )
        ) {
            & $addBlocker 'foundry_role_scope_endpoint_mismatch'
        }
        $projectName = [uri]::UnescapeDataString(
            $validatedEndpoint.AbsolutePath.Substring(
                '/api/projects/'.Length
            )
        )
        if ($projectName -eq '_project') {
            if ($foundryResource.typeNamePairs.Count -ne 1) {
                & $addBlocker 'foundry_role_scope_endpoint_mismatch'
            }
        }
        elseif (
            $foundryResource.typeNamePairs.Count -eq 2 -and
            -not $projectName.Equals(
                $foundryResource.typeNamePairs[1].name,
                [System.StringComparison]::Ordinal
            )
        ) {
            & $addBlocker 'foundry_role_scope_endpoint_mismatch'
        }
    }

    $azureContext = [pscustomobject][ordered]@{
        subscriptionIdHash = $null
        tenantIdHash = $null
    }
    $deploymentInventory = [pscustomobject][ordered]@{
        pageCount = 0
        inventoryHash = $null
        deployments = @()
    }
    $selection = [pscustomobject][ordered]@{
        ready = $false
        selectedDeploymentNames = @()
        blockers = @()
    }
    $identityCheck = [pscustomobject][ordered]@{
        attached = $false
        clientIdHash = if (
            -not [string]::IsNullOrWhiteSpace($ManagedIdentityClientIdValue)
        ) {
            Get-FoundrySha256Label `
                -Value $ManagedIdentityClientIdValue.ToLowerInvariant()
        }
        else {
            $null
        }
        resourceIdHash = $null
        principalIdHash = $null
    }
    $roleCheck = [pscustomobject][ordered]@{
        assigned = $false
        roleDefinitionId = '53ca6127-db72-4b80-b1b0-d745d6d5456d'
        scopeHash = $null
    }
    $tokenAcquired = $false
    $inventoryCaptured = $false

    if ($null -eq $AzInvoker) {
        $AzInvoker = {
            param([string[]] $Arguments)
            Invoke-FoundryAzCli -Arguments $Arguments
        }
    }

    $account = $null
    try {
        $accountArguments = [System.Collections.Generic.List[string]]::new()
        $accountArguments.Add('account')
        $accountArguments.Add('show')
        if ($null -ne $foundryResource) {
            $accountArguments.Add('--subscription')
            $accountArguments.Add($foundryResource.subscriptionId)
        }
        $accountArguments.Add('--output')
        $accountArguments.Add('json')
        $accountArguments.Add('--only-show-errors')
        $accountJson = & $AzInvoker -Arguments @($accountArguments)
        $account = $accountJson |
            ConvertFrom-Json -Depth 16 -NoEnumerate
        $accountSubscriptionId = [guid]::Empty
        $accountTenantId = [guid]::Empty
        $accountShapeIsValid = (
            $null -ne $account -and
            [guid]::TryParse(
                [string] $account.id,
                [ref] $accountSubscriptionId
            ) -and
            [guid]::TryParse(
                [string] $account.tenantId,
                [ref] $accountTenantId
            )
        )
        if (
            $accountShapeIsValid -and
            $null -ne $foundryResource
        ) {
            $accountShapeIsValid = (
                $accountSubscriptionId -eq
                    [guid] $foundryResource.subscriptionId
            )
        }
        if (-not $accountShapeIsValid) {
            throw 'invalid Azure account projection'
        }
        $azureContext = [pscustomobject][ordered]@{
            subscriptionIdHash = Get-FoundrySha256Label `
                -Value $accountSubscriptionId.ToString()
            tenantIdHash = Get-FoundrySha256Label `
                -Value $accountTenantId.ToString()
        }
    }
    catch {
        $account = $null
        & $addBlocker 'azure_cli_authentication_missing'
    }

    if ($blockers.Count -eq 0) {
        $accessToken = $null
        if ($null -ne $account) {
            try {
                $accessToken = [string] (
                    & $AzInvoker -Arguments @(
                        'account',
                        'get-access-token',
                        '--subscription',
                        $foundryResource.subscriptionId,
                        '--tenant',
                        [string] $account.tenantId,
                        '--scope',
                        'https://ai.azure.com/.default',
                        '--query',
                        'accessToken',
                        '--output',
                        'tsv',
                        '--only-show-errors'
                    )
                )
                if (
                    [string]::IsNullOrWhiteSpace($accessToken) -or
                    $accessToken.Length -lt 20
                ) {
                    throw 'empty token'
                }
                $tokenAcquired = $true
            }
            catch {
                & $addBlocker 'tenant_access_unavailable'
            }
        }

        if ($tokenAcquired) {
            try {
                if ($null -eq $PageFetcher) {
                    $pageReader = {
                        param([uri] $Uri)
                        Invoke-FoundryDeploymentPage `
                            -Uri $Uri `
                            -AccessToken $accessToken
                    }.GetNewClosure()
                }
                else {
                    $pageReader = {
                        param([uri] $Uri)
                        & $PageFetcher `
                            -Uri $Uri `
                            -AccessToken $accessToken
                    }.GetNewClosure()
                }
                $deploymentInventory = Get-FoundryDeploymentInventory `
                    -ProjectEndpoint $validatedEndpoint `
                    -PageFetcher $pageReader
                $inventoryCaptured = $true
                $selection = Test-FoundryDeploymentSelection `
                    -Deployments @($deploymentInventory.deployments) `
                    -IntendedDeploymentNames $intendedNames
                foreach ($selectionBlocker in $selection.blockers) {
                    & $addBlocker $selectionBlocker
                }
            }
            catch {
                & $addBlocker (
                    & $readFailureCode `
                        $_.Exception `
                        'deployment_inventory_unavailable'
                )
            }
            finally {
                $accessToken = $null
                Remove-Variable accessToken -ErrorAction SilentlyContinue
            }
        }

        $managedIdentity = $null
        if ($null -ne $account) {
            try {
                $identityJson = & $AzInvoker -Arguments @(
                    'identity',
                    'list',
                    '--subscription',
                    $containerResource.subscriptionId,
                    '--query',
                    "[?clientId=='$($managedIdentityGuid.ToString())']",
                    '--output',
                    'json',
                    '--only-show-errors'
                )
                $identities = @(
                    $identityJson |
                        ConvertFrom-Json -Depth 16
                )
                if ($identities.Count -eq 0) {
                    & $addBlocker 'managed_identity_not_found'
                }
                elseif ($identities.Count -gt 1) {
                    & $addBlocker 'managed_identity_ambiguous'
                }
                else {
                    $managedIdentity = $identities[0]
                }
            }
            catch {
                & $addBlocker 'managed_identity_lookup_unavailable'
            }
        }

        if ($null -ne $managedIdentity) {
            try {
                $containerIdentityJson = & $AzInvoker -Arguments @(
                    'containerapp',
                    'identity',
                    'show',
                    '--name',
                    $containerResource.typeNamePairs[0].name,
                    '--resource-group',
                    $containerResource.resourceGroupName,
                    '--subscription',
                    $containerResource.subscriptionId,
                    '--output',
                    'json',
                    '--only-show-errors'
                )
                $containerIdentity = $containerIdentityJson |
                    ConvertFrom-Json -Depth 32 -NoEnumerate
                $identityCheck = Test-FoundryManagedIdentityAttachment `
                    -ManagedIdentityClientId $managedIdentityGuid.ToString() `
                    -ManagedIdentity $managedIdentity `
                    -ContainerAppIdentity $containerIdentity
                if (-not $identityCheck.attached) {
                    & $addBlocker 'managed_identity_not_attached'
                }
            }
            catch {
                & $addBlocker 'container_app_identity_unavailable'
            }

            try {
                $roleJson = & $AzInvoker -Arguments @(
                    'role',
                    'assignment',
                    'list',
                    '--assignee-object-id',
                    [string] $managedIdentity.principalId,
                    '--scope',
                    $foundryResource.normalizedId,
                    '--include-inherited',
                    '--all',
                    '--subscription',
                    $foundryResource.subscriptionId,
                    '--fill-principal-name',
                    'false',
                    '--fill-role-definition-name',
                    'false',
                    '--output',
                    'json',
                    '--only-show-errors'
                )
                $assignments = @(
                    $roleJson |
                        ConvertFrom-Json -Depth 32
                )
                $roleCheck = Test-FoundryRoleAssignment `
                    -PrincipalId ([string] $managedIdentity.principalId) `
                    -FoundryScope $foundryResource.normalizedId `
                    -Assignments $assignments
                if (-not $roleCheck.assigned) {
                    & $addBlocker 'foundry_user_role_missing'
                }
            }
            catch {
                & $addBlocker 'foundry_role_lookup_unavailable'
            }
        }
    }

    $ready = $blockers.Count -eq 0
    $report = [pscustomobject][ordered]@{
        schemaVersion = 'forge.foundry-prerequisites.v1'
        generatedAt = [DateTimeOffset]::UtcNow.ToString('O')
        ready = $ready
        projectEndpoint = if ($null -ne $validatedEndpoint) {
            $validatedEndpoint.AbsoluteUri
        }
        else {
            $null
        }
        intendedDeploymentNames = $intendedNames
        selectedDeploymentNames = @($selection.selectedDeploymentNames)
        checks = [pscustomobject][ordered]@{
            endpointValidated = $null -ne $validatedEndpoint
            entraTokenAcquired = $tokenAcquired
            deploymentInventoryCaptured = $inventoryCaptured
            managedIdentityAttached = $identityCheck.attached
            foundryRoleAssigned = $roleCheck.assigned
        }
        azureContext = $azureContext
        managedIdentity = $identityCheck
        authorization = $roleCheck
        deploymentInventory = $deploymentInventory
        blockers = @($blockers | Sort-Object -Unique)
    }
    Write-FoundryPrerequisiteReport -Report $report -Path $ReportPath
    if ($ready) {
        return 0
    }
    return 2
}

if ($MyInvocation.InvocationName -ne '.') {
    $exitCode = Invoke-FoundryApiKeyPrerequisiteCheck `
        -ProjectEndpointValue $ProjectEndpoint `
        -ApiKeyEnvironmentVariable $ApiKeyEnvironmentVariable `
        -ProfilesPath $Profiles `
        -InventoryMode $InventoryMode `
        -BudgetMicrounits $BudgetMicrounits `
        -CredentialVersion $CredentialVersion `
        -ReportPath $OutputPath
    exit $exitCode
}
