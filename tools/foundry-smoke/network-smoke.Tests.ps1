$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

. "$PSScriptRoot/network-smoke.ps1"
# The prerequisite checker is dot-sourced only so that the two endpoint
# validators can be compared against one corpus. Its dot-source guard keeps it
# from running any check here.
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
        $expectedMessage = "foundry_network:$Code"
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

$namedEndpointValue =
    'https://forge-east.services.ai.azure.com/api/projects/forge-prod'
$defaultEndpointValue =
    'https://forge-east.services.ai.azure.com/api/projects/_project'

$namedEndpoint = Resolve-FoundryNetworkEndpoint -Value $namedEndpointValue
Assert-Equal -Name 'named endpoint canonical form' `
    -Actual $namedEndpoint.AbsoluteUri `
    -Expected $namedEndpointValue

$defaultEndpoint = Resolve-FoundryNetworkEndpoint -Value $defaultEndpointValue
Assert-Equal -Name 'default project endpoint canonical form' `
    -Actual $defaultEndpoint.AbsoluteUri `
    -Expected $defaultEndpointValue

Assert-Equal -Name 'host case is normalized before any network call' `
    -Actual (
        Resolve-FoundryNetworkEndpoint -Value (
            'https://FORGE-EAST.services.ai.azure.com/api/projects/forge-prod'
        )
    ).AbsoluteUri `
    -Expected $namedEndpointValue

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
    'bare suffix host is rejected' = @(
        'https://services.ai.azure.com/api/projects/forge-prod',
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
    'account host without a project path is rejected' = @(
        'https://forge-east.services.ai.azure.com/',
        'project_endpoint_path_invalid'
    )
    'extra path is rejected' = @(
        'https://forge-east.services.ai.azure.com/api/projects/forge-prod/extra',
        'project_endpoint_path_invalid'
    )
    'trailing slash is rejected' = @(
        'https://forge-east.services.ai.azure.com/api/projects/forge-prod/',
        'project_endpoint_path_invalid'
    )
    'encoded slash is rejected' = @(
        'https://forge-east.services.ai.azure.com/api/projects/forge%2Fprod',
        'project_endpoint_path_invalid'
    )
    'empty project name is rejected' = @(
        'https://forge-east.services.ai.azure.com/api/projects/',
        'project_endpoint_path_invalid'
    )
    'relative value is rejected' = @(
        '/api/projects/forge-prod',
        'project_endpoint_invalid'
    )
}
foreach ($case in $invalidEndpoints.GetEnumerator()) {
    $value = $case.Value[0]
    $code = $case.Value[1]
    Assert-ThrowsCode -Name $case.Key -Code $code -Action {
        Resolve-FoundryNetworkEndpoint -Value $value
    }
}

Invoke-TestCase -Name 'both smoke tools make the same endpoint decision' -Action {
    $corpus = @($namedEndpointValue, $defaultEndpointValue) +
        @($invalidEndpoints.GetEnumerator() | ForEach-Object { $_.Value[0] })
    foreach ($candidate in $corpus) {
        $networkCode = 'accepted'
        $networkValue = $null
        try {
            $networkValue = (
                Resolve-FoundryNetworkEndpoint -Value $candidate
            ).AbsoluteUri
        }
        catch {
            $networkCode = $_.Exception.Message.Replace(
                'foundry_network:',
                ''
            )
        }
        $prerequisiteCode = 'accepted'
        $prerequisiteValue = $null
        try {
            $prerequisiteValue = (
                Resolve-FoundryProjectEndpoint -Value $candidate
            ).AbsoluteUri
        }
        catch {
            $prerequisiteCode = $_.Exception.Message.Replace(
                'foundry_prerequisite:',
                ''
            )
        }
        Assert-Equal -Name "endpoint decision agrees for '$candidate'" `
            -Actual $networkCode `
            -Expected $prerequisiteCode
        Assert-Equal -Name "endpoint canonical form agrees for '$candidate'" `
            -Actual $networkValue `
            -Expected $prerequisiteValue
    }
}

