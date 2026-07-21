# Local Docker smoke for the desktop-support API (testing only).
# Mirrors the unauthenticated assertions of deployed-smoke.ps1: bounded
# health shape and fail-closed authenticated routes. It cannot exercise
# token-bearing paths; those remain the deployed gate's job.
[CmdletBinding()]
param(
    [string] $SupportOrigin = 'http://localhost:8080'
)

$ErrorActionPreference = 'Stop'

function Assert-True([bool] $Condition, [string] $Message) {
    if (-not $Condition) { throw "LOCAL SMOKE FAIL: $Message" }
}

Write-Host '== health: only status and safe dependency classes may appear'
foreach ($probe in @('healthz/live', 'healthz/ready')) {
    $health = Invoke-RestMethod -Uri "$SupportOrigin/$probe" -Method Get
    Assert-True ($null -ne $health.status) "$probe status present"
    Assert-True ($null -eq ($health.PSObject.Properties.Name | Where-Object { $_ -notin @('status', 'dependencies') })) "$probe shape is bounded"
    Assert-True ($health.status -eq 'healthy') "$probe reports healthy"
}

Write-Host '== authenticated routes fail closed without a token'
foreach ($route in @('desktop/v1/bootstrap', 'desktop/v1/policy/current')) {
    $rejected = $null
    try {
        Invoke-RestMethod -Uri "$SupportOrigin/$route" -Method Get
    } catch { $rejected = $_.Exception.Response.StatusCode.value__ }
    Assert-True ($rejected -eq 401) "$route rejects anonymous access with 401 (got '$rejected')"
}

Write-Host '== forged bearer tokens fail closed'
$rejected = $null
try {
    Invoke-RestMethod -Uri "$SupportOrigin/desktop/v1/bootstrap" `
        -Headers @{ Authorization = 'Bearer not-a-real-token' }
} catch { $rejected = $_.Exception.Response.StatusCode.value__ }
Assert-True ($rejected -eq 401) "forged token rejected with 401 (got '$rejected')"

Write-Host 'LOCAL SMOKE PASS'
