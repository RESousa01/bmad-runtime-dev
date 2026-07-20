# Deployed support-plane smoke qualification (D2-E Task 11, gates 2-3).
# No tenant, subscription, or endpoint values are embedded: everything comes
# from parameters or the protected-environment configuration.
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $SupportOrigin,
    [Parameter(Mandatory)] [string] $TenantId,
    [Parameter(Mandatory)] [string] $ApiClientId,
    [Parameter(Mandatory)] [string] $Scope,
    [switch] $IncludeModelCall
)

$ErrorActionPreference = 'Stop'

function Assert-True([bool] $Condition, [string] $Message) {
    if (-not $Condition) { throw "SMOKE FAIL: $Message" }
}

Write-Host "== acquiring Entra token for $Scope (workload identity / device context)"
$token = az account get-access-token --scope $Scope --tenant $TenantId --query accessToken -o tsv
Assert-True ($token.Length -gt 100) 'token acquisition'

$headers = @{ Authorization = "Bearer $token" }

Write-Host '== health: only status and safe dependency classes may appear'
$health = Invoke-RestMethod -Uri "$SupportOrigin/healthz/ready" -Method Get
Assert-True ($null -ne $health.status) 'health status present'
Assert-True ($null -eq ($health.PSObject.Properties.Name | Where-Object { $_ -notin @('status', 'dependencies') })) 'health shape is bounded'

Write-Host '== bootstrap'
$bootstrap = Invoke-RestMethod -Uri "$SupportOrigin/desktop/v1/bootstrap" -Headers $headers
Assert-True ($bootstrap.schemaVersion -eq 'sapphirus.desktop-bootstrap.v1') 'bootstrap schema'

Write-Host '== signed policy verifies structurally'
$policy = Invoke-RestMethod -Uri "$SupportOrigin/desktop/v1/policy/current" -Headers $headers
Assert-True ($policy.policyHash -match '^sha256:[0-9a-f]{64}$') 'policy hash shape'
Assert-True ($policy.signature.Length -gt 40) 'policy signature present'
Assert-True ($policy.keyId.Length -gt 8) 'policy key version pinned'

Write-Host '== registration + lease'
$idempotency = [guid]::NewGuid().ToString('N')
# Registration requires a real installation key; the end-to-end gate runs the
# Rust test-installation harness for that step. This script asserts the
# unauthenticated and malformed paths fail closed.
$rejected = $null
try {
    Invoke-RestMethod -Uri "$SupportOrigin/desktop/v1/entitlements/leases" `
        -Method Post -Headers ($headers + @{ 'Idempotency-Key' = $idempotency }) `
        -ContentType 'application/json' -Body '{"registrationId":"dreg_INVALID"}'
} catch { $rejected = $_.Exception.Response.StatusCode.value__ }
Assert-True ($rejected -eq 400 -or $rejected -eq 403) 'invalid lease request fails closed'

if ($IncludeModelCall) {
    Write-Host '== model call is exercised by the Rust end-to-end harness, not this script'
}

Write-Host 'SMOKE PASS'