Invoke-TestCase -Name 'private and public addresses are classified' -Action {
    $privateAddresses = @(
        '10.1.2.3',
        '172.16.0.9',
        '172.31.255.254',
        '192.168.4.5',
        '100.72.0.1',
        'fd00::1'
    )
    foreach ($address in $privateAddresses) {
        Assert-True -Name "$address is private" `
            -Condition (
                Test-FoundryPrivateAddress `
                    -Address ([System.Net.IPAddress]::Parse($address))
            )
    }
    $publicAddresses = @('20.50.1.1', '172.32.0.1', '8.8.8.8', '2606:4700::1')
    foreach ($address in $publicAddresses) {
        Assert-True -Name "$address is public" `
            -Condition (
                -not (
                    Test-FoundryPrivateAddress `
                        -Address ([System.Net.IPAddress]::Parse($address))
                )
            )
    }
}

Invoke-TestCase -Name 'DNS results are reduced to counts and a hash' -Action {
    $result = ConvertTo-RedactedFoundryDnsResult -Addresses @(
        '10.42.2.7',
        '10.42.2.8'
    )
    Assert-Equal -Name 'DNS projection has exact fields' `
        -Actual ($result.PSObject.Properties.Name -join ',') `
        -Expected (
            'resolved,addressCount,addressFamilies,privateAddressCount,' +
            'allAddressesPrivate,addressSetHash'
        )
    Assert-Equal -Name 'DNS address count is exact' `
        -Actual $result.addressCount `
        -Expected 2
    Assert-True -Name 'private endpoint answers are marked private' `
        -Condition $result.allAddressesPrivate
    $json = $result | ConvertTo-Json -Depth 8 -Compress
    Assert-True -Name 'DNS projection never carries a raw address' `
        -Condition (
            -not $json.Contains('10.42.2.7') -and
            -not $json.Contains('10.42.2.8')
        )
    Assert-True -Name 'DNS hash is a sha256 label' `
        -Condition ($result.addressSetHash -match '^sha256:[0-9a-f]{64}$')

    $publicResult = ConvertTo-RedactedFoundryDnsResult -Addresses @(
        '20.50.1.1',
        '10.0.0.1'
    )
    Assert-True -Name 'a mixed answer is not treated as private egress' `
        -Condition (-not $publicResult.allAddressesPrivate)
    Assert-Equal -Name 'mixed answer still counts private addresses' `
        -Actual $publicResult.privateAddressCount `
        -Expected 1
}

Assert-ThrowsCode -Name 'an empty DNS answer fails closed' `
    -Code 'dns_unresolved' `
    -Action { ConvertTo-RedactedFoundryDnsResult -Addresses @() }

Assert-ThrowsCode -Name 'a non-address DNS answer fails closed' `
    -Code 'dns_result_invalid' `
    -Action {
        ConvertTo-RedactedFoundryDnsResult -Addresses @('not-an-address')
    }

Assert-ThrowsCode -Name 'an unbounded DNS answer fails closed' `
    -Code 'dns_result_invalid' `
    -Action {
        ConvertTo-RedactedFoundryDnsResult -Addresses @(
            1..65 | ForEach-Object { "10.0.0.$_" }
        )
    }

Invoke-TestCase -Name 'TLS results are reduced to hashes and booleans' -Action {
    $result = ConvertTo-RedactedFoundryTlsResult -Value ([pscustomobject]@{
        negotiatedProtocol = 'Tls13'
        certificateSubject = 'CN=must-not-leak.services.ai.azure.com'
        certificateIssuer = 'CN=must-not-leak issuing CA'
        certificateNotAfter = '2099-01-01T00:00:00.0000000+00:00'
        sessionKey = 'must-not-leak'
    })
    Assert-Equal -Name 'TLS projection has exact fields' `
        -Actual ($result.PSObject.Properties.Name -join ',') `
        -Expected (
            'reachable,negotiatedProtocol,protocolIsModern,' +
            'certificateValidated,certificateSubjectHash,' +
            'certificateIssuerHash,certificateNotAfter,certificateExpired'
        )
    Assert-True -Name 'modern TLS is recognized' -Condition $result.protocolIsModern
    Assert-True -Name 'unexpired certificate is not flagged' `
        -Condition (-not $result.certificateExpired)
    $json = $result | ConvertTo-Json -Depth 8 -Compress
    Assert-True -Name 'TLS projection never carries certificate identity' `
        -Condition (-not $json.Contains('must-not-leak'))

    $obsolete = ConvertTo-RedactedFoundryTlsResult -Value ([pscustomobject]@{
        negotiatedProtocol = 'Tls11'
        certificateSubject = 'CN=old'
        certificateIssuer = 'CN=old CA'
        certificateNotAfter = '2000-01-01T00:00:00.0000000+00:00'
    })
    Assert-True -Name 'obsolete TLS is flagged' `
        -Condition (-not $obsolete.protocolIsModern)
    Assert-True -Name 'expired certificate is flagged' `
        -Condition $obsolete.certificateExpired
}

