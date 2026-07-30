Set-StrictMode -Version Latest

function Test-FoundryApiKeyShape {
    param([AllowNull()][string] $Value)

    if ($null -eq $Value -or $Value.Length -lt 16 -or $Value.Length -gt 512) {
        return $false
    }
    foreach ($character in $Value.ToCharArray()) {
        if ([int] $character -lt 0x21 -or [int] $character -gt 0x7e) {
            return $false
        }
    }
    return $true
}

function Get-FoundryApiKeyFromEnvironment {
    param([Parameter(Mandatory)][string] $VariableName)

    if ($VariableName -cnotmatch '^[A-Z_][A-Z0-9_]{0,127}$') {
        Throw-FoundryPrerequisite -Code 'api_key_environment_variable_invalid'
    }
    $value = [Environment]::GetEnvironmentVariable(
        $VariableName,
        [EnvironmentVariableTarget]::Process
    )
    if ([string]::IsNullOrWhiteSpace($value)) {
        Throw-FoundryPrerequisite -Code 'api_key_missing'
    }
    if (-not (Test-FoundryApiKeyShape -Value $value)) {
        Throw-FoundryPrerequisite -Code 'api_key_invalid'
    }
    return $value
}

function Get-FoundryCredentialVersionHash {
    param(
        [Parameter(Mandatory)][string] $ApiKeyEnvironmentVariable,
        [Parameter(Mandatory)][string] $CredentialVersion
    )

    if ($CredentialVersion -cnotmatch '^[A-Za-z0-9._-]{1,64}$') {
        Throw-FoundryPrerequisite -Code 'credential_version_invalid'
    }
    return Get-FoundrySha256Label `
        -Value "foundry-credential-version:$ApiKeyEnvironmentVariable`:$CredentialVersion"
}

function Get-FoundryProfileRoutes {
    param(
        [Parameter(Mandatory)][string] $Path,
        [ValidateRange(1024, 16777216)][int] $MaximumBytes = 1048576
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        Throw-FoundryPrerequisite -Code 'profiles_missing'
    }
    $item = Get-Item -LiteralPath $Path
    if ($item.Length -gt $MaximumBytes) {
        Throw-FoundryPrerequisite -Code 'profiles_too_large'
    }
    try {
        $document = Get-Content -Raw -LiteralPath $Path |
            ConvertFrom-Json -Depth 64 -NoEnumerate
    }
    catch {
        Throw-FoundryPrerequisite -Code 'profiles_json_invalid'
    }
    if (
        $null -eq $document -or
        $null -eq $document.PSObject.Properties['profiles'] -or
        $document.profiles -is [string]
    ) {
        Throw-FoundryPrerequisite -Code 'profiles_shape_invalid'
    }

    $routes = [System.Collections.Generic.List[string]]::new()
    foreach ($profile in @($document.profiles)) {
        if ($null -eq $profile) {
            Throw-FoundryPrerequisite -Code 'profile_record_invalid'
        }
        $enabledProperty = $profile.PSObject.Properties['enabled']
        if ($null -eq $enabledProperty -or $enabledProperty.Value -ne $true) {
            continue
        }
        $routeProperty = $profile.PSObject.Properties['deploymentName']
        if ($null -eq $routeProperty -or $routeProperty.Value -isnot [string]) {
            Throw-FoundryPrerequisite -Code 'profile_route_invalid'
        }
        $route = [string] $routeProperty.Value
        if (
            $route -cnotmatch '^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$' -or
            $route.Contains('do-not-deploy', [StringComparison]::OrdinalIgnoreCase)
        ) {
            Throw-FoundryPrerequisite -Code 'profile_route_invalid'
        }
        $routes.Add($route)
    }

    $unique = @($routes | Sort-Object -Unique)
    if ($unique.Count -eq 0) {
        Throw-FoundryPrerequisite -Code 'callable_profile_route_missing'
    }
    return $unique
}

