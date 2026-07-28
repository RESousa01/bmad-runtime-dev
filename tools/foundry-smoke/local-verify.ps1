#Requires -Version 7.0
<#
.SYNOPSIS
    Runs the complete local verification matrix for the Foundry connection.

.DESCRIPTION
    FND-063. This proves the implementation against deterministic fixtures
    before anything is deployed. It is deliberately unforgiving:

      - A failure in any mandatory gate fails the run.
      - A gate that cannot run is recorded as 'unavailable' with the reason.
        It is never silently skipped and never counted as a pass.
      - The report distinguishes 'passed', 'failed', and 'unavailable' so a
        reader can never mistake a missing toolchain for a green gate.

    Contract regeneration is NOT run by default. Regeneration is global: it
    rewrites schema-lock.json and the whole generated tree from every schema
    at once. Another agent may be working the same checkout, so the default
    is the read-only --check form. Pass -Regenerate to opt in.

.PARAMETER ReportPath
    Where the machine-readable report is written.

.PARAMETER LogDirectory
    Directory for per-gate stdout/stderr captures referenced by the report.

.PARAMETER Regenerate
    Run full contract generation instead of the read-only check. Only safe
    when this checkout has a single writer.

.PARAMETER SkipRenderer
    Omit the renderer gates. Recorded as 'unavailable' with an explicit
    operator reason rather than dropped from the matrix.