Assert-ThrowsCode -Name 'an unparseable TLS result fails closed' `
    -Code 'tls_result_invalid' `
    -Action {
        ConvertTo-RedactedFoundryTlsResult -Value ([pscustomobject]@{
            negotiatedProtocol = 'Tls13'
            certificateSubject = 'CN=x'
            certificateIssuer = 'CN=y'
            certificateNotAfter = 'not-a-date'
        })
    }

Assert-ThrowsCode -Name 'an incomplete TLS result fails closed' `
    -Code 'tls_result_invalid' `
    -Action {
        ConvertTo-RedactedFoundryTlsResult -Value ([pscustomobject]@{
            negotiatedProtocol = 'Tls13'
        })
    }

Invoke-TestCase -Name 'the deployment route is reduced to counts' -Action {
    $body = @'
{
  "value": [
    {
      "name": "must-not-leak-one",
      "type": "ModelDeployment",
      "modelName": "must-not-leak-model",
      "connectionName": "must-not-leak-connection"
    },
    {
      "name": "must-not-leak-two",
      "type": "SomethingElse"
    }
  ]
}
'@
    $result = ConvertTo-RedactedFoundryDeploymentRoute `
        -StatusCode 200 `
        -Body $body
    Assert-Equal -Name 'route projection has exact fields' `
        -Actual ($result.PSObject.Properties.Name -join ',') `
        -Expected (
            'requested,statusClass,deploymentCount,modelDeploymentCount,' +
            'inventoryHash,additionalPagesAvailable'
        )
    Assert-Equal -Name 'route status class is a class, not a code' `
        -Actual $result.statusClass `
        -Expected '2xx'
    Assert-Equal -Name 'route deployment count is exact' `
        -Actual $result.deploymentCount `
        -Expected 2
    Assert-Equal -Name 'route model deployment count is exact' `
        -Actual $result.modelDeploymentCount `
        -Expected 1
    Assert-True -Name 'route projection has no further pages' `
        -Condition (-not $result.additionalPagesAvailable)
    $json = $result | ConvertTo-Json -Depth 8 -Compress
    Assert-True -Name 'route projection never carries a deployment name' `
        -Condition (-not $json.Contains('must-not-leak'))
    Assert-True -Name 'route hash is a sha256 label' `
        -Condition ($result.inventoryHash -match '^sha256:[0-9a-f]{64}$')

    $paged = ConvertTo-RedactedFoundryDeploymentRoute `
        -StatusCode 200 `
        -Body '{"value":[],"nextLink":"https://forge-east.services.ai.azure.com/x"}'
    Assert-True -Name 'further pages are reported, not followed' `
        -Condition $paged.additionalPagesAvailable
    Assert-Equal -Name 'an empty catalog is still a reachable route' `
        -Actual $paged.deploymentCount `
        -Expected 0
}

$routeFailures = [ordered]@{
    'unauthorized route fails closed' = @(401, 'deployment_route_unauthorized')
    'forbidden route fails closed' = @(403, 'deployment_route_forbidden')
    'missing project fails closed' = @(404, 'deployment_route_not_found')
    'throttled route fails closed' = @(429, 'deployment_route_rate_limited')
    'server error is reported as a class' = @(503, 'deployment_route_http_5xx')
    'redirect is reported as a class' = @(302, 'deployment_route_http_3xx')
}
foreach ($case in $routeFailures.GetEnumerator()) {
    $statusCode = $case.Value[0]
    $code = $case.Value[1]
    Assert-ThrowsCode -Name $case.Key -Code $code -Action {
        ConvertTo-RedactedFoundryDeploymentRoute `
            -StatusCode $statusCode `
            -Body '{"value":[]}'
    }
}