function Read-FoundryHttpContent {
    param(
        [Parameter(Mandatory)][System.Net.Http.HttpContent] $Content,
        [ValidateRange(1, 16777216)][int] $MaximumBytes = 1048576
    )

    $stream = $Content.ReadAsStream()
    try {
        $buffer = [byte[]]::new(8192)
        $memory = [System.IO.MemoryStream]::new()
        try {
            while ($true) {
                $remaining = $MaximumBytes - [int] $memory.Length
                $read = $stream.Read($buffer, 0, [Math]::Min($buffer.Length, $remaining + 1))
                if ($read -eq 0) {
                    break
                }
                if ($read -gt $remaining) {
                    Throw-FoundryPrerequisite -Code 'http_body_too_large'
                }
                $memory.Write($buffer, 0, $read)
            }
            return [Text.Encoding]::UTF8.GetString($memory.ToArray())
        }
        finally {
            $memory.Dispose()
        }
    }
    finally {
        $stream.Dispose()
    }
}

function Invoke-FoundryApiKeyRequest {
    param(
        [Parameter(Mandatory)][ValidateSet('GET', 'POST')][string] $Method,
        [Parameter(Mandatory)][uri] $Uri,
        [Parameter(Mandatory)][string] $ApiKey,
        [AllowNull()][string] $Body,
        [ValidateRange(1, 16777216)][int] $MaximumBytes = 1048576
    )

    if ($Uri.Scheme -cne 'https' -or -not $Uri.IsDefaultPort) {
        Throw-FoundryPrerequisite -Code 'request_uri_invalid'
    }

    $handler = [System.Net.Http.SocketsHttpHandler]::new()
    $handler.AllowAutoRedirect = $false
    $handler.AutomaticDecompression =
        [System.Net.DecompressionMethods]::GZip -bor
        [System.Net.DecompressionMethods]::Deflate
    $handler.ConnectTimeout = [TimeSpan]::FromSeconds(10)
    $handler.UseCookies = $false
    $handler.UseProxy = $false
    $client = [System.Net.Http.HttpClient]::new($handler, $true)
    $client.Timeout = [TimeSpan]::FromSeconds(30)
    $request = [System.Net.Http.HttpRequestMessage]::new(
        [System.Net.Http.HttpMethod]::new($Method),
        $Uri
    )
    try {
        if (-not $request.Headers.TryAddWithoutValidation('api-key', $ApiKey)) {
            Throw-FoundryPrerequisite -Code 'api_key_header_rejected'
        }
        $request.Headers.TryAddWithoutValidation(
            'x-ms-client-request-id',
            [guid]::NewGuid().ToString('D')
        ) | Out-Null
        $request.Headers.TryAddWithoutValidation(
            'User-Agent',
            'sapphirus-foundry-prerequisite/1'
        ) | Out-Null
        if ($Method -ceq 'POST') {
            $request.Content = [System.Net.Http.StringContent]::new(
                ($Body ?? ''),
                [Text.Encoding]::UTF8,
                'application/json'
            )
        }
        $response = $client.Send(
            $request,
            [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead
        )
        try {
            $content = Read-FoundryHttpContent `
                -Content $response.Content `
                -MaximumBytes $MaximumBytes
            $requestId = $null
            foreach ($name in @('x-ms-request-id', 'apim-request-id', 'request-id')) {
                [System.Collections.Generic.IEnumerable[string]] $values = $null
                if ($response.Headers.TryGetValues($name, [ref] $values)) {
                    $requestId = @($values)[0]
                    break
                }
            }
            return [pscustomobject][ordered]@{
                statusCode = [int] $response.StatusCode
                body = $content
                requestIdHash = if ([string]::IsNullOrWhiteSpace($requestId)) {
                    $null
                }
                else {
                    Get-FoundrySha256Label -Value $requestId
                }
            }
        }
        finally {
            $response.Dispose()
        }
    }
    finally {
        $request.Dispose()
        $client.Dispose()
    }
}

function ConvertFrom-FoundryModelsList {
    param(
        [Parameter(Mandatory)][string] $Body,
        [ValidateRange(1, 4096)][int] $MaximumModels = 1024
    )

    try {
        $document = $Body | ConvertFrom-Json -Depth 32 -NoEnumerate
    }
    catch {
        Throw-FoundryPrerequisite -Code 'models_json_invalid'
    }
    if (
        $null -eq $document -or
        $null -eq $document.PSObject.Properties['data']
    ) {
        Throw-FoundryPrerequisite -Code 'models_shape_invalid'
    }
    $ids = [System.Collections.Generic.List[string]]::new()
    foreach ($record in @($document.data)) {
        if ($ids.Count -ge $MaximumModels) {
            Throw-FoundryPrerequisite -Code 'models_limit_exceeded'
        }
        $idProperty = $record.PSObject.Properties['id']
        if ($null -eq $idProperty -or $idProperty.Value -isnot [string]) {
            Throw-FoundryPrerequisite -Code 'model_record_invalid'
        }
        $id = [string] $idProperty.Value
        if ($id -cnotmatch '^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$') {
            Throw-FoundryPrerequisite -Code 'model_record_invalid'
        }
        $ids.Add($id)
    }
    $unique = @($ids | Sort-Object -Unique)
    if ($unique.Count -ne $ids.Count) {
        Throw-FoundryPrerequisite -Code 'model_duplicate'
    }
    return $unique
}

function Invoke-FoundryApiKeyPrerequisiteCheck {
    param(
        [AllowNull()][string] $ProjectEndpointValue,
        [Parameter(Mandatory)][string] $ApiKeyEnvironmentVariable,
        [Parameter(Mandatory)][string] $ProfilesPath,
        [ValidateSet(
            'configured_and_probed',
            'openai_models',
            'project_deployments_api_key'
        )]
        [string] $InventoryMode = 'configured_and_probed',
        [ValidateRange(0, 1000000000)][long] $BudgetMicrounits = 5000,
        [string] $CredentialVersion = 'unversioned',
        [Parameter(Mandatory)][string] $ReportPath,
        [scriptblock] $RequestSender
    )

    $blockers = [System.Collections.Generic.List[string]]::new()
    $addBlocker = {
        param([string] $Code)
        if (-not $blockers.Contains($Code)) {
            $blockers.Add($Code)
        }
    }
    $failureCode = {
        param([System.Exception] $Exception, [string] $Fallback)
        $prefix = 'foundry_prerequisite:'
        if ($Exception.Message.StartsWith($prefix, [StringComparison]::Ordinal)) {
            return $Exception.Message.Substring($prefix.Length)
        }
        return $Fallback
    }

    $endpoint = $null
    if ([string]::IsNullOrWhiteSpace($ProjectEndpointValue)) {
        & $addBlocker 'project_endpoint_missing'
    }
    else {
        try {
            $endpoint = Resolve-FoundryProjectEndpoint -Value $ProjectEndpointValue
        }
        catch {
            & $addBlocker (& $failureCode $_.Exception 'project_endpoint_invalid')
        }
    }

    $credentialValue = $null
    try {
        $credentialValue = Get-FoundryApiKeyFromEnvironment `
            -VariableName $ApiKeyEnvironmentVariable
    }
    catch {
        & $addBlocker (& $failureCode $_.Exception 'api_key_invalid')
    }

    $credentialVersionHash = $null
    try {
        $credentialVersionHash = Get-FoundryCredentialVersionHash `
            -ApiKeyEnvironmentVariable $ApiKeyEnvironmentVariable `
            -CredentialVersion $CredentialVersion
    }
    catch {
        & $addBlocker (& $failureCode $_.Exception 'credential_version_invalid')
    }

    $profileRoutes = @()
    try {
        $profileRoutes = @(
            Get-FoundryProfileRoutes -Path $ProfilesPath
        )
    }
    catch {
        & $addBlocker (& $failureCode $_.Exception 'profiles_invalid')
    }

    if ($null -eq $RequestSender) {
        $RequestSender = {
            param(
                [string] $Method,
                [uri] $Uri,
                [string] $ApiKeyValue,
                [AllowNull()][string] $Body,
                [int] $MaximumBytes
            )
            Invoke-FoundryApiKeyRequest `
                -Method $Method `
                -Uri $Uri `
                -ApiKey $ApiKeyValue `
                -Body $Body `
                -MaximumBytes $MaximumBytes
        }
    }

    $models = @()
    $selectedRoutes = @()
    $observations = [ordered]@{
        modelsStatus = $null
        deploymentStatus = $null
        routeProbeCount = 0
        responsesCanaryCount = 0
        providerRequestIdHashes = @()
    }

    if (
        $null -ne $endpoint -and
        $null -ne $credentialValue -and
        $profileRoutes.Count -gt 0
    ) {
        try {
            $modelsUri = [uri]::new($endpoint, 'openai/v1/models')
            $modelsResponse = & $RequestSender `
                -Method 'GET' `
                -Uri $modelsUri `
                -ApiKeyValue $credentialValue `
                -Body $null `
                -MaximumBytes 1048576
            $observations.modelsStatus = [int] $modelsResponse.statusCode
            if ($null -ne $modelsResponse.requestIdHash) {
                $observations.providerRequestIdHashes += $modelsResponse.requestIdHash
            }
            if ([int] $modelsResponse.statusCode -in @(401, 403)) {
                & $addBlocker 'api_key_rejected'
            }
            elseif ([int] $modelsResponse.statusCode -eq 200) {
                $models = @(ConvertFrom-FoundryModelsList -Body $modelsResponse.body)
            }
            elseif ($InventoryMode -ceq 'openai_models') {
                & $addBlocker 'openai_models_unavailable'
            }
        }
        catch {
            & $addBlocker (& $failureCode $_.Exception 'models_unavailable')
        }

        if ($InventoryMode -ceq 'configured_and_probed') {
            foreach ($route in $profileRoutes) {
                try {
                    $escapedRoute = [uri]::EscapeDataString($route)
                    $probeUri = [uri]::new($endpoint, "openai/v1/models/$escapedRoute")
                    $probe = & $RequestSender `
                        -Method 'GET' `
                        -Uri $probeUri `
                        -ApiKeyValue $credentialValue `
                        -Body $null `
                        -MaximumBytes 262144
                    $observations.routeProbeCount += 1
                    if ([int] $probe.statusCode -in @(401, 403)) {
                        & $addBlocker 'api_key_rejected'
                    }
                    elseif ([int] $probe.statusCode -eq 200) {
                        $selectedRoutes += $route
                    }
                    elseif ([int] $probe.statusCode -in @(404, 405)) {
                        if ($BudgetMicrounits -le 0) {
                            & $addBlocker 'responses_canary_budget_unavailable'
                            continue
                        }
                        $canaryUri = [uri]::new($endpoint, 'openai/v1/responses')
                        $canaryBody = [ordered]@{
                            model = $route
                            input = 'Reply with OK.'
                            max_output_tokens = 1
                            store = $false
                        } | ConvertTo-Json -Compress
                        $canary = & $RequestSender `
                            -Method 'POST' `
                            -Uri $canaryUri `
                            -ApiKeyValue $credentialValue `
                            -Body $canaryBody `
                            -MaximumBytes 262144
                        $observations.responsesCanaryCount += 1
                        if ([int] $canary.statusCode -ge 200 -and
                            [int] $canary.statusCode -lt 300) {
                            $selectedRoutes += $route
                        }
                        elseif ([int] $canary.statusCode -in @(401, 403)) {
                            & $addBlocker 'api_key_rejected'
                        }
                        else {
                            & $addBlocker 'profile_route_not_callable'
                        }
                    }
                    else {
                        & $addBlocker 'profile_route_not_callable'
                    }
                }
                catch {
                    & $addBlocker (& $failureCode $_.Exception 'profile_route_probe_failed')
                }
            }
        }
        elseif ($InventoryMode -ceq 'openai_models') {
            foreach ($route in $profileRoutes) {
                if ($models -ccontains $route) {
                    $selectedRoutes += $route
                }
                else {
                    & $addBlocker 'profile_route_missing_from_models'
                }
            }
        }
        else {
            try {
                $pageFetcher = {
                    param([uri] $Uri)
                    $response = & $RequestSender `
                        -Method 'GET' `
                        -Uri $Uri `
                        -ApiKeyValue $credentialValue `
                        -Body $null `
                        -MaximumBytes 1048576
                    $observations.deploymentStatus = [int] $response.statusCode
                    if ([int] $response.statusCode -in @(401, 403)) {
                        Throw-FoundryPrerequisite -Code 'api_key_rejected'
                    }
                    if ([int] $response.statusCode -ne 200) {
                        Throw-FoundryPrerequisite -Code 'deployment_inventory_unavailable'
                    }
                    return [string] $response.body
                }.GetNewClosure()
                $inventory = Get-FoundryDeploymentInventory `
                    -ProjectEndpoint $endpoint `
                    -PageFetcher $pageFetcher
                $names = @($inventory.deployments | ForEach-Object deploymentName)
                foreach ($route in $profileRoutes) {
                    if ($names -ccontains $route) {
                        $selectedRoutes += $route
                    }
                    else {
                        & $addBlocker 'profile_route_missing_from_deployments'
                    }
                }
            }
            catch {
                & $addBlocker (
                    & $failureCode $_.Exception 'deployment_inventory_unavailable'
                )
            }
        }
    }

    if (
        $profileRoutes.Count -eq 0 -or
        $selectedRoutes.Count -ne $profileRoutes.Count
    ) {
        & $addBlocker 'callable_profile_route_missing'
    }

    $canonicalRoutes = @($selectedRoutes | Sort-Object -Unique)
    $routeEvidence = [ordered]@{
        inventoryMode = $InventoryMode
        routeCount = $canonicalRoutes.Count
        routeHashes = @(
            $canonicalRoutes | ForEach-Object {
                Get-FoundrySha256Label -Value "foundry-route:$_"
            }
        )
    }
    $routeEvidenceHash = Get-FoundrySha256Label `
        -Value ($routeEvidence | ConvertTo-Json -Depth 4 -Compress)
    $deploymentInventory = [ordered]@{
        pageCount = $null
        inventoryHash = $routeEvidenceHash
        deployments = @(
            $canonicalRoutes | ForEach-Object {
                [ordered]@{ deploymentName = $_ }
            }
        )
    }

    $report = [ordered]@{
        schemaVersion = 'forge.foundry-prerequisites.v2'
        generatedAt = [DateTimeOffset]::UtcNow.ToString('O')
        ready = $blockers.Count -eq 0
        projectEndpointHash = if ($null -eq $endpoint) {
            $null
        }
        else {
            Get-FoundrySha256Label -Value $endpoint.AbsoluteUri
        }
        credential = [ordered]@{
            mechanism = 'api_key'
            environmentVariableHash = Get-FoundrySha256Label `
                -Value $ApiKeyEnvironmentVariable
            versionHash = $credentialVersionHash
            rawKeyPersisted = $false
        }
        selectedInventoryMode = $InventoryMode
        routeEvidenceHash = $routeEvidenceHash
        routeEvidence = $routeEvidence
        deploymentInventory = $deploymentInventory
        observations = $observations
        checks = [ordered]@{
            endpointValidated = $null -ne $endpoint
            apiKeyPresent = $null -ne $credentialValue
            profilesValidated = $profileRoutes.Count -gt 0
            responsesCompatibleRouteProved =
                $canonicalRoutes.Count -eq $profileRoutes.Count -and
                $canonicalRoutes.Count -gt 0
            exactlyOneInventoryModeSelected = $true
            foundryEntraTokenAcquired = $false
        }
        blockers = @($blockers | Sort-Object -Unique)
    }

    Write-FoundryPrerequisiteReport -Report $report -Path $ReportPath
    $credentialValue = $null
    Remove-Variable credentialValue -ErrorAction SilentlyContinue
    if ($report.ready) {
        return 0
    }
    return 2
}