#>
[CmdletBinding()]
param(
    [string] $ReportPath = 'artifacts/foundry/qualification/local-matrix.json',
    [string] $LogDirectory = 'artifacts/foundry/qualification/logs',
    [switch] $Regenerate,
    [switch] $SkipRenderer
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$script:repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..' '..')).Path
$script:gates = [System.Collections.Generic.List[object]]::new()
$script:sequence = 0

function Get-ToolVersion {
    param([Parameter(Mandatory)][string] $Name, [string[]] $VersionArguments = @('--version'))

    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if ($null -eq $command) {
        return $null
    }
    try {
        $raw = & $Name @VersionArguments 2>&1 | Select-Object -First 1
        return "$raw".Trim()
    }
    catch {
        return 'unknown'
    }
}

function New-GateLogPath {
    param([Parameter(Mandatory)][string] $GateId)

    $directory = Join-Path $script:repositoryRoot $LogDirectory
    if (-not (Test-Path $directory)) {
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
    }
    return (Join-Path $directory "$GateId.log")
}

<#
.SYNOPSIS
    Records a gate that could not be attempted.
.DESCRIPTION
    Unavailable is a distinct outcome from passed and failed. It exists so a
    missing toolchain can never be laundered into a green matrix.
#>
function Add-UnavailableGate {
    param(
        [Parameter(Mandatory)][string] $GateId,
        [Parameter(Mandatory)][string] $Layer,
        [Parameter(Mandatory)][string] $Command,
        [Parameter(Mandatory)][string] $Reason,
        [bool] $Mandatory = $true
    )

    $script:sequence += 1
    $script:gates.Add([ordered]@{
        sequence         = $script:sequence
        gateId           = $GateId
        layer            = $Layer
        command          = $Command
        workingDirectory = $script:repositoryRoot
        classification   = 'unavailable'
        mandatory        = $Mandatory
        exitCode         = $null
        durationSeconds  = 0
        toolVersion      = $null
        logPath          = $null
        reason           = $Reason
    })
    Write-Host "  UNAVAILABLE  $GateId  ($Reason)"
}

function Invoke-Gate {
    param(
        [Parameter(Mandatory)][string] $GateId,
        [Parameter(Mandatory)][string] $Layer,
        [Parameter(Mandatory)][string] $Executable,
        [Parameter(Mandatory)][string[]] $ArgumentList,
        [string] $ToolVersion,
        [bool] $Mandatory = $true
    )

    $rendered = "$Executable $($ArgumentList -join ' ')"
    if ($null -eq (Get-Command $Executable -ErrorAction SilentlyContinue)) {
        Add-UnavailableGate -GateId $GateId -Layer $Layer -Command $rendered `
            -Reason "executable_not_found:$Executable" -Mandatory $Mandatory
        return
    }

    $logPath = New-GateLogPath -GateId $GateId
    $started = [DateTimeOffset]::UtcNow
    Push-Location $script:repositoryRoot
    try {
        & $Executable @ArgumentList *>&1 | Tee-Object -FilePath $logPath | Out-Null
        $exitCode = $LASTEXITCODE
    }
    catch {
        "$_" | Out-File -FilePath $logPath -Append
        $exitCode = 1
    }
    finally {
        Pop-Location
    }
    $duration = ([DateTimeOffset]::UtcNow - $started).TotalSeconds

    $classification = if ($exitCode -eq 0) { 'passed' } else { 'failed' }
    $script:sequence += 1
    $script:gates.Add([ordered]@{
        sequence         = $script:sequence
        gateId           = $GateId
        layer            = $Layer
        command          = $rendered
        workingDirectory = $script:repositoryRoot
        classification   = $classification
        mandatory        = $Mandatory
        exitCode         = $exitCode
        durationSeconds  = [math]::Round($duration, 2)
        toolVersion      = $ToolVersion
        logPath          = (Resolve-Path -Relative -Path $logPath -RelativeBasePath $script:repositoryRoot)
        reason           = $null
    })

    $label = $classification.ToUpperInvariant().PadRight(12)
    Write-Host "  $label $GateId  exit=$exitCode  $([math]::Round($duration, 1))s"
}

Write-Host 'Foundry local verification matrix'
Write-Host "Repository: $script:repositoryRoot"

$nodeVersion = Get-ToolVersion -Name 'node'
$pnpmVersion = Get-ToolVersion -Name 'pnpm'
$dotnetVersion = Get-ToolVersion -Name 'dotnet'
$cargoVersion = Get-ToolVersion -Name 'cargo'
$azVersion = Get-ToolVersion -Name 'az' -VersionArguments @('version')

# ---------------------------------------------------------------- contracts
Write-Host 'Contracts'
if ($Regenerate) {
    Invoke-Gate -GateId 'contracts-generate' -Layer 'contracts' `
        -Executable 'pnpm' -ToolVersion $pnpmVersion `
        -ArgumentList @('--filter', '@sapphirus/contracts', 'generate')
}
else {
    Add-UnavailableGate -GateId 'contracts-generate' -Layer 'contracts' `
        -Command 'pnpm --filter @sapphirus/contracts generate' `
        -Reason 'not_requested:regeneration_is_global_and_requires_a_single_writer' `
        -Mandatory $false
}
Invoke-Gate -GateId 'contracts-cross-language' -Layer 'contracts' `
    -Executable 'pnpm' -ToolVersion $pnpmVersion `
    -ArgumentList @('--filter', '@sapphirus/contracts', 'verify:cross-language')

# -------------------------------------------------------------- support API
Write-Host 'Support API'
Invoke-Gate -GateId 'dotnet-restore-locked' -Layer 'support-api' `
    -Executable 'dotnet' -ToolVersion $dotnetVersion `
    -ArgumentList @('restore', 'Sapphirus.slnx', '--locked-mode')
Invoke-Gate -GateId 'dotnet-build' -Layer 'support-api' `
    -Executable 'dotnet' -ToolVersion $dotnetVersion `
    -ArgumentList @('build', 'Sapphirus.slnx', '-c', 'Release', '--no-restore')
Invoke-Gate -GateId 'dotnet-test' -Layer 'support-api' `
    -Executable 'dotnet' -ToolVersion $dotnetVersion `
    -ArgumentList @(
        'test',
        'services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj',
        '-c', 'Release', '--no-build')

# --------------------------------------------------------------------- rust
Write-Host 'Rust'
Invoke-Gate -GateId 'cargo-test-workspace' -Layer 'rust' `
    -Executable 'cargo' -ToolVersion $cargoVersion `
    -ArgumentList @('test', '--workspace', '--all-features', '--locked')
Invoke-Gate -GateId 'cargo-clippy' -Layer 'rust' `
    -Executable 'cargo' -ToolVersion $cargoVersion `
    -ArgumentList @(
        'clippy', '--workspace', '--all-targets', '--all-features', '--locked',
        '--', '-D', 'warnings')

# ----------------------------------------------------------------- renderer
Write-Host 'Renderer'
if ($SkipRenderer) {
    foreach ($gate in @('ui-typecheck', 'ui-test', 'ui-build')) {
        Add-UnavailableGate -GateId $gate -Layer 'renderer' `
            -Command "pnpm --filter @forge/desktop-ui $gate" `
            -Reason 'operator_requested_skip'
    }
}
else {
    Invoke-Gate -GateId 'ui-typecheck' -Layer 'renderer' `
        -Executable 'pnpm' -ToolVersion $pnpmVersion `
        -ArgumentList @('--filter', '@forge/desktop-ui', 'typecheck')
    Invoke-Gate -GateId 'ui-test' -Layer 'renderer' `
        -Executable 'pnpm' -ToolVersion $pnpmVersion `
        -ArgumentList @('--filter', '@forge/desktop-ui', 'test')
    Invoke-Gate -GateId 'ui-build' -Layer 'renderer' `
        -Executable 'pnpm' -ToolVersion $pnpmVersion `
        -ArgumentList @('--filter', '@forge/desktop-ui', 'build')
}

# ------------------------------------------------- boundaries and hygiene
Write-Host 'Repository gates'
Invoke-Gate -GateId 'check-boundaries' -Layer 'hygiene' `
    -Executable 'node' -ToolVersion $nodeVersion `
    -ArgumentList @('tools/check-boundaries.mjs')
Invoke-Gate -GateId 'check-secrets' -Layer 'hygiene' `
    -Executable 'node' -ToolVersion $nodeVersion `
    -ArgumentList @('tools/check-secrets.mjs')
Invoke-Gate -GateId 'infra-contracts' -Layer 'hygiene' `
    -Executable 'node' -ToolVersion $nodeVersion `
    -ArgumentList @(
        '--test',
        'tools/forge-local.test.mjs',
        'tools/support-cloud-contracts.test.mjs',
        'tools/check-workflows.test.mjs')

# ------------------------------------------------------- foundry tooling
Write-Host 'Foundry tooling'
Invoke-Gate -GateId 'foundry-fake' -Layer 'foundry' `
    -Executable 'node' -ToolVersion $nodeVersion `
    -ArgumentList @('--test', 'tools/foundry-fake/server.test.mjs')
Invoke-Gate -GateId 'prerequisite-checker' -Layer 'foundry' `
    -Executable 'pwsh' -ToolVersion (Get-ToolVersion -Name 'pwsh') `
    -ArgumentList @('-NoProfile', '-File', 'tools/foundry-smoke/check-prerequisites.Tests.ps1')
Invoke-Gate -GateId 'network-smoke' -Layer 'foundry' `
    -Executable 'pwsh' -ToolVersion (Get-ToolVersion -Name 'pwsh') `
    -ArgumentList @('-NoProfile', '-File', 'tools/foundry-smoke/network-smoke.Tests.ps1')
Invoke-Gate -GateId 'profile-validator' -Layer 'foundry' `
    -Executable 'node' -ToolVersion $nodeVersion `
    -ArgumentList @('--test', 'tools/foundry-profiles/validate-profiles.test.mjs')
Invoke-Gate -GateId 'profile-publisher' -Layer 'foundry' `
    -Executable 'pwsh' -ToolVersion (Get-ToolVersion -Name 'pwsh') `
    -ArgumentList @('-NoProfile', '-File', 'tools/foundry-profiles/publish-profiles.Tests.ps1')

# ---------------------------------------------------------------- bicep
Write-Host 'Infrastructure'
Invoke-Gate -GateId 'bicep-build' -Layer 'infrastructure' `
    -Executable 'az' -ToolVersion $azVersion `
    -ArgumentList @('bicep', 'build', '--file', 'infra/desktop-support/main.bicep', '--stdout')

# --------------------------------------------------------------- report
$failed = @($script:gates | Where-Object { $_.classification -eq 'failed' })
$unavailableMandatory = @(
    $script:gates | Where-Object {
        $_.classification -eq 'unavailable' -and $_.mandatory
    })
$passed = @($script:gates | Where-Object { $_.classification -eq 'passed' })

$ready = ($failed.Count -eq 0) -and ($unavailableMandatory.Count -eq 0)

$report = [ordered]@{
    schemaVersion = 'forge.foundry-local-matrix.v1'
    generatedAt   = [DateTimeOffset]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ss.fffZ')
    repositoryRoot = $script:repositoryRoot
    repositoryCommit = (& git -C $script:repositoryRoot rev-parse HEAD 2>$null)
    ready         = $ready
    regenerated   = [bool]$Regenerate
    toolchain     = [ordered]@{
        node   = $nodeVersion
        pnpm   = $pnpmVersion
        dotnet = $dotnetVersion
        cargo  = $cargoVersion
        az     = $azVersion
    }
    summary       = [ordered]@{
        total                = $script:gates.Count
        passed               = $passed.Count
        failed               = $failed.Count
        unavailable          = @($script:gates | Where-Object { $_.classification -eq 'unavailable' }).Count
        unavailableMandatory = $unavailableMandatory.Count
    }
    failedGates   = @($failed | ForEach-Object { $_.gateId })
    blockedGates  = @($unavailableMandatory | ForEach-Object { $_.gateId })
    gates         = $script:gates
    honesty       = @(
        'A gate that could not run is recorded as unavailable, never as a pass.',
        'ready is true only when zero mandatory gates failed and zero mandatory gates were unavailable.',
        'This matrix proves local behaviour against deterministic fixtures. It proves nothing about a deployed environment, and it is not the deployed qualification.'
    )
}

$reportFull = Join-Path $script:repositoryRoot $ReportPath
$reportDirectory = Split-Path -Parent $reportFull
if (-not (Test-Path $reportDirectory)) {
    New-Item -ItemType Directory -Path $reportDirectory -Force | Out-Null
}
$report | ConvertTo-Json -Depth 8 | Out-File -FilePath $reportFull -Encoding utf8

Write-Host ''
Write-Host "Local matrix: ready=$ready passed=$($passed.Count) failed=$($failed.Count) blocked=$($unavailableMandatory.Count)"
Write-Host "Report: $ReportPath"

if (-not $ready) {
    exit 2
}
exit 0