Assert-ThrowsCode -Name 'an empty route body fails closed' `
    -Code 'deployment_route_body_invalid' `
    -Action {
        ConvertTo-RedactedFoundryDeploymentRoute -StatusCode 200 -Body ''
    }

Assert-ThrowsCode -Name 'a non-JSON route body fails closed' `
    -Code 'deployment_route_json_invalid' `
    -Action {
        ConvertTo-RedactedFoundryDeploymentRoute `
            -StatusCode 200 `
            -Body 'not json'
    }

Assert-ThrowsCode -Name 'a route body without a value array fails closed' `
    -Code 'deployment_route_shape_invalid' `
    -Action {
        ConvertTo-RedactedFoundryDeploymentRoute `
            -StatusCode 200 `
            -Body '{"deployments":[]}'
    }

Assert-ThrowsCode -Name 'an unnamed deployment record fails closed' `
    -Code 'deployment_route_shape_invalid' `
    -Action {
        ConvertTo-RedactedFoundryDeploymentRoute `
            -StatusCode 200 `
            -Body '{"value":[{"type":"ModelDeployment"}]}'
    }

Assert-ThrowsCode -Name 'an oversized route body fails closed' `
    -Code 'deployment_route_body_too_large' `
    -Action {
        ConvertTo-RedactedFoundryDeploymentRoute `
            -StatusCode 200 `
            -Body ('{"value":[]}' + (' ' * 2048)) `
            -MaximumBytes 1024
    }

# `.invalid` is reserved by RFC 2606 and must never resolve, so these two cases
# exercise the real (non-injected) resolver and TLS boundaries without needing
# any network, any Azure resource, or any real Foundry host.
Assert-ThrowsCode -Name 'the real resolver fails closed on a reserved name' `
    -Code 'dns_unresolved' `
    -Action { Resolve-FoundryNetworkDns -HostName 'forge-smoke.invalid' }

Assert-ThrowsCode -Name 'the real TLS probe fails closed on a reserved name' `
    -Code 'tls_unreachable' `
    -Action {
        Test-FoundryNetworkTls `
            -HostName 'forge-smoke.invalid' `
            -Port 443 `
            -TimeoutSeconds 5
    }

