[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$composeFile = Join-Path $repositoryRoot "infra\local\compose.yaml"
$tauriConfig = Join-Path $repositoryRoot "crates\desktop-app\tauri.conf.json"
$localTauriConfig = Join-Path $repositoryRoot "infra\local\tauri.conf.json"

function Stop-KnownForgeHosts {
    $knownHostPaths = @(
        (Join-Path $env:LOCALAPPDATA "Forge\forge.exe"),
        (Join-Path $repositoryRoot "target\debug\desktop-app.exe"),
        (Join-Path $repositoryRoot "target\debug\forge.exe")
    ) | ForEach-Object { [IO.Path]::GetFullPath($_) }

    foreach ($processName in @("forge", "desktop-app")) {
        foreach ($process in Get-Process -Name $processName -ErrorAction SilentlyContinue) {
            $processPath = $null
            try {
                $processPath = $process.Path
            }
            catch {
                continue
            }
            if (-not $processPath) {
                continue
            }
            $resolvedProcessPath = [IO.Path]::GetFullPath($processPath)
            $isKnownForgeHost = $knownHostPaths | Where-Object {
                [StringComparer]::OrdinalIgnoreCase.Equals($resolvedProcessPath, $_)
            }
            if ($isKnownForgeHost) {
                Write-Host "Closing an older Forge host so the current source can start."
                Stop-Process -Id $process.Id -Force
                Wait-Process -Id $process.Id -Timeout 5 -ErrorAction SilentlyContinue
            }
        }
    }
}

docker info *> $null
if ($LASTEXITCODE -ne 0) {
    throw "Docker Desktop is not ready. Start Docker Desktop and retry."
}

Push-Location $repositoryRoot
try {
    docker compose -f $composeFile up --detach --build --wait
    if ($LASTEXITCODE -ne 0) {
        throw "The local Forge frontend did not become healthy."
    }

    Write-Host "Forge browser preview is ready at http://127.0.0.1:1420/"
    $supportPort = if ([string]::IsNullOrWhiteSpace($env:FORGE_SUPPORT_PORT)) { "18080" } else { $env:FORGE_SUPPORT_PORT }
    Write-Host "Forge support API test seam is healthy at http://127.0.0.1:$supportPort/healthz/live"
    Write-Host "Choose a provider and model, then enter its API key in Forge Settings > Providers."
    Stop-KnownForgeHosts
    pnpm exec tauri dev --config $tauriConfig --config $localTauriConfig --features direct-model-providers --no-watch --no-dev-server-wait
    if ($LASTEXITCODE -ne 0) {
        throw "The Forge native host exited with an error."
    }
}
finally {
    Pop-Location
}
