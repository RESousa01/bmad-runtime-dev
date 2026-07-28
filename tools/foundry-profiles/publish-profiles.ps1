[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
param(
    [string] $ProfilePath = (
        Join-Path `
            $PSScriptRoot `
            '..\..\infra\desktop-support\model-profiles.example.json'
    ),
    [string] $DeploymentPath = (
        Join-Path `
            $PSScriptRoot `
            '..\..\artifacts\foundry\prerequisites.json'
    ),
    [string] $OutputPath = (
        Join-Path `
            $PSScriptRoot `
            '..\..\artifacts\foundry\infra\profile-dry-run.json'
    ),
    [string] $Label = 'production',
    [string] $StoreEndpoint = $env:FORGE_APP_CONFIGURATION_ENDPOINT,
    [ValidateRange(0, 2147483647)]
    [int] $RollbackToRegistryVersion = 0,
    [switch] $Approve
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

<#
    Publishes a validated model profile set to Azure App Configuration.

    Design rules this file exists to enforce:

      * -WhatIf performs no writes at all and is the default way to inspect a
        change. A dry run reads, validates, and plans; it never sets a key.
      * A write additionally requires explicit approval: -Approve, or the CI
        variable FORGE_MODEL_PROFILE_PUBLISH_APPROVAL set to 'approved'.
      * Every write is conditional. An existing key is written with If-Match on
        the ETag observed while planning; a new key is written with
        If-None-Match: *. A concurrent operator change therefore fails the
        publish instead of being overwritten.
      * The registry-version key is written last, so a reader never observes a
        partially published set. Profile keys are never deleted by this tool:
        the registry record's profileIds list is the manifest, and a profile
        key absent from it is inert.
      * Nothing here can deploy a model or grant a role. Every store
        interaction passes Assert-ProfileStoreOperation, whose allowlist is
        closed over list/get/set on the App Configuration data plane, and the
        only Azure CLI commands reachable are two read-only token commands.
      * No token, connection string, endpoint, or key value reaches the report
        or the console. Endpoints and ETags appear only as SHA-256 labels.

    Exit codes: 0 completed as intended, 2 blocked or refused.
#>

$script:ProfileKeyPrefix = 'desktop-support:model-profile:'
$script:RegistryVersionKey = 'desktop-support:model-profile-registry:version'
$script:ProfileHashPurpose = 'sapphirus:model-profile:v1'
$script:RegistryHashPurpose = 'sapphirus:model-profile-registry:v1'
$script:RegistryRecordPurpose = 'sapphirus:model-profile-registry-record:v1'
$script:RegistryRecordSchema = 'forge.model-profile-registry.v1'
$script:ApprovalValue = 'approved'
$script:StoreApiVersion = '2023-11-01'
$script:StoreContentType = 'application/vnd.microsoft.appconfig.kv+json'
$script:AllowedStoreOperations = @('list', 'get', 'set')
$script:AllowedAzCommands = @('account show', 'account get-access-token')

function Throw-ProfilePublish {
    param([Parameter(Mandatory)][string] $Code)
    throw "profile_publish:$Code"
}

function Get-ProfileSha256Label {
    param([Parameter(Mandatory)][AllowEmptyString()][string] $Value)
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($Value)
    $hash = [System.Security.Cryptography.SHA256]::HashData($bytes)
    return "sha256:$([Convert]::ToHexString($hash).ToLowerInvariant())"
}

<#
    Reproduces packages/contracts/scripts/lib/canonical-json.mjs canonicalHash
    for a value that is already canonical JSON. Stored values are written from
    the validator's canonical serialization, so hashing them here yields the
    digest the validator computed without a second canonicalizer existing in
    this language.
#>
function Get-ProfileValueHash {
    param(
        [Parameter(Mandatory)][string] $CanonicalJson,
        [Parameter(Mandatory)][string] $Purpose
    )
    return Get-ProfileSha256Label -Value "$Purpose`n$CanonicalJson"
}

function Test-ProfileLabelName {
    param([Parameter(Mandatory)][AllowEmptyString()][string] $Value)
    return $Value -cmatch '^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$'
}

function Test-ProfileIdentifier {
    param([Parameter(Mandatory)][AllowEmptyString()][string] $Value)
    return $Value -cmatch '^[a-z][a-z0-9]*(-[a-z0-9]+){0,7}$'
}

function Test-ProfileSha256Label {
    param([Parameter(Mandatory)][AllowEmptyString()][string] $Value)
    return $Value -cmatch '^sha256:[0-9a-f]{64}$'
}

function Get-ProfileArchiveLabel {
    param(
        [Parameter(Mandatory)][string] $Label,
        [Parameter(Mandatory)][ValidateRange(1, 2147483647)][int] $RegistryVersion
    )

    if (-not (Test-ProfileLabelName -Value $Label)) {
        Throw-ProfilePublish -Code 'label_invalid'
    }
    $archive = "$Label-v$RegistryVersion"
    if (-not (Test-ProfileLabelName -Value $archive)) {
        Throw-ProfilePublish -Code 'label_invalid'
    }
    return $archive
}

<#
    A deliberately tiny canonical writer for the one record shape this tool
    builds without the validator: the registry record rebuilt during a
    rollback. Every input is constrained to a printable ASCII alphabet that
    needs no JSON escaping, so a general serializer is never required. Member
    order is the RFC 8785 ordering of these exact names.
#>
function ConvertTo-ProfileCanonicalString {
    param([Parameter(Mandatory)][AllowEmptyString()][string] $Value)

    if ($Value -cnotmatch '^[A-Za-z0-9:._\- ]{1,256}$') {
        Throw-ProfilePublish -Code 'canonical_value_unrepresentable'
    }
    return '"' + $Value + '"'
}

function New-ProfileRegistryRecord {
    param(
        [Parameter(Mandatory)][ValidateRange(1, 2147483647)][int] $RegistryVersion,
        [Parameter(Mandatory)][string] $CapabilityHash,
        [Parameter(Mandatory)][object[]] $ProfileHashPairs
    )

    if (-not (Test-ProfileSha256Label -Value $CapabilityHash)) {
        Throw-ProfilePublish -Code 'capability_hash_invalid'
    }
    $ordered = @(
        $ProfileHashPairs |
            Sort-Object -CaseSensitive -Property { [string] $_.profileId }
    )
    foreach ($pair in $ordered) {
        if (
            -not (Test-ProfileIdentifier -Value ([string] $pair.profileId)) -or
            -not (Test-ProfileSha256Label -Value ([string] $pair.profileHash))
        ) {
            Throw-ProfilePublish -Code 'registry_pair_invalid'
        }
    }
    $profileIds = @($ordered | ForEach-Object { [string] $_.profileId })
    $uniqueIds = @($profileIds | Sort-Object -CaseSensitive -Unique)
    if ($uniqueIds.Count -ne $profileIds.Count) {
        Throw-ProfilePublish -Code 'registry_pair_duplicate'
    }

    $hashMembers = @(
        $ordered |
            ForEach-Object {
                '{"profileHash":' +
                (ConvertTo-ProfileCanonicalString -Value ([string] $_.profileHash)) +
                ',"profileId":' +
                (ConvertTo-ProfileCanonicalString -Value ([string] $_.profileId)) +
                '}'
            }
    )
    $hashPreimageValue = '{"profiles":[' + ($hashMembers -join ',') +
        '],"registryVersion":' + $RegistryVersion + '}'
    $registryHash = Get-ProfileValueHash `
        -CanonicalJson $hashPreimageValue `
        -Purpose $script:RegistryHashPurpose

    $idMembers = @(
        $profileIds | ForEach-Object { ConvertTo-ProfileCanonicalString -Value $_ }
    )
    $value = '{"capabilityHash":' +
        (ConvertTo-ProfileCanonicalString -Value $CapabilityHash) +
        ',"profileIds":[' + ($idMembers -join ',') +
        '],"registryHash":' +
        (ConvertTo-ProfileCanonicalString -Value $registryHash) +
        ',"registryVersion":' + $RegistryVersion +
        ',"schemaVersion":' +
        (ConvertTo-ProfileCanonicalString -Value $script:RegistryRecordSchema) +
        '}'

    return [pscustomobject][ordered]@{
        registryVersion = $RegistryVersion
        registryHash = $registryHash
        capabilityHash = $CapabilityHash
        profileIds = $profileIds
        value = $value
    }
}

function Read-ProfileRegistryRecord {
    param([Parameter(Mandatory)][AllowEmptyString()][string] $Value)

    if ([string]::IsNullOrWhiteSpace($Value) -or $Value.Length -gt 65536) {
        Throw-ProfilePublish -Code 'registry_record_invalid'
    }
    $record = $null
    try {
        $record = $Value | ConvertFrom-Json -Depth 8 -NoEnumerate
    }
    catch {
        Throw-ProfilePublish -Code 'registry_record_invalid'
    }
    if ($record -isnot [System.Management.Automation.PSCustomObject]) {
        Throw-ProfilePublish -Code 'registry_record_invalid'
    }
    $required = @(
        'schemaVersion',
        'registryVersion',
        'registryHash',
        'capabilityHash',
        'profileIds'
    )
    foreach ($name in $required) {
        if ($null -eq $record.PSObject.Properties[$name]) {
            Throw-ProfilePublish -Code 'registry_record_invalid'
        }
    }
    if (([string] $record.schemaVersion) -cne $script:RegistryRecordSchema) {
        Throw-ProfilePublish -Code 'registry_record_invalid'
    }
    $registryVersion = 0
    $parsed = [int]::TryParse(
        [string] $record.registryVersion,
        [ref] $registryVersion
    )
    if (-not $parsed -or $registryVersion -lt 1) {
        Throw-ProfilePublish -Code 'registry_record_invalid'
    }
    if (
        -not (Test-ProfileSha256Label -Value ([string] $record.registryHash)) -or
        -not (Test-ProfileSha256Label -Value ([string] $record.capabilityHash))
    ) {
        Throw-ProfilePublish -Code 'registry_record_invalid'
    }
    $profileIds = @($record.profileIds | ForEach-Object { [string] $_ })
    foreach ($profileId in $profileIds) {
        if (-not (Test-ProfileIdentifier -Value $profileId)) {
            Throw-ProfilePublish -Code 'registry_record_invalid'
        }
    }

    return [pscustomobject][ordered]@{
        registryVersion = $registryVersion
        registryHash = [string] $record.registryHash
        capabilityHash = [string] $record.capabilityHash
        profileIds = $profileIds
    }
}

<#
    The single choke point through which every store interaction passes. It is
    what makes "this tool cannot deploy a model or grant a role" a property of
    the code rather than a claim: only list, get, and set against two key
    shapes exist, and nothing here can construct an ARM request.
#>
function Assert-ProfileStoreOperation {
    param([Parameter(Mandatory)][hashtable] $Request)

    if (-not $Request.ContainsKey('operation')) {
        Throw-ProfilePublish -Code 'store_operation_forbidden'
    }
    $operation = [string] $Request['operation']
    if ($script:AllowedStoreOperations -cnotcontains $operation) {
        Throw-ProfilePublish -Code 'store_operation_forbidden'
    }
    if (-not $Request.ContainsKey('label')) {
        Throw-ProfilePublish -Code 'label_invalid'
    }
    if (-not (Test-ProfileLabelName -Value ([string] $Request['label']))) {
        Throw-ProfilePublish -Code 'label_invalid'
    }
    if ($Request.ContainsKey('keyFilter')) {
        if (([string] $Request['keyFilter']) -cne "$($script:ProfileKeyPrefix)*") {
            Throw-ProfilePublish -Code 'store_key_forbidden'
        }
    }
    if ($Request.ContainsKey('key')) {
        $key = [string] $Request['key']
        $isProfileKey = $false
        if (
            $key.StartsWith(
                $script:ProfileKeyPrefix,
                [System.StringComparison]::Ordinal
            )
        ) {
            $isProfileKey = Test-ProfileIdentifier -Value (
                $key.Substring($script:ProfileKeyPrefix.Length)
            )
        }
        if (-not $isProfileKey -and $key -cne $script:RegistryVersionKey) {
            Throw-ProfilePublish -Code 'store_key_forbidden'
        }
    }
    if ($operation -ceq 'set') {
        if (-not $Request.ContainsKey('key') -or -not $Request.ContainsKey('value')) {
            Throw-ProfilePublish -Code 'store_operation_forbidden'
        }
        $hasIfMatch = (
            $Request.ContainsKey('ifMatch') -and
            -not [string]::IsNullOrWhiteSpace([string] $Request['ifMatch'])
        )
        $hasIfNoneMatch = (
            $Request.ContainsKey('ifNoneMatch') -and
            ([string] $Request['ifNoneMatch']) -ceq '*'
        )
        if ($hasIfMatch -eq $hasIfNoneMatch) {
            # Exactly one precondition, always. An unconditional write is the
            # thing that silently overwrites a concurrent operator change.
            Throw-ProfilePublish -Code 'store_write_unconditional'
        }
    }
    return $true
}

function Invoke-ProfileStore {
    param(
        [Parameter(Mandatory)][scriptblock] $StoreInvoker,
        [Parameter(Mandatory)][hashtable] $Request
    )

    Assert-ProfileStoreOperation -Request $Request | Out-Null
    return (& $StoreInvoker $Request)
}

function Resolve-ProfileStoreEndpoint {
    param([Parameter(Mandatory)][string] $Value)

    $parsed = $null
    if (-not [uri]::TryCreate($Value, [System.UriKind]::Absolute, [ref] $parsed)) {
        Throw-ProfilePublish -Code 'store_endpoint_invalid'
    }
    if ($parsed.Scheme -cne 'https') {
        Throw-ProfilePublish -Code 'store_endpoint_scheme_invalid'
    }
    if (-not $parsed.IsDefaultPort -or $parsed.Port -ne 443) {
        Throw-ProfilePublish -Code 'store_endpoint_port_invalid'
    }
    if (
        -not [string]::IsNullOrEmpty($parsed.UserInfo) -or
        -not [string]::IsNullOrEmpty($parsed.Query) -or
        -not [string]::IsNullOrEmpty($parsed.Fragment)
    ) {
        Throw-ProfilePublish -Code 'store_endpoint_invalid'
    }
    $hostSuffix = '.azconfig.io'
    if (
        -not $parsed.DnsSafeHost.EndsWith(
            $hostSuffix,
            [System.StringComparison]::OrdinalIgnoreCase
        ) -or
        $parsed.DnsSafeHost.Length -le $hostSuffix.Length
    ) {
        Throw-ProfilePublish -Code 'store_endpoint_host_invalid'
    }
    if ($parsed.AbsolutePath -cne '/') {
        Throw-ProfilePublish -Code 'store_endpoint_path_invalid'
    }

    $builder = [System.UriBuilder]::new(
        'https',
        $parsed.DnsSafeHost.ToLowerInvariant()
    )
    $builder.Port = -1
    $builder.Path = '/'
    return $builder.Uri
}

function Invoke-ProfileAzCli {
    param([Parameter(Mandatory)][string[]] $Arguments)

    $command = $Arguments -join ' '
    $permitted = $false
    foreach ($allowed in $script:AllowedAzCommands) {
        if ($command.StartsWith($allowed, [System.StringComparison]::Ordinal)) {
            $permitted = $true
            break
        }
    }
    if (-not $permitted) {
        # Anything that could create a deployment or write a role assignment
        # dies here, before a process is started.
        Throw-ProfilePublish -Code 'azure_command_forbidden'
    }
    if ($null -eq (Get-Command az -ErrorAction SilentlyContinue)) {
        Throw-ProfilePublish -Code 'azure_cli_unavailable'
    }
    $output = & az @Arguments 2>$null
    if ($LASTEXITCODE -ne 0) {
        Throw-ProfilePublish -Code 'azure_cli_command_failed'
    }
    return ($output -join [Environment]::NewLine).Trim()
}

function Read-ProfileBoundedStream {
    param(
        [Parameter(Mandatory)][System.IO.Stream] $Stream,
        [ValidateRange(1, 16777216)][int] $MaximumBytes = 1048576
    )

    $buffer = [byte[]]::new([Math]::Min(8192, $MaximumBytes + 1))
    $memory = [System.IO.MemoryStream]::new()
    $totalBytes = 0
    try {
        while ($true) {
            $remainingBytes = $MaximumBytes - $totalBytes
            $readLimit = [Math]::Min($buffer.Length, $remainingBytes + 1)
            $bytesRead = $Stream.Read($buffer, 0, $readLimit)
            if ($bytesRead -eq 0) {
                break
            }
            if ($bytesRead -gt $remainingBytes) {
                Throw-ProfilePublish -Code 'store_response_too_large'
            }
            $memory.Write($buffer, 0, $bytesRead)
            $totalBytes += $bytesRead
        }
        try {
            $utf8 = [System.Text.UTF8Encoding]::new($false, $true)
            return $utf8.GetString($memory.ToArray())
        }
        catch [System.Text.DecoderFallbackException] {
            Throw-ProfilePublish -Code 'store_response_invalid'
        }
    }
    finally {
        $memory.Dispose()
    }
}

<#
    Default App Configuration data-plane boundary.

    This path is unexercised offline: every test injects -StoreInvoker instead.
    It exists so the tool is complete, and it keeps the discipline of
    tools/foundry-smoke/check-prerequisites.ps1 -- the bearer token lives in a
    local, is passed only to the request, and is cleared in a finally block.
#>
function Invoke-ProfileStoreRequest {
    param(
        [Parameter(Mandatory)][uri] $Endpoint,
        [Parameter(Mandatory)][string] $Method,
        [Parameter(Mandatory)][string] $PathAndQuery,
        [Parameter(Mandatory)][string] $AccessToken,
        [AllowEmptyString()][string] $Body = '',
        [AllowEmptyString()][string] $IfMatch = '',
        [AllowEmptyString()][string] $IfNoneMatch = ''
    )

    $handler = [System.Net.Http.HttpClientHandler]::new()
    $handler.AllowAutoRedirect = $false
    $client = [System.Net.Http.HttpClient]::new($handler)
    $client.Timeout = [TimeSpan]::FromSeconds(30)
    $request = [System.Net.Http.HttpRequestMessage]::new(
        [System.Net.Http.HttpMethod]::new($Method),
        [uri]::new($Endpoint, $PathAndQuery)
    )
    $request.Headers.Authorization =
        [System.Net.Http.Headers.AuthenticationHeaderValue]::new(
            'Bearer',
            $AccessToken
        )
    $request.Headers.Accept.ParseAdd('application/json')
    if (-not [string]::IsNullOrWhiteSpace($IfMatch)) {
        $request.Headers.TryAddWithoutValidation('If-Match', $IfMatch) | Out-Null
    }
    if (-not [string]::IsNullOrWhiteSpace($IfNoneMatch)) {
        $request.Headers.TryAddWithoutValidation('If-None-Match', $IfNoneMatch) |
            Out-Null
    }
    if (-not [string]::IsNullOrEmpty($Body)) {
        $request.Content = [System.Net.Http.StringContent]::new(
            $Body,
            [System.Text.UTF8Encoding]::new($false),
            $script:StoreContentType
        )
    }

    try {
        $response = $null
        try {
            $response = $client.Send(
                $request,
                [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead
            )
        }
        catch [System.Net.Http.HttpRequestException] {
            Throw-ProfilePublish -Code 'store_unavailable'
        }
        try {
            $statusCode = [int] $response.StatusCode
            if ($statusCode -eq 412) {
                Throw-ProfilePublish -Code 'concurrent_update'
            }
            if ($statusCode -eq 404) {
                return $null
            }
            if (-not $response.IsSuccessStatusCode) {
                switch ($statusCode) {
                    401 { Throw-ProfilePublish -Code 'store_access_unavailable' }
                    403 { Throw-ProfilePublish -Code 'store_access_unavailable' }
                    429 { Throw-ProfilePublish -Code 'store_rate_limited' }
                    default { Throw-ProfilePublish -Code "store_http_$statusCode" }
                }
            }
            $stream = $null
            try {
                $stream = $response.Content.ReadAsStream()
                return Read-ProfileBoundedStream -Stream $stream
            }
            finally {
                if ($null -ne $stream) {
                    $stream.Dispose()
                }
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

function ConvertTo-ProfileStoreItem {
    param([Parameter(Mandatory)][object] $Value)
    return [pscustomobject][ordered]@{
        key = [string] $Value.key
        label = [string] $Value.label
        value = [string] $Value.value
        etag = [string] $Value.etag
    }
}

function Invoke-ProfileStoreOperation {
    param(
        [Parameter(Mandatory)][hashtable] $Request,
        [Parameter(Mandatory)][uri] $Endpoint
    )

    Assert-ProfileStoreOperation -Request $Request | Out-Null
    $operation = [string] $Request['operation']
    $label = [uri]::EscapeDataString([string] $Request['label'])
    $accessToken = ''
    try {
        $accessToken = [string] (
            Invoke-ProfileAzCli -Arguments @(
                'account',
                'get-access-token',
                '--scope',
                'https://azconfig.io/.default',
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
            Throw-ProfilePublish -Code 'store_access_unavailable'
        }

        if ($operation -ceq 'list') {
            $filter = [uri]::EscapeDataString([string] $Request['keyFilter'])
            $query = "/kv?key=$filter&label=$label" +
                "&api-version=$($script:StoreApiVersion)"
            $body = Invoke-ProfileStoreRequest `
                -Endpoint $Endpoint `
                -Method 'GET' `
                -PathAndQuery $query `
                -AccessToken $accessToken
            if ([string]::IsNullOrWhiteSpace($body)) {
                return @()
            }
            $page = $body | ConvertFrom-Json -Depth 16 -NoEnumerate
            if ($null -eq $page.PSObject.Properties['items']) {
                Throw-ProfilePublish -Code 'store_response_invalid'
            }
            return @(
                $page.items |
                    ForEach-Object { ConvertTo-ProfileStoreItem -Value $_ }
            )
        }

        $key = [uri]::EscapeDataString([string] $Request['key'])
        $query = "/kv/$key`?label=$label&api-version=$($script:StoreApiVersion)"

        if ($operation -ceq 'get') {
            $body = Invoke-ProfileStoreRequest `
                -Endpoint $Endpoint `
                -Method 'GET' `
                -PathAndQuery $query `
                -AccessToken $accessToken
            if ([string]::IsNullOrWhiteSpace($body)) {
                return $null
            }
            return ConvertTo-ProfileStoreItem -Value (
                $body | ConvertFrom-Json -Depth 16 -NoEnumerate
            )
        }

        $ifMatch = ''
        if ($Request.ContainsKey('ifMatch')) {
            $ifMatch = [string] $Request['ifMatch']
        }
        $ifNoneMatch = ''
        if ($Request.ContainsKey('ifNoneMatch')) {
            $ifNoneMatch = [string] $Request['ifNoneMatch']
        }
        $payload = [pscustomobject]@{ value = [string] $Request['value'] } |
            ConvertTo-Json -Depth 4 -Compress
        $body = Invoke-ProfileStoreRequest `
            -Endpoint $Endpoint `
            -Method 'PUT' `
            -PathAndQuery $query `
            -AccessToken $accessToken `
            -Body $payload `
            -IfMatch $ifMatch `
            -IfNoneMatch $ifNoneMatch
        if ([string]::IsNullOrWhiteSpace($body)) {
            Throw-ProfilePublish -Code 'store_response_invalid'
        }
        return ConvertTo-ProfileStoreItem -Value (
            $body | ConvertFrom-Json -Depth 16 -NoEnumerate
        )
    }
    finally {
        $accessToken = ''
        Remove-Variable accessToken -ErrorAction SilentlyContinue
    }
}

function Get-ProfileStoreState {
    param(
        [Parameter(Mandatory)][scriptblock] $StoreInvoker,
        [Parameter(Mandatory)][string] $Label
    )

    $items = @(
        Invoke-ProfileStore -StoreInvoker $StoreInvoker -Request @{
            operation = 'list'
            keyFilter = "$($script:ProfileKeyPrefix)*"
            label = $Label
        }
    )
    $profiles = [System.Collections.Generic.List[object]]::new()
    $etags = @{}
    foreach ($item in $items) {
        $key = [string] $item.key
        if (
            -not $key.StartsWith(
                $script:ProfileKeyPrefix,
                [System.StringComparison]::Ordinal
            )
        ) {
            continue
        }
        $profileId = $key.Substring($script:ProfileKeyPrefix.Length)
        if (-not (Test-ProfileIdentifier -Value $profileId)) {
            Throw-ProfilePublish -Code 'store_key_forbidden'
        }
        $profiles.Add(
            [pscustomobject][ordered]@{
                profileId = $profileId
                valueHash = Get-ProfileValueHash `
                    -CanonicalJson ([string] $item.value) `
                    -Purpose $script:ProfileHashPurpose
            }
        )
        $etags[$key] = [string] $item.etag
    }

    $registryItem = Invoke-ProfileStore -StoreInvoker $StoreInvoker -Request @{
        operation = 'get'
        key = $script:RegistryVersionKey
        label = $Label
    }
    $registryVersion = $null
    if ($null -ne $registryItem) {
        $record = Read-ProfileRegistryRecord -Value ([string] $registryItem.value)
        $registryVersion = $record.registryVersion
        $etags[$script:RegistryVersionKey] = [string] $registryItem.etag
    }

    return [pscustomobject][ordered]@{
        schemaVersion = 'forge.model-profile-store-state.v1'
        label = $Label
        registryVersion = $registryVersion
        profiles = @(
            $profiles |
                Sort-Object -CaseSensitive -Property { [string] $_.profileId }
        )
        etags = $etags
    }
}

function New-ProfilePublishOperation {
    param(
        [Parameter(Mandatory)][ValidateRange(1, 1024)][int] $Order,
        [Parameter(Mandatory)][string] $Kind,
        [Parameter(Mandatory)][string] $Key,
        [Parameter(Mandatory)][string] $Label,
        [Parameter(Mandatory)][string] $Value,
        [Parameter(Mandatory)][string] $Purpose,
        [AllowEmptyString()][AllowNull()][string] $Etag
    )

    $conditional = -not [string]::IsNullOrWhiteSpace($Etag)
    $condition = 'if-none-match'
    $expectedEtagHash = $null
    if ($conditional) {
        $condition = 'if-match'
        $expectedEtagHash = Get-ProfileSha256Label -Value $Etag
    }

    return [pscustomobject][ordered]@{
        order = $Order
        kind = $Kind
        key = $Key
        label = $Label
        condition = $condition
        expectedEtagHash = $expectedEtagHash
        valueHash = Get-ProfileValueHash -CanonicalJson $Value -Purpose $Purpose
        value = $Value
        ifMatch = $Etag
    }
}

function New-ProfilePublishPlan {
    param(
        [Parameter(Mandatory)][object] $Plan,
        [Parameter(Mandatory)][object] $StoreState,
        [Parameter(Mandatory)][string] $Label,
        [Parameter(Mandatory)][string] $RegistryValue,
        [Parameter(Mandatory)][ValidateRange(1, 2147483647)][int] $RegistryVersion
    )

    $archiveLabel = Get-ProfileArchiveLabel `
        -Label $Label `
        -RegistryVersion $RegistryVersion
    $operations = [System.Collections.Generic.List[object]]::new()
    $etags = $StoreState.etags

    # The immutable archive under a version label is written first. It is what
    # a later rollback reads, and no reader watches that label.
    foreach ($profile in @($Plan.profiles)) {
        $operations.Add(
            (New-ProfilePublishOperation `
                -Order ($operations.Count + 1) `
                -Kind 'archive_profile' `
                -Key ([string] $profile.key) `
                -Label $archiveLabel `
                -Value ([string] $profile.value) `
                -Purpose $script:ProfileHashPurpose `
                -Etag $null)
        )
    }
    $operations.Add(
        (New-ProfilePublishOperation `
            -Order ($operations.Count + 1) `
            -Kind 'archive_registry' `
            -Key $script:RegistryVersionKey `
            -Label $archiveLabel `
            -Value $RegistryValue `
            -Purpose $script:RegistryRecordPurpose `
            -Etag $null)
    )

    foreach ($profile in @($Plan.profiles)) {
        $key = [string] $profile.key
        $etag = $null
        if ($etags.ContainsKey($key)) {
            $etag = [string] $etags[$key]
        }
        $operations.Add(
            (New-ProfilePublishOperation `
                -Order ($operations.Count + 1) `
                -Kind 'set_profile' `
                -Key $key `
                -Label $Label `
                -Value ([string] $profile.value) `
                -Purpose $script:ProfileHashPurpose `
                -Etag $etag)
        )
    }

    # Last, always: the registry-version key is what makes the set visible.
    $registryEtag = $null
    if ($etags.ContainsKey($script:RegistryVersionKey)) {
        $registryEtag = [string] $etags[$script:RegistryVersionKey]
    }
    $operations.Add(
        (New-ProfilePublishOperation `
            -Order ($operations.Count + 1) `
            -Kind 'set_registry' `
            -Key $script:RegistryVersionKey `
            -Label $Label `
            -Value $RegistryValue `
            -Purpose $script:RegistryRecordPurpose `
            -Etag $registryEtag)
    )

    return @($operations)
}

function Invoke-ProfilePublishOperations {
    param(
        [Parameter(Mandatory)][scriptblock] $StoreInvoker,
        [Parameter(Mandatory)][AllowEmptyCollection()][object[]] $Operations
    )

    $applied = [System.Collections.Generic.List[object]]::new()
    foreach ($operation in $Operations) {
        $request = @{
            operation = 'set'
            key = [string] $operation.key
            label = [string] $operation.label
            value = [string] $operation.value
        }
        if ([string]::IsNullOrWhiteSpace([string] $operation.ifMatch)) {
            $request['ifNoneMatch'] = '*'
        }
        else {
            $request['ifMatch'] = [string] $operation.ifMatch
        }
        Invoke-ProfileStore -StoreInvoker $StoreInvoker -Request $request |
            Out-Null
        $applied.Add(
            [pscustomobject][ordered]@{
                order = $operation.order
                kind = $operation.kind
                key = $operation.key
                label = $operation.label
                condition = $operation.condition
                valueHash = $operation.valueHash
            }
        )
    }
    return @($applied)
}

function ConvertTo-ProfileRedactedOperation {
    param([Parameter(Mandatory)][object] $Operation)
    return [pscustomobject][ordered]@{
        order = $Operation.order
        kind = $Operation.kind
        key = $Operation.key
        label = $Operation.label
        condition = $Operation.condition
        expectedEtagHash = $Operation.expectedEtagHash
        valueHash = $Operation.valueHash
    }
}

function Write-ProfilePublishReport {
    param(
        [Parameter(Mandatory)][object] $Report,
        [Parameter(Mandatory)][string] $Path
    )

    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    $directory = [System.IO.Path]::GetDirectoryName($resolvedPath)
    if ([string]::IsNullOrWhiteSpace($directory)) {
        Throw-ProfilePublish -Code 'report_path_invalid'
    }
    [System.IO.Directory]::CreateDirectory($directory) | Out-Null
    $json = $Report | ConvertTo-Json -Depth 32
    [System.IO.File]::WriteAllText(
        $resolvedPath,
        "$json`n",
        [System.Text.UTF8Encoding]::new($false)
    )
}

function Invoke-ProfileValidator {
    param(
        [Parameter(Mandatory)][string] $ProfilePath,
        [Parameter(Mandatory)][string] $DeploymentPath,
        [AllowEmptyString()][AllowNull()][string] $CurrentStatePath,
        [Parameter(Mandatory)][string] $PlanPath
    )

    $validator = Join-Path $PSScriptRoot 'validate-profiles.mjs'
    $arguments = [System.Collections.Generic.List[string]]::new()
    $arguments.Add($validator)
    $arguments.Add('--profiles')
    $arguments.Add($ProfilePath)
    $arguments.Add('--deployments')
    $arguments.Add($DeploymentPath)
    if (-not [string]::IsNullOrWhiteSpace($CurrentStatePath)) {
        $arguments.Add('--current')
        $arguments.Add($CurrentStatePath)
    }
    $arguments.Add('--emit-plan')
    $arguments.Add($PlanPath)
    $arguments.Add('--quiet')

    if ($null -eq (Get-Command node -ErrorAction SilentlyContinue)) {
        Throw-ProfilePublish -Code 'validator_unavailable'
    }
    $output = & node @arguments 2>$null
    $exitCode = $LASTEXITCODE
    $text = ($output -join [Environment]::NewLine).Trim()
    $report = $null
    if (-not [string]::IsNullOrWhiteSpace($text)) {
        try {
            $report = $text | ConvertFrom-Json -Depth 32 -NoEnumerate
        }
        catch {
            $report = $null
        }
    }
    return [pscustomobject][ordered]@{
        exitCode = $exitCode
        report = $report
    }
}

function Get-ProfilePublishApproval {
    param(
        [Parameter(Mandatory)][bool] $Approve,
        [AllowEmptyString()][AllowNull()][string] $EnvironmentApproval
    )

    if ($Approve) {
        return [pscustomobject]@{ approved = $true; source = 'operator_switch' }
    }
    if (
        -not [string]::IsNullOrEmpty($EnvironmentApproval) -and
        $EnvironmentApproval -ceq $script:ApprovalValue
    ) {
        return [pscustomobject]@{ approved = $true; source = 'ci_environment' }
    }
    return [pscustomobject]@{ approved = $false; source = 'none' }
}

function Invoke-ProfilePublish {
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter(Mandatory)][string] $ProfilePath,
        [Parameter(Mandatory)][string] $DeploymentPath,
        [Parameter(Mandatory)][string] $ReportPath,
        [Parameter(Mandatory)][string] $Label,
        [AllowEmptyString()][AllowNull()][string] $StoreEndpoint,
        [ValidateRange(0, 2147483647)][int] $RollbackToRegistryVersion = 0,
        [bool] $Approve = $false,
        [AllowEmptyString()][AllowNull()][string] $EnvironmentApproval = (
            $env:FORGE_MODEL_PROFILE_PUBLISH_APPROVAL
        ),
        [scriptblock] $StoreInvoker,
        [scriptblock] $ValidatorInvoker
    )

    $blockers = [System.Collections.Generic.List[string]]::new()
    $addBlocker = {
        param([string] $Code)
        if (-not $blockers.Contains($Code)) {
            $blockers.Add($Code)
        }
    }
    $readFailureCode = {
        param([System.Exception] $Exception, [string] $Fallback)
        $prefix = 'profile_publish:'
        if ($Exception.Message.StartsWith($prefix)) {
            return $Exception.Message.Substring($prefix.Length)
        }
        return $Fallback
    }

    $dryRun = -not $PSCmdlet.ShouldProcess(
        "Azure App Configuration label '$Label'",
        'publish the model profile registry'
    )
    $approval = Get-ProfilePublishApproval `
        -Approve $Approve `
        -EnvironmentApproval $EnvironmentApproval
    $rollback = $RollbackToRegistryVersion -gt 0
    $mode = 'publish'
    if ($dryRun) {
        $mode = 'dry_run'
    }
    elseif ($rollback) {
        $mode = 'rollback'
    }

    if (-not (Test-ProfileLabelName -Value $Label)) {
        & $addBlocker 'label_invalid'
    }

    $endpointHash = $null
    $resolvedEndpoint = $null
    if (-not [string]::IsNullOrWhiteSpace($StoreEndpoint)) {
        try {
            $resolvedEndpoint = Resolve-ProfileStoreEndpoint -Value $StoreEndpoint
            $endpointHash = Get-ProfileSha256Label `
                -Value $resolvedEndpoint.AbsoluteUri.ToLowerInvariant()
        }
        catch {
            & $addBlocker (
                & $readFailureCode $_.Exception 'store_endpoint_invalid'
            )
        }
    }

    if ($null -eq $StoreInvoker) {
        if ($null -eq $resolvedEndpoint) {
            & $addBlocker 'store_endpoint_missing'
        }
        else {
            $endpoint = $resolvedEndpoint
            $StoreInvoker = {
                param([hashtable] $Request)
                Invoke-ProfileStoreOperation -Request $Request -Endpoint $endpoint
            }.GetNewClosure()
        }
    }

    $storeState = $null
    $storeReadable = $false
    if ($blockers.Count -eq 0 -and $null -ne $StoreInvoker) {
        try {
            $storeState = Get-ProfileStoreState `
                -StoreInvoker $StoreInvoker `
                -Label $Label
            $storeReadable = $true
        }
        catch {
            & $addBlocker (& $readFailureCode $_.Exception 'store_unavailable')
        }
    }

    $validation = $null
    $plan = $null
    $registryVersion = $null
    $registryValue = $null
    $registryHash = $null
    $capabilityHash = $null
    $diff = $null
    $plannedOperations = @()
    $appliedOperations = @()
    $temporaryPaths = [System.Collections.Generic.List[string]]::new()

    try {
        if ($rollback -and $blockers.Count -eq 0 -and $storeReadable) {
            try {
                $archiveLabel = Get-ProfileArchiveLabel `
                    -Label $Label `
                    -RegistryVersion $RollbackToRegistryVersion
                $archiveRegistry = Invoke-ProfileStore `
                    -StoreInvoker $StoreInvoker `
                    -Request @{
                        operation = 'get'
                        key = $script:RegistryVersionKey
                        label = $archiveLabel
                    }
                if ($null -eq $archiveRegistry) {
                    Throw-ProfilePublish -Code 'rollback_target_missing'
                }
                $archiveRecord = Read-ProfileRegistryRecord `
                    -Value ([string] $archiveRegistry.value)
                if ($archiveRecord.registryVersion -ne $RollbackToRegistryVersion) {
                    Throw-ProfilePublish -Code 'rollback_target_mismatch'
                }

                $currentVersion = 0
                if ($null -ne $storeState.registryVersion) {
                    $currentVersion = [int] $storeState.registryVersion
                }
                if ($RollbackToRegistryVersion -ge $currentVersion) {
                    Throw-ProfilePublish -Code 'rollback_target_not_prior'
                }

                $restored = [System.Collections.Generic.List[object]]::new()
                $pairs = [System.Collections.Generic.List[object]]::new()
                foreach ($profileId in $archiveRecord.profileIds) {
                    $key = "$($script:ProfileKeyPrefix)$profileId"
                    $item = Invoke-ProfileStore `
                        -StoreInvoker $StoreInvoker `
                        -Request @{
                            operation = 'get'
                            key = $key
                            label = $archiveLabel
                        }
                    if ($null -eq $item) {
                        Throw-ProfilePublish -Code 'rollback_profile_missing'
                    }
                    $value = [string] $item.value
                    $valueHash = Get-ProfileValueHash `
                        -CanonicalJson $value `
                        -Purpose $script:ProfileHashPurpose
                    $restored.Add(
                        [pscustomobject][ordered]@{
                            profileId = $profileId
                            key = $key
                            value = $value
                            valueHash = $valueHash
                        }
                    )
                    $pairs.Add(
                        [pscustomobject]@{
                            profileId = $profileId
                            profileHash = $valueHash
                        }
                    )
                }

                $registryVersion = $currentVersion + 1
                $record = New-ProfileRegistryRecord `
                    -RegistryVersion $registryVersion `
                    -CapabilityHash $archiveRecord.capabilityHash `
                    -ProfileHashPairs @($pairs)
                $registryValue = $record.value
                $registryHash = $record.registryHash
                $capabilityHash = $record.capabilityHash
                $plan = [pscustomobject]@{ profiles = @($restored) }
            }
            catch {
                & $addBlocker (& $readFailureCode $_.Exception 'rollback_failed')
            }
        }
        elseif (-not $rollback) {
            $planPath = Join-Path `
                ([System.IO.Path]::GetTempPath()) `
                "forge-profile-plan-$([guid]::NewGuid().ToString('N')).json"
            $temporaryPaths.Add($planPath)
            $currentStatePath = ''
            if ($storeReadable) {
                $currentStatePath = Join-Path `
                    ([System.IO.Path]::GetTempPath()) `
                    "forge-profile-state-$([guid]::NewGuid().ToString('N')).json"
                $temporaryPaths.Add($currentStatePath)
                $redactedState = [pscustomobject][ordered]@{
                    schemaVersion = $storeState.schemaVersion
                    label = $storeState.label
                    registryVersion = $storeState.registryVersion
                    profiles = @($storeState.profiles)
                }
                [System.IO.File]::WriteAllText(
                    $currentStatePath,
                    ($redactedState | ConvertTo-Json -Depth 8),
                    [System.Text.UTF8Encoding]::new($false)
                )
            }

            if ($null -eq $ValidatorInvoker) {
                $ValidatorInvoker = {
                    param(
                        [string] $Profiles,
                        [string] $Deployments,
                        [string] $Current,
                        [string] $PlanOutput
                    )
                    Invoke-ProfileValidator `
                        -ProfilePath $Profiles `
                        -DeploymentPath $Deployments `
                        -CurrentStatePath $Current `
                        -PlanPath $PlanOutput
                }
            }
            $validationResult = & $ValidatorInvoker `
                -Profiles $ProfilePath `
                -Deployments $DeploymentPath `
                -Current $currentStatePath `
                -PlanOutput $planPath
            $validation = $validationResult.report

            if ($validationResult.exitCode -ne 0) {
                & $addBlocker 'document_refused'
            }
            elseif (-not (Test-Path -LiteralPath $planPath -PathType Leaf)) {
                & $addBlocker 'publish_plan_missing'
            }
            else {
                $plan = Get-Content -Raw -LiteralPath $planPath |
                    ConvertFrom-Json -Depth 32 -NoEnumerate
                $registryVersion = [int] $plan.registryVersion
                $registryValue = [string] $plan.registryValue
                $registryHash = [string] $plan.registryHash
                $capabilityHash = [string] $plan.capabilityHash
                if (
                    $null -ne $validation -and
                    $null -ne $validation.PSObject.Properties['diff']
                ) {
                    $diff = $validation.diff
                }
                if (
                    $storeReadable -and
                    $null -ne $storeState.registryVersion -and
                    $registryVersion -le [int] $storeState.registryVersion
                ) {
                    & $addBlocker 'registry_version_not_increasing'
                }
            }
        }

        if ($blockers.Count -eq 0 -and $null -ne $plan) {
            if ($storeReadable) {
                try {
                    $plannedOperations = New-ProfilePublishPlan `
                        -Plan $plan `
                        -StoreState $storeState `
                        -Label $Label `
                        -RegistryValue $registryValue `
                        -RegistryVersion $registryVersion
                }
                catch {
                    & $addBlocker (& $readFailureCode $_.Exception 'plan_failed')
                }
            }
            else {
                & $addBlocker 'store_unreadable'
            }
        }

        if (-not $dryRun -and -not $approval.approved) {
            & $addBlocker 'publish_approval_missing'
        }

        if (
            -not $dryRun -and
            $approval.approved -and
            $blockers.Count -eq 0 -and
            @($plannedOperations).Count -gt 0
        ) {
            try {
                $appliedOperations = Invoke-ProfilePublishOperations `
                    -StoreInvoker $StoreInvoker `
                    -Operations @($plannedOperations)
            }
            catch {
                & $addBlocker (& $readFailureCode $_.Exception 'publish_failed')
            }
        }
    }
    finally {
        foreach ($path in $temporaryPaths) {
            if (Test-Path -LiteralPath $path -PathType Leaf) {
                # -WhatIf:$false because a dry run must still clean up after
                # itself; the switch governs store writes, not scratch files.
                Remove-Item `
                    -LiteralPath $path `
                    -Force `
                    -WhatIf:$false `
                    -ErrorAction SilentlyContinue
            }
        }
    }

    $completed = $false
    if ($blockers.Count -eq 0) {
        $completed = $dryRun -or (@($appliedOperations).Count -gt 0)
    }
    $archiveLabelReported = $null
    if ($null -ne $registryVersion) {
        $archiveLabelReported = "$Label-v$registryVersion"
    }
    $currentRegistryVersion = $null
    if ($storeReadable) {
        $currentRegistryVersion = $storeState.registryVersion
    }
    $validationSummary = $null
    if ($null -ne $validation) {
        $validationSummary = [pscustomobject][ordered]@{
            valid = $validation.valid
            profileCount = @($validation.profiles).Count
            deploymentInventoryReady = $validation.deploymentInventory.ready
            violations = @(
                $validation.violations |
                    ForEach-Object {
                        [pscustomobject][ordered]@{
                            code = [string] $_.code
                            path = [string] $_.path
                        }
                    }
            )
        }
    }

    $report = [pscustomobject][ordered]@{
        schemaVersion = 'forge.model-profile-publish.v1'
        generatedAt = [DateTimeOffset]::UtcNow.ToString('O')
        mode = $mode
        completed = $completed
        approved = $approval.approved
        approvalSource = $approval.source
        label = $Label
        archiveLabel = $archiveLabelReported
        keyPrefix = $script:ProfileKeyPrefix
        registryVersionKey = $script:RegistryVersionKey
        storeEndpointHash = $endpointHash
        storeReadable = $storeReadable
        currentRegistryVersion = $currentRegistryVersion
        registryVersion = $registryVersion
        registryHash = $registryHash
        capabilityHash = $capabilityHash
        validation = $validationSummary
        diff = $diff
        plannedOperations = @(
            $plannedOperations |
                ForEach-Object { ConvertTo-ProfileRedactedOperation -Operation $_ }
        )
        appliedOperations = @($appliedOperations)
        blockers = @($blockers | Sort-Object -Unique)
    }
    Write-ProfilePublishReport -Report $report -Path $ReportPath

    if ($blockers.Count -gt 0) {
        return 2
    }
    return 0
}

if ($MyInvocation.InvocationName -ne '.') {
    $exitCode = Invoke-ProfilePublish `
        -ProfilePath $ProfilePath `
        -DeploymentPath $DeploymentPath `
        -ReportPath $OutputPath `
        -Label $Label `
        -StoreEndpoint $StoreEndpoint `
        -RollbackToRegistryVersion $RollbackToRegistryVersion `
        -Approve $Approve.IsPresent `
        -WhatIf:$WhatIfPreference
    exit $exitCode
}