Invoke-TestCase -Name 'an invalid endpoint reaches no network boundary' -Action {
    $reportPath = Join-Path `
        ([System.IO.Path]::GetTempPath()) `
        "forge-foundry-network-shape-$([guid]::NewGuid().ToString('N')).json"
    $calls = [pscustomobject]@{ dns = 0; tls = 0; token = 0; route = 0 }
    try {
        $exitCode = Invoke-FoundryNetworkSmoke `
            -ProjectEndpointValue 'https://forge-east.services.ai.azure.com.evil.example/api/projects/forge-prod' `
            -ManagedIdentityClientIdValue '' `
            -ReportPath $reportPath `
            -DnsResolver {
                param([string] $HostName)
                $calls.dns += 1
                throw 'the resolver must not run before the shape is proved'
            }.GetNewClosure() `
            -TlsProbe {
                param([string] $HostName, [int] $Port)
                $calls.tls += 1
                throw 'the TLS probe must not run before the shape is proved'
            }.GetNewClosure() `
            -TokenProvider {
                param([string] $Scope, [string] $ClientId)
                $calls.token += 1
                throw 'the token provider must not run before the shape is proved'
            }.GetNewClosure() `
            -DeploymentProbe {
                param([uri] $Uri, [string] $AccessToken)
                $calls.route += 1
                throw 'the route probe must not run before the shape is proved'
            }.GetNewClosure()
        Assert-Equal -Name 'an invalid endpoint is not ready' `
            -Actual $exitCode `
            -Expected 2
        Assert-Equal -Name 'no network boundary was touched' `
            -Actual "$($calls.dns),$($calls.tls),$($calls.token),$($calls.route)" `
            -Expected '0,0,0,0'
        $report = Get-Content -Raw -LiteralPath $reportPath |
            ConvertFrom-Json -Depth 32
        Assert-True -Name 'a lookalike host is a stable blocker' `
            -Condition (
                @($report.blockers).Contains('project_endpoint_host_invalid')
            )
        Assert-Equal -Name 'an unvalidated endpoint is never echoed' `
            -Actual $report.projectEndpoint `
            -Expected $null
        Assert-Equal -Name 'egress posture is unknown without DNS' `
            -Actual $report.egress.posture `
            -Expected 'unknown'
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

Invoke-TestCase -Name 'an unresolvable host stops before TLS and token' -Action {
    $reportPath = Join-Path `
        ([System.IO.Path]::GetTempPath()) `
        "forge-foundry-network-dns-$([guid]::NewGuid().ToString('N')).json"
    $calls = [pscustomobject]@{ tls = 0; token = 0 }
    try {
        $exitCode = Invoke-FoundryNetworkSmoke `
            -ProjectEndpointValue $namedEndpointValue `
            -ManagedIdentityClientIdValue '' `
            -ReportPath $reportPath `
            -DnsResolver {
                param([string] $HostName)
                Throw-FoundryNetworkFailure -Code 'dns_unresolved'
            } `
            -TlsProbe {
                param([string] $HostName, [int] $Port)
                $calls.tls += 1
                throw 'TLS must not be attempted without a resolved name'
            }.GetNewClosure() `
            -TokenProvider {
                param([string] $Scope, [string] $ClientId)
                $calls.token += 1
                throw 'no token may be requested for an unreachable host'
            }.GetNewClosure() `
            -DeploymentProbe {
                param([uri] $Uri, [string] $AccessToken)
                throw 'no route may be probed for an unreachable host'
            }
        Assert-Equal -Name 'an unresolvable host is not ready' `
            -Actual $exitCode `
            -Expected 2
        Assert-Equal -Name 'TLS and token were not attempted' `
            -Actual "$($calls.tls),$($calls.token)" `
            -Expected '0,0'
        $report = Get-Content -Raw -LiteralPath $reportPath |
            ConvertFrom-Json -Depth 32
        Assert-True -Name 'DNS failure is explicit' `
            -Condition (@($report.blockers).Contains('dns_unresolved'))
        Assert-True -Name 'a validated endpoint is still recorded' `
            -Condition ($report.checks.endpointValidated)
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

Invoke-TestCase -Name 'a malformed identity client ID fails closed' -Action {
    $reportPath = Join-Path `
        ([System.IO.Path]::GetTempPath()) `
        "forge-foundry-network-identity-$([guid]::NewGuid().ToString('N')).json"
    try {
        $exitCode = Invoke-FoundryNetworkSmoke `
            -ProjectEndpointValue $namedEndpointValue `
            -ManagedIdentityClientIdValue 'not-a-guid' `
            -ReportPath $reportPath `
            -DnsResolver {
                param([string] $HostName)
                Throw-FoundryNetworkFailure -Code 'dns_unresolved'
            } `
            -TlsProbe {
                param([string] $HostName, [int] $Port)
                throw 'unreachable'
            } `
            -TokenProvider {
                param([string] $Scope, [string] $ClientId)
                throw 'unreachable'
            } `
            -DeploymentProbe {
                param([uri] $Uri, [string] $AccessToken)
                throw 'unreachable'
            }
        Assert-Equal -Name 'a malformed client ID is not ready' `
            -Actual $exitCode `
            -Expected 2
        $report = Get-Content -Raw -LiteralPath $reportPath |
            ConvertFrom-Json -Depth 32
        Assert-True -Name 'a malformed client ID is a stable blocker' `
            -Condition (
                @($report.blockers).Contains(
                    'managed_identity_client_id_invalid'
                )
            )
        $reportJson = $report | ConvertTo-Json -Depth 32 -Compress
        Assert-True -Name 'a rejected client ID is never echoed' `
            -Condition (-not $reportJson.Contains('not-a-guid'))
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
        "forge-foundry-network-blocked-$([guid]::NewGuid().ToString('N')).json"
    try {
        $output = & pwsh -NoProfile -File "$PSScriptRoot/network-smoke.ps1" `
            -ProjectEndpoint '' `
            -ManagedIdentityClientId '' `
            -OutputPath $reportPath 2>&1
        $exitCode = $LASTEXITCODE
        Assert-Equal -Name 'blocked network smoke exit code' `
            -Actual $exitCode `
            -Expected 2
        Assert-True -Name 'blocked network report exists' `
            -Condition (Test-Path -LiteralPath $reportPath -PathType Leaf)
        $report = Get-Content -Raw -LiteralPath $reportPath |
            ConvertFrom-Json -Depth 32
        Assert-True -Name 'missing-input report is not ready' `
            -Condition (-not $report.ready)
        $requiredBlockers = @(
            'deployment_route_unavailable',
            'dns_unresolved',
            'managed_identity_token_unavailable',
            'project_endpoint_missing',
            'tls_unreachable'
        )
        Assert-True -Name 'missing-input blockers are all explicit' `
            -Condition (
                @(
                    $requiredBlockers |
                        Where-Object { $_ -notin $report.blockers }
                ).Count -eq 0
            )
        $reportJson = $report | ConvertTo-Json -Depth 32 -Compress
        Assert-True -Name 'blocked report does not contain credentials' `
            -Condition (
                -not $reportJson.Contains('accessToken') -and
                -not $reportJson.Contains('access_token') -and
                -not $reportJson.Contains('Authorization') -and
                -not $reportJson.Contains('Bearer')
            )
        Assert-True -Name 'blocked summary is printed without a host' `
            -Condition (
                ($output -join "`n").Contains('ready=False')
            )
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

Invoke-TestCase -Name 'a complete network path becomes ready without Azure' -Action {
    $reportPath = Join-Path `
        ([System.IO.Path]::GetTempPath()) `
        "forge-foundry-network-ready-$([guid]::NewGuid().ToString('N')).json"
    $identityClientId = '11111111-2222-3333-4444-555555555555'
    $observed = [pscustomobject]@{ scope = ''; clientId = ''; routeUri = '' }
    try {
        $exitCode = Invoke-FoundryNetworkSmoke `
            -ProjectEndpointValue $namedEndpointValue `
            -ManagedIdentityClientIdValue $identityClientId `
            -ReportPath $reportPath `
            -DnsResolver {
                param([string] $HostName)
                if ($HostName -cne 'forge-east.services.ai.azure.com') {
                    throw "resolver received an unexpected host: $HostName"
                }
                return @('10.42.2.7')
            } `
            -TlsProbe {
                param([string] $HostName, [int] $Port)
                if (
                    $HostName -cne 'forge-east.services.ai.azure.com' -or
                    $Port -ne 443
                ) {
                    throw 'TLS probe did not carry the validated host and port'
                }
                return [pscustomobject]@{
                    negotiatedProtocol = 'Tls13'
                    certificateSubject = 'CN=forge-east.services.ai.azure.com'
                    certificateIssuer = 'CN=synthetic issuing CA'
                    certificateNotAfter = '2099-01-01T00:00:00.0000000+00:00'
                }
            } `
            -TokenProvider {
                param([string] $Scope, [string] $ClientId)
                $observed.scope = $Scope
                $observed.clientId = $ClientId
                return [pscustomobject]@{
                    source = 'synthetic'
                    accessToken = 'synthetic-secret-token-must-not-leak'
                }
            }.GetNewClosure() `
            -DeploymentProbe {
                param([uri] $Uri, [string] $AccessToken)
                $observed.routeUri = $Uri.AbsoluteUri
                if ($AccessToken -cne 'synthetic-secret-token-must-not-leak') {
                    throw 'the token was not kept at the HTTP boundary'
                }
                return [pscustomobject]@{
                    statusCode = 200
                    body = @'
{
  "value": [
    { "name": "must-not-leak-deployment", "type": "ModelDeployment" }
  ]
}
'@
                }
            }.GetNewClosure()
        $report = Get-Content -Raw -LiteralPath $reportPath |
            ConvertFrom-Json -Depth 32
        Assert-Equal -Name (
            'ready network smoke exit code; blockers=' +
            (@($report.blockers) -join ',')
        ) `
            -Actual $exitCode `
            -Expected 0
        Assert-True -Name 'complete network report is ready' `
            -Condition $report.ready
        Assert-Equal -Name 'complete network report has no blockers' `
            -Actual @($report.blockers).Count `
            -Expected 0
        Assert-True -Name 'complete report records every stage' `
            -Condition (
                $report.checks.endpointValidated -and
                $report.checks.dnsResolved -and
                $report.checks.tlsReachable -and
                $report.checks.managedIdentityTokenAcquired -and
                $report.checks.deploymentRouteReachable
            )
        Assert-Equal -Name 'the documented token scope is requested' `
            -Actual $observed.scope `
            -Expected 'https://ai.azure.com/.default'
        Assert-Equal -Name 'the supplied identity is requested' `
            -Actual $observed.clientId `
            -Expected $identityClientId
        Assert-Equal -Name 'the documented deployment route is requested' `
            -Actual $observed.routeUri `
            -Expected (
                'https://forge-east.services.ai.azure.com/api/projects/' +
                'forge-prod/deployments' +
                '?api-version=v1&deploymentType=ModelDeployment'
            )
        Assert-Equal -Name 'a private answer is reported as private egress' `
            -Actual $report.egress.posture `
            -Expected 'private-address-egress'
        Assert-Equal -Name 'the route reports one deployment' `
            -Actual $report.deploymentRoute.deploymentCount `
            -Expected 1
        $reportJson = $report | ConvertTo-Json -Depth 32 -Compress
        Assert-True -Name 'ready report redacts token, names, and addresses' `
            -Condition (
                -not $reportJson.Contains('synthetic-secret-token-must-not-leak') -and
                -not $reportJson.Contains('must-not-leak-deployment') -and
                -not $reportJson.Contains('10.42.2.7') -and
                -not $reportJson.Contains($identityClientId) -and
                -not $reportJson.Contains('CN=')
            )
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

Invoke-TestCase -Name 'an unauthorized route keeps the smoke blocked' -Action {
    $reportPath = Join-Path `
        ([System.IO.Path]::GetTempPath()) `
        "forge-foundry-network-denied-$([guid]::NewGuid().ToString('N')).json"
    try {
        $exitCode = Invoke-FoundryNetworkSmoke `
            -ProjectEndpointValue $namedEndpointValue `
            -ManagedIdentityClientIdValue '' `
            -ReportPath $reportPath `
            -DnsResolver {
                param([string] $HostName)
                return @('20.50.1.1')
            } `
            -TlsProbe {
                param([string] $HostName, [int] $Port)
                return [pscustomobject]@{
                    negotiatedProtocol = 'Tls13'
                    certificateSubject = 'CN=forge-east.services.ai.azure.com'
                    certificateIssuer = 'CN=synthetic issuing CA'
                    certificateNotAfter = '2099-01-01T00:00:00.0000000+00:00'
                }
            } `
            -TokenProvider {
                param([string] $Scope, [string] $ClientId)
                return [pscustomobject]@{
                    source = 'synthetic'
                    accessToken = 'synthetic-secret-token-must-not-leak'
                }
            } `
            -DeploymentProbe {
                param([uri] $Uri, [string] $AccessToken)
                return [pscustomobject]@{
                    statusCode = 403
                    body = '{"error":{"message":"must-not-leak"}}'
                }
            }
        Assert-Equal -Name 'a denied route is not ready' `
            -Actual $exitCode `
            -Expected 2
        $report = Get-Content -Raw -LiteralPath $reportPath |
            ConvertFrom-Json -Depth 32
        Assert-True -Name 'a denied route is an explicit blocker' `
            -Condition (
                @($report.blockers).Contains('deployment_route_forbidden')
            )
        Assert-Equal -Name 'a public answer is reported as public egress' `
            -Actual $report.egress.posture `
            -Expected 'public-address-egress'
        $reportJson = $report | ConvertTo-Json -Depth 32 -Compress
        Assert-True -Name 'a denied response body never reaches the report' `
            -Condition (
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
    throw "Foundry network smoke tests failed: $($script:failures.Count)"
}

Write-Output "Foundry network smoke tests passed: $script:passes"
