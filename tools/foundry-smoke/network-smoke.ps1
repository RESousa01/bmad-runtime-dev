[CmdletBinding()]
param(
    [string] $ProjectEndpoint = $env:FOUNDRY_PROJECT_ENDPOINT,
    [string] $ManagedIdentityClientId = $env:FOUNDRY_MODEL_IDENTITY_CLIENT_ID,
    [string] $OutputPath = (
        Join-Path `
            $PSScriptRoot `
            '..\..\artifacts\foundry\infra\network-smoke.json'
    )
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Network reachability smoke for the EXTERNAL Microsoft Foundry project.
# It proves the network path only: name resolution, TLS/SNI, a managed-identity
# token, and that the project's own deployment route answers. It never selects,
# names, or assumes a deployment; tools/foundry-smoke/check-prerequisites.ps1
# remains the authority on deployment selection and authorization.
#
# Redaction discipline matches check-prerequisites.ps1: the report and the
# printed summary carry counts, status classes, booleans, and sha256 labels
# only. No token, no response body, no deployment name, and no raw address ever
# reaches disk or the console.

$script:FoundryTokenScope = 'https://ai.azure.com/.default'

function Throw-FoundryNetworkFailure {
    param([Parameter(Mandatory)][string] $Code)
    throw "foundry_network:$Code"
}

function Get-FoundryNetworkSha256Label {
    param([Parameter(Mandatory)][string] $Value)
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($Value)
    $hash = [System.Security.Cryptography.SHA256]::HashData($bytes)
    return "sha256:$([Convert]::ToHexString($hash).ToLowerInvariant())"
}

function Resolve-FoundryNetworkEndpoint {
    param([Parameter(Mandatory)][string] $Value)

    # Deliberately a faithful mirror of Resolve-FoundryProjectEndpoint in
    # check-prerequisites.ps1: the same accept/reject decision, so an endpoint
    # that one tool refuses is never smoke-tested by the other. The tests
    # cross-check both implementations against the same corpus.
    $parsed = $null
    if (-not [uri]::TryCreate($Value, [System.UriKind]::Absolute, [ref] $parsed)) {
        Throw-FoundryNetworkFailure -Code 'project_endpoint_invalid'
    }
    if ($parsed.Scheme -cne 'https') {
        Throw-FoundryNetworkFailure -Code 'project_endpoint_scheme_invalid'
    }
    if (-not $parsed.IsDefaultPort -or $parsed.Port -ne 443) {
        Throw-FoundryNetworkFailure -Code 'project_endpoint_port_invalid'
    }
    if (-not [string]::IsNullOrEmpty($parsed.UserInfo)) {
        Throw-FoundryNetworkFailure -Code 'project_endpoint_userinfo_invalid'
    }
    if (-not [string]::IsNullOrEmpty($parsed.Query)) {
        Throw-FoundryNetworkFailure -Code 'project_endpoint_query_invalid'
    }
    if (-not [string]::IsNullOrEmpty($parsed.Fragment)) {
        Throw-FoundryNetworkFailure -Code 'project_endpoint_fragment_invalid'
    }

    $hostSuffix = '.services.ai.azure.com'
    if (
        -not $parsed.DnsSafeHost.EndsWith(
            $hostSuffix,
            [System.StringComparison]::OrdinalIgnoreCase
        ) -or
        $parsed.DnsSafeHost.Length -le $hostSuffix.Length
    ) {
        Throw-FoundryNetworkFailure -Code 'project_endpoint_host_invalid'
    }

    $projectPrefix = '/api/projects/'
    if (
        -not $parsed.AbsolutePath.StartsWith(
            $projectPrefix,
            [System.StringComparison]::Ordinal
        )
    ) {
        Throw-FoundryNetworkFailure -Code 'project_endpoint_path_invalid'
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
        Throw-FoundryNetworkFailure -Code 'project_endpoint_path_invalid'
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

function Test-FoundryPrivateAddress {
    param([Parameter(Mandatory)][System.Net.IPAddress] $Address)

    if (
        $Address.AddressFamily -ne
            [System.Net.Sockets.AddressFamily]::InterNetwork
    ) {
        # Unique local addresses fc00::/7 are the IPv6 equivalent.
        $bytes = $Address.GetAddressBytes()
        return ($bytes[0] -band 0xfe) -eq 0xfc
    }
    $octets = $Address.GetAddressBytes()
    if ($octets[0] -eq 10) {
        return $true
    }
    if ($octets[0] -eq 172 -and $octets[1] -ge 16 -and $octets[1] -le 31) {
        return $true
    }
    if ($octets[0] -eq 192 -and $octets[1] -eq 168) {
        return $true
    }
    # Carrier-grade NAT 100.64.0.0/10 is what Azure hands some private
    # endpoints, so it counts as private egress rather than public egress.
    if ($octets[0] -eq 100 -and $octets[1] -ge 64 -and $octets[1] -le 127) {
        return $true
    }
    return $false
}

function ConvertTo-RedactedFoundryDnsResult {
    param(
        [Parameter(Mandatory)]
        [AllowEmptyCollection()]
        [object[]] $Addresses
    )

    $parsedAddresses = [System.Collections.Generic.List[System.Net.IPAddress]]::new()
    foreach ($address in $Addresses) {
        if ($address -is [System.Net.IPAddress]) {
            $parsedAddresses.Add($address)
            continue
        }
        $candidate = [System.Net.IPAddress]::None
        if (
            $address -isnot [string] -or
            -not [System.Net.IPAddress]::TryParse(
                [string] $address,
                [ref] $candidate
            )
        ) {
            Throw-FoundryNetworkFailure -Code 'dns_result_invalid'
        }
        $parsedAddresses.Add($candidate)
    }
    if ($parsedAddresses.Count -eq 0) {
        Throw-FoundryNetworkFailure -Code 'dns_unresolved'
    }
    if ($parsedAddresses.Count -gt 64) {
        Throw-FoundryNetworkFailure -Code 'dns_result_invalid'
    }

    $families = @(
        $parsedAddresses |
            ForEach-Object { [string] $_.AddressFamily } |
            Sort-Object -Unique
    )
    $privateCount = @(
        $parsedAddresses |
            Where-Object { Test-FoundryPrivateAddress -Address $_ }
    ).Count
    # Addresses are network topology. Only the shape of the answer and a stable
    # correlation label are recorded; never the addresses themselves.
    $canonical = (
        @($parsedAddresses | ForEach-Object { $_.ToString() } | Sort-Object) -join ','
    )
    return [pscustomobject][ordered]@{
        resolved = $true
        addressCount = $parsedAddresses.Count
        addressFamilies = $families
        privateAddressCount = $privateCount
        allAddressesPrivate = $privateCount -eq $parsedAddresses.Count
        addressSetHash = Get-FoundryNetworkSha256Label -Value $canonical
    }
}

function ConvertTo-RedactedFoundryTlsResult {
    param([Parameter(Mandatory)][object] $Value)

    $protocolProperty = $Value.PSObject.Properties['negotiatedProtocol']
    $subjectProperty = $Value.PSObject.Properties['certificateSubject']
    $issuerProperty = $Value.PSObject.Properties['certificateIssuer']
    $expiryProperty = $Value.PSObject.Properties['certificateNotAfter']
    if (
        $null -eq $protocolProperty -or
        $null -eq $subjectProperty -or
        $null -eq $issuerProperty -or
        $null -eq $expiryProperty
    ) {
        Throw-FoundryNetworkFailure -Code 'tls_result_invalid'
    }
    $protocol = [string] $protocolProperty.Value
    if (
        [string]::IsNullOrWhiteSpace($protocol) -or
        $protocol.Length -gt 32 -or
        $protocol -match '[\x00-\x1f\x7f]'
    ) {
        Throw-FoundryNetworkFailure -Code 'tls_result_invalid'
    }
    # TLS 1.0/1.1 are not an acceptable path to a data plane holding support
    # metadata, so an obsolete negotiation is a blocker, not a warning.
    $modernProtocols = @('Tls12', 'Tls13')
    $expiry = [DateTimeOffset]::MinValue
    if (
        -not [DateTimeOffset]::TryParse(
            [string] $expiryProperty.Value,
            [ref] $expiry
        )
    ) {
        Throw-FoundryNetworkFailure -Code 'tls_result_invalid'
    }

    return [pscustomobject][ordered]@{
        reachable = $true
        negotiatedProtocol = $protocol
        protocolIsModern = $protocol -in $modernProtocols
        certificateValidated = $true
        certificateSubjectHash = Get-FoundryNetworkSha256Label `
            -Value ([string] $subjectProperty.Value)
        certificateIssuerHash = Get-FoundryNetworkSha256Label `
            -Value ([string] $issuerProperty.Value)
        certificateNotAfter = $expiry.ToUniversalTime().ToString('O')
        certificateExpired = $expiry -le [DateTimeOffset]::UtcNow
    }
}

function ConvertTo-RedactedFoundryDeploymentRoute {
    param(
        [Parameter(Mandatory)][int] $StatusCode,
        [AllowEmptyString()][string] $Body,
        [ValidateRange(1024, 16777216)][int] $MaximumBytes = 1048576
    )

    $statusClass = "$([Math]::Floor($StatusCode / 100))xx"
    if ($StatusCode -lt 200 -or $StatusCode -ge 300) {
        switch ($StatusCode) {
            401 { Throw-FoundryNetworkFailure -Code 'deployment_route_unauthorized' }
            403 { Throw-FoundryNetworkFailure -Code 'deployment_route_forbidden' }
            404 { Throw-FoundryNetworkFailure -Code 'deployment_route_not_found' }
            429 { Throw-FoundryNetworkFailure -Code 'deployment_route_rate_limited' }
            default {
                Throw-FoundryNetworkFailure `
                    -Code "deployment_route_http_$statusClass"
            }
        }
    }
    if ([string]::IsNullOrWhiteSpace($Body)) {
        Throw-FoundryNetworkFailure -Code 'deployment_route_body_invalid'
    }
    if ([System.Text.Encoding]::UTF8.GetByteCount($Body) -gt $MaximumBytes) {
        Throw-FoundryNetworkFailure -Code 'deployment_route_body_too_large'
    }
    try {
        $page = $Body | ConvertFrom-Json -Depth 64 -NoEnumerate
    }
    catch {
        Throw-FoundryNetworkFailure -Code 'deployment_route_json_invalid'
    }
    if (
        $null -eq $page -or
        $null -eq $page.PSObject.Properties['value']
    ) {
        Throw-FoundryNetworkFailure -Code 'deployment_route_shape_invalid'
    }

    $names = [System.Collections.Generic.List[string]]::new()
    $modelDeploymentCount = 0
    foreach ($deployment in @($page.value)) {
        if (
            $deployment -isnot
                [System.Management.Automation.PSCustomObject]
        ) {
            Throw-FoundryNetworkFailure -Code 'deployment_route_shape_invalid'
        }
        $nameProperty = $deployment.PSObject.Properties['name']
        if (
            $null -eq $nameProperty -or
            $nameProperty.Value -isnot [string] -or
            [string]::IsNullOrWhiteSpace([string] $nameProperty.Value)
        ) {
            Throw-FoundryNetworkFailure -Code 'deployment_route_shape_invalid'
        }
        $names.Add([string] $nameProperty.Value)
        $typeProperty = $deployment.PSObject.Properties['type']
        if (
            $null -ne $typeProperty -and
            ([string] $typeProperty.Value) -ceq 'ModelDeployment'
        ) {
            $modelDeploymentCount += 1
        }
    }

    # A count and a correlation hash, never a name: this smoke must not become
    # a second, weaker source of deployment identity.
    $canonical = if ($names.Count -eq 0) {
        '[]'
    }
    else {
        (@($names | Sort-Object) -join "`n")
    }
    $nextLinkProperty = $page.PSObject.Properties['nextLink']
    return [pscustomobject][ordered]@{
        requested = $true
        statusClass = $statusClass
        deploymentCount = $names.Count
        modelDeploymentCount = $modelDeploymentCount
        inventoryHash = Get-FoundryNetworkSha256Label -Value $canonical
        additionalPagesAvailable = (
            $null -ne $nextLinkProperty -and
            -not [string]::IsNullOrWhiteSpace(
                [string] $nextLinkProperty.Value
            )
        )
    }
}

function Read-FoundryNetworkBoundedStream {
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
                Throw-FoundryNetworkFailure -Code 'http_response_timeout'
            }
            if ($bytesRead -eq 0) {
                break
            }
            if ($bytesRead -gt $remainingBytes) {
                Throw-FoundryNetworkFailure -Code 'http_response_too_large'
            }
            $memory.Write($buffer, 0, $bytesRead)
            $totalBytes += $bytesRead
        }
        try {
            $utf8 = [System.Text.UTF8Encoding]::new($false, $true)
            return $utf8.GetString($memory.ToArray())
        }
        catch [System.Text.DecoderFallbackException] {
            Throw-FoundryNetworkFailure -Code 'http_response_invalid'
        }
    }
    finally {
        $memory.Dispose()
    }
}

function Invoke-FoundryNetworkHttpGet {
    param(
        [Parameter(Mandatory)][uri] $Uri,
        [hashtable] $Headers = @{},
        [ValidateRange(1024, 16777216)][int] $MaximumBytes = 1048576,
        [ValidateRange(1, 120)][int] $TimeoutSeconds = 30
    )

    $handler = [System.Net.Http.HttpClientHandler]::new()
    $handler.AllowAutoRedirect = $false
    $client = [System.Net.Http.HttpClient]::new($handler)
    $client.Timeout = [TimeSpan]::FromSeconds($TimeoutSeconds)
    $request = [System.Net.Http.HttpRequestMessage]::new(
        [System.Net.Http.HttpMethod]::Get,
        $Uri
    )
    foreach ($header in $Headers.GetEnumerator()) {
        $request.Headers.TryAddWithoutValidation(
            [string] $header.Key,
            [string] $header.Value
        ) | Out-Null
    }
    $request.Headers.Accept.ParseAdd('application/json')

    $response = $null
    try {
        try {
            $response = $client.Send(
                $request,
                [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead
            )
        }
        catch [System.Net.Http.HttpRequestException] {
            Throw-FoundryNetworkFailure -Code 'http_request_failed'
        }
        catch [System.Threading.Tasks.TaskCanceledException] {
            Throw-FoundryNetworkFailure -Code 'http_response_timeout'
        }
        if (
            $response.Content.Headers.ContentLength.HasValue -and
            $response.Content.Headers.ContentLength.Value -gt $MaximumBytes
        ) {
            Throw-FoundryNetworkFailure -Code 'http_response_too_large'
        }
        $stream = $null
        $deadline = [System.Threading.CancellationTokenSource]::new()
        try {
            $deadline.CancelAfter([TimeSpan]::FromSeconds($TimeoutSeconds))
            $stream = $response.Content.ReadAsStream()
            $body = Read-FoundryNetworkBoundedStream `
                -Stream $stream `
                -MaximumBytes $MaximumBytes `
                -CancellationToken $deadline.Token
            return [pscustomobject][ordered]@{
                statusCode = [int] $response.StatusCode
                body = $body
            }
        }
        finally {
            if ($null -ne $stream) {
                $stream.Dispose()
            }
            $deadline.Dispose()
        }
    }
    finally {
        if ($null -ne $response) {
            $response.Dispose()
        }
        $request.Dispose()
        $client.Dispose()
        $handler.Dispose()
    }
}

function Resolve-FoundryNetworkDns {
    param([Parameter(Mandatory)][string] $HostName)

    try {
        return @([System.Net.Dns]::GetHostAddresses($HostName))
    }
    catch [System.Net.Sockets.SocketException] {
        Throw-FoundryNetworkFailure -Code 'dns_unresolved'
    }
    catch {
        Throw-FoundryNetworkFailure -Code 'dns_unresolved'
    }
}

function Test-FoundryNetworkTls {
    param(
        [Parameter(Mandatory)][string] $HostName,
        [ValidateRange(1, 65535)][int] $Port = 443,
        [ValidateRange(1, 120)][int] $TimeoutSeconds = 15
    )

    # Default certificate validation is deliberately left in place: the trust
    # decision the runtime will make is the trust decision this smoke reports.
    # The host name is passed to AuthenticateAsClient so the handshake carries
    # the same SNI the broker will send.
    $client = [System.Net.Sockets.TcpClient]::new()
    $stream = $null
    $ssl = $null
    try {
        $connect = $client.ConnectAsync($HostName, $Port)
        # Task.Wait surfaces a faulted connect as an AggregateException rather
        # than returning false, and that exception carries the host in its
        # message. Every connect failure is normalized to one blocker code.
        $connected = $false
        try {
            $connected = $connect.Wait([TimeSpan]::FromSeconds($TimeoutSeconds))
        }
        catch {
            Throw-FoundryNetworkFailure -Code 'tls_unreachable'
        }
        if (-not $connected -or $connect.IsFaulted) {
            Throw-FoundryNetworkFailure -Code 'tls_unreachable'
        }
        $stream = $client.GetStream()
        $ssl = [System.Net.Security.SslStream]::new($stream, $false)
        try {
            $ssl.AuthenticateAsClient($HostName)
        }
        catch [System.Security.Authentication.AuthenticationException] {
            Throw-FoundryNetworkFailure -Code 'tls_certificate_untrusted'
        }
        catch {
            Throw-FoundryNetworkFailure -Code 'tls_unreachable'
        }
        if ($null -eq $ssl.RemoteCertificate) {
            Throw-FoundryNetworkFailure -Code 'tls_certificate_untrusted'
        }
        $certificate =
            [System.Security.Cryptography.X509Certificates.X509Certificate2]::new(
                $ssl.RemoteCertificate
            )
        try {
            return [pscustomobject][ordered]@{
                negotiatedProtocol = [string] $ssl.SslProtocol
                certificateSubject = $certificate.Subject
                certificateIssuer = $certificate.Issuer
                certificateNotAfter = ([DateTimeOffset] $certificate.NotAfter).
                    ToUniversalTime().
                    ToString('O')
            }
        }
        finally {
            $certificate.Dispose()
        }
    }
    catch [System.Net.Sockets.SocketException] {
        Throw-FoundryNetworkFailure -Code 'tls_unreachable'
    }
    finally {
        if ($null -ne $ssl) {
            $ssl.Dispose()
        }
        if ($null -ne $stream) {
            $stream.Dispose()
        }
        $client.Dispose()
    }
}

function Get-FoundryNetworkManagedIdentityToken {
    param(
        [Parameter(Mandatory)][string] $Scope,
        [AllowEmptyString()][string] $ClientId
    )

    # The IMDS and Container Apps identity protocols both take a resource URI
    # rather than an OAuth scope, so the documented `/.default` scope is
    # reduced to its resource here and reported unchanged.
    $suffix = '/.default'
    $resource = if (
        $Scope.EndsWith($suffix, [System.StringComparison]::Ordinal)
    ) {
        $Scope.Substring(0, $Scope.Length - $suffix.Length)
    }
    else {
        $Scope
    }

    $identityEndpoint = $env:IDENTITY_ENDPOINT
    $identityHeader = $env:IDENTITY_HEADER
    $useContainerAppsIdentity = (
        -not [string]::IsNullOrWhiteSpace($identityEndpoint) -and
        -not [string]::IsNullOrWhiteSpace($identityHeader)
    )
    if ($useContainerAppsIdentity) {
        $source = 'container-apps-identity-endpoint'
        $builder = [System.UriBuilder]::new($identityEndpoint)
        $query = "api-version=2019-08-01&resource=$([uri]::EscapeDataString($resource))"
        if (-not [string]::IsNullOrWhiteSpace($ClientId)) {
            $query += "&client_id=$([uri]::EscapeDataString($ClientId))"
        }
        $builder.Query = $query
        $uri = $builder.Uri
        $headers = @{ 'X-IDENTITY-HEADER' = $identityHeader }
    }
    else {
        $source = 'instance-metadata-service'
        $query = 'api-version=2018-02-01&resource=' +
            [uri]::EscapeDataString($resource)
        if (-not [string]::IsNullOrWhiteSpace($ClientId)) {
            $query += "&client_id=$([uri]::EscapeDataString($ClientId))"
        }
        $uri = [uri] (
            'http://169.254.169.254/metadata/identity/oauth2/token?' + $query
        )
        $headers = @{ 'Metadata' = 'true' }
    }

    # Any transport failure here means the same thing to the operator: no
    # managed-identity token is available on this host. The underlying code is
    # not surfaced, so an identity-endpoint URL never reaches the report.
    try {
        $response = Invoke-FoundryNetworkHttpGet `
            -Uri $uri `
            -Headers $headers `
            -MaximumBytes 65536 `
            -TimeoutSeconds 10
    }
    catch {
        Throw-FoundryNetworkFailure -Code 'managed_identity_token_unavailable'
    }
    if ($response.statusCode -ne 200) {
        Throw-FoundryNetworkFailure -Code 'managed_identity_token_unavailable'
    }
    try {
        $payload = $response.body | ConvertFrom-Json -Depth 8 -NoEnumerate
    }
    catch {
        Throw-FoundryNetworkFailure -Code 'managed_identity_token_unavailable'
    }
    $tokenProperty = if ($null -ne $payload) {
        $payload.PSObject.Properties['access_token']
    }
    else {
        $null
    }
    $token = if ($null -ne $tokenProperty) {
        [string] $tokenProperty.Value
    }
    else {
        ''
    }
    if ([string]::IsNullOrWhiteSpace($token) -or $token.Length -lt 20) {
        Throw-FoundryNetworkFailure -Code 'managed_identity_token_unavailable'
    }
    return [pscustomobject][ordered]@{
        source = $source
        accessToken = $token
    }
}

function Invoke-FoundryDeploymentRouteProbe {
    param(
        [Parameter(Mandatory)][uri] $Uri,
        [Parameter(Mandatory)][string] $AccessToken
    )

    return Invoke-FoundryNetworkHttpGet `
        -Uri $Uri `
        -Headers @{ 'Authorization' = "Bearer $AccessToken" }
}

function Write-FoundryNetworkReport {
    param(
        [Parameter(Mandatory)]
        [object] $Report,

        [Parameter(Mandatory)]
        [string] $Path
    )

    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    $directory = [System.IO.Path]::GetDirectoryName($resolvedPath)
    if ([string]::IsNullOrWhiteSpace($directory)) {
        Throw-FoundryNetworkFailure -Code 'report_path_invalid'
    }
    [System.IO.Directory]::CreateDirectory($directory) | Out-Null
    $json = $Report | ConvertTo-Json -Depth 32
    [System.IO.File]::WriteAllText(
        $resolvedPath,
        "$json`n",
        [System.Text.UTF8Encoding]::new($false)
    )
}

function Invoke-FoundryNetworkSmoke {
    param(
        [AllowEmptyString()]
        [string] $ProjectEndpointValue,

        [AllowEmptyString()]
        [string] $ManagedIdentityClientIdValue,

        [Parameter(Mandatory)]
        [string] $ReportPath,

        [scriptblock] $DnsResolver,

        [scriptblock] $TlsProbe,

        [scriptblock] $TokenProvider,

        [scriptblock] $DeploymentProbe
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
        $prefix = 'foundry_network:'
        if ($Exception.Message.StartsWith($prefix)) {
            return $Exception.Message.Substring($prefix.Length)
        }
        return $Fallback
    }

    # Stage 1: shape. Nothing below this point may touch the network until the
    # endpoint has been proved to be the exact documented project endpoint,
    # because an unvalidated endpoint is an attacker-chosen destination.
    $validatedEndpoint = $null
    if ([string]::IsNullOrWhiteSpace($ProjectEndpointValue)) {
        & $addBlocker 'project_endpoint_missing'
    }
    else {
        try {
            $validatedEndpoint = Resolve-FoundryNetworkEndpoint `
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
    $hasManagedIdentityClientId = -not [string]::IsNullOrWhiteSpace(
        $ManagedIdentityClientIdValue
    )
    if (
        $hasManagedIdentityClientId -and
        -not [guid]::TryParse(
            $ManagedIdentityClientIdValue,
            [ref] $managedIdentityGuid
        )
    ) {
        & $addBlocker 'managed_identity_client_id_invalid'
        $hasManagedIdentityClientId = $false
    }

    $dnsResult = [pscustomobject][ordered]@{
        resolved = $false
        addressCount = 0
        addressFamilies = @()
        privateAddressCount = 0
        allAddressesPrivate = $false
        addressSetHash = $null
    }
    $tlsResult = [pscustomobject][ordered]@{
        reachable = $false
        negotiatedProtocol = $null
        protocolIsModern = $false
        certificateValidated = $false
        certificateSubjectHash = $null
        certificateIssuerHash = $null
        certificateNotAfter = $null
        certificateExpired = $false
    }
    $routeResult = [pscustomobject][ordered]@{
        requested = $false
        statusClass = $null
        deploymentCount = 0
        modelDeploymentCount = 0
        inventoryHash = $null
        additionalPagesAvailable = $false
    }
    $tokenAcquired = $false
    $tokenSource = $null

    if ($null -eq $DnsResolver) {
        $DnsResolver = {
            param([string] $HostName)
            Resolve-FoundryNetworkDns -HostName $HostName
        }
    }
    if ($null -eq $TlsProbe) {
        $TlsProbe = {
            param([string] $HostName, [int] $Port)
            Test-FoundryNetworkTls -HostName $HostName -Port $Port
        }
    }
    if ($null -eq $TokenProvider) {
        $TokenProvider = {
            param([string] $Scope, [string] $ClientId)
            Get-FoundryNetworkManagedIdentityToken `
                -Scope $Scope `
                -ClientId $ClientId
        }
    }
    if ($null -eq $DeploymentProbe) {
        $DeploymentProbe = {
            param([uri] $Uri, [string] $AccessToken)
            Invoke-FoundryDeploymentRouteProbe `
                -Uri $Uri `
                -AccessToken $AccessToken
        }
    }

    if ($null -ne $validatedEndpoint) {
        # Stage 2: name resolution.
        try {
            $addresses = @(
                & $DnsResolver -HostName $validatedEndpoint.DnsSafeHost
            )
            $dnsResult = ConvertTo-RedactedFoundryDnsResult `
                -Addresses $addresses
        }
        catch {
            & $addBlocker (
                & $readFailureCode $_.Exception 'dns_unresolved'
            )
        }

        # Stage 3: TLS and SNI to the same host.
        if ($dnsResult.resolved) {
            try {
                $tlsProbeResult = & $TlsProbe `
                    -HostName $validatedEndpoint.DnsSafeHost `
                    -Port 443
                $tlsResult = ConvertTo-RedactedFoundryTlsResult `
                    -Value $tlsProbeResult
                if (-not $tlsResult.protocolIsModern) {
                    & $addBlocker 'tls_protocol_obsolete'
                }
                if ($tlsResult.certificateExpired) {
                    & $addBlocker 'tls_certificate_expired'
                }
            }
            catch {
                & $addBlocker (
                    & $readFailureCode $_.Exception 'tls_unreachable'
                )
            }
        }

        # Stage 4: managed-identity token, then the project's own deployment
        # route. The token is held in one local and dropped immediately.
        if ($tlsResult.reachable) {
            $accessToken = $null
            try {
                $tokenResult = & $TokenProvider `
                    -Scope $script:FoundryTokenScope `
                    -ClientId (
                        $hasManagedIdentityClientId ?
                            $managedIdentityGuid.ToString() :
                            ''
                    )
                $sourceProperty = $tokenResult.PSObject.Properties['source']
                $tokenProperty =
                    $tokenResult.PSObject.Properties['accessToken']
                if (
                    $null -eq $tokenProperty -or
                    [string]::IsNullOrWhiteSpace(
                        [string] $tokenProperty.Value
                    )
                ) {
                    Throw-FoundryNetworkFailure `
                        -Code 'managed_identity_token_unavailable'
                }
                $accessToken = [string] $tokenProperty.Value
                $tokenSource = if ($null -ne $sourceProperty) {
                    [string] $sourceProperty.Value
                }
                else {
                    'injected'
                }
                $tokenAcquired = $true
            }
            catch {
                & $addBlocker (
                    & $readFailureCode `
                        $_.Exception `
                        'managed_identity_token_unavailable'
                )
            }

            if ($tokenAcquired) {
                try {
                    $routeUri = [uri] (
                        "$($validatedEndpoint.AbsoluteUri)/deployments" +
                        '?api-version=v1&deploymentType=ModelDeployment'
                    )
                    $probe = & $DeploymentProbe `
                        -Uri $routeUri `
                        -AccessToken $accessToken
                    $statusProperty = $probe.PSObject.Properties['statusCode']
                    $bodyProperty = $probe.PSObject.Properties['body']
                    if ($null -eq $statusProperty) {
                        Throw-FoundryNetworkFailure `
                            -Code 'deployment_route_shape_invalid'
                    }
                    $routeResult = ConvertTo-RedactedFoundryDeploymentRoute `
                        -StatusCode ([int] $statusProperty.Value) `
                        -Body (
                            $null -ne $bodyProperty ?
                                [string] $bodyProperty.Value :
                                ''
                        )
                }
                catch {
                    & $addBlocker (
                        & $readFailureCode `
                            $_.Exception `
                            'deployment_route_unavailable'
                    )
                }
                finally {
                    $accessToken = $null
                    Remove-Variable accessToken -ErrorAction SilentlyContinue
                }
            }
        }
    }

    if (-not $dnsResult.resolved) {
        & $addBlocker 'dns_unresolved'
    }
    if (-not $tlsResult.reachable) {
        & $addBlocker 'tls_unreachable'
    }
    if (-not $tokenAcquired) {
        & $addBlocker 'managed_identity_token_unavailable'
    }
    if (-not $routeResult.requested) {
        & $addBlocker 'deployment_route_unavailable'
    }

    $egressPosture = if (-not $dnsResult.resolved) {
        'unknown'
    }
    elseif ($dnsResult.allAddressesPrivate) {
        'private-address-egress'
    }
    else {
        'public-address-egress'
    }

    $ready = $blockers.Count -eq 0
    $report = [pscustomobject][ordered]@{
        schemaVersion = 'forge.foundry-network-smoke.v1'
        generatedAt = [DateTimeOffset]::UtcNow.ToString('O')
        ready = $ready
        projectEndpoint = if ($null -ne $validatedEndpoint) {
            $validatedEndpoint.AbsoluteUri
        }
        else {
            $null
        }
        checks = [pscustomobject][ordered]@{
            endpointValidated = $null -ne $validatedEndpoint
            dnsResolved = $dnsResult.resolved
            tlsReachable = $tlsResult.reachable
            managedIdentityTokenAcquired = $tokenAcquired
            deploymentRouteReachable = $routeResult.requested
        }
        dns = $dnsResult
        tls = $tlsResult
        managedIdentity = [pscustomobject][ordered]@{
            tokenAcquired = $tokenAcquired
            tokenScope = $script:FoundryTokenScope
            tokenSource = $tokenSource
            clientIdSupplied = $hasManagedIdentityClientId
            clientIdHash = if ($hasManagedIdentityClientId) {
                Get-FoundryNetworkSha256Label `
                    -Value $managedIdentityGuid.ToString()
            }
            else {
                $null
            }
        }
        deploymentRoute = $routeResult
        egress = [pscustomobject][ordered]@{
            posture = $egressPosture
        }
        blockers = @($blockers | Sort-Object -Unique)
    }
    Write-FoundryNetworkReport -Report $report -Path $ReportPath

    # The printed summary is counts and classes only, by the same rule as the
    # report: no endpoint host, no token, no deployment name. Write-Host keeps
    # it off the pipeline so the caller receives only the exit code.
    Write-Host (
        "foundry network smoke: ready=$($report.ready) " +
        "dns=$($dnsResult.resolved) tls=$($tlsResult.reachable) " +
        "token=$tokenAcquired route=$($routeResult.statusClass ?? 'none') " +
        "deployments=$($routeResult.deploymentCount) " +
        "egress=$egressPosture " +
        "blockers=$(@($report.blockers).Count)"
    )
    if ($ready) {
        return 0
    }
    return 2
}

if ($MyInvocation.InvocationName -ne '.') {
    $exitCode = Invoke-FoundryNetworkSmoke `
        -ProjectEndpointValue $ProjectEndpoint `
        -ManagedIdentityClientIdValue $ManagedIdentityClientId `
        -ReportPath $OutputPath
    exit $exitCode
}
