[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string] $InstallerPath,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$')]
    [string] $ExpectedVersion,

    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string] $InstallRoot,

    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string] $EvidencePath,

    [string] $FoundationPath = 'packages/bmad-foundation',

    [string] $PriorInstallerPath,

    [ValidatePattern('^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$')]
    [string] $ExpectedPriorVersion,

    [switch] $RequireValidSignature,

    [switch] $SkipLaunchSmoke
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Resolve-ExistingFile {
    param([Parameter(Mandatory = $true)][string] $Path)

    $resolved = Resolve-Path -LiteralPath $Path
    $item = Get-Item -LiteralPath $resolved.Path -Force
    if (-not $item.PSIsContainer -and -not ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        return $item.FullName
    }

    throw "Expected a regular file: $Path"
}

function Resolve-ExistingDirectory {
    param([Parameter(Mandatory = $true)][string] $Path)

    $resolved = Resolve-Path -LiteralPath $Path
    $item = Get-Item -LiteralPath $resolved.Path -Force
    if ($item.PSIsContainer -and -not ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        return $item.FullName.TrimEnd([IO.Path]::DirectorySeparatorChar)
    }

    throw "Expected a regular directory: $Path"
}

function Get-AbsolutePath {
    param([Parameter(Mandatory = $true)][string] $Path)

    if ([IO.Path]::IsPathRooted($Path)) {
        return [IO.Path]::GetFullPath($Path)
    }

    return [IO.Path]::GetFullPath((Join-Path (Get-Location).Path $Path))
}

function Test-ContainedPath {
    param(
        [Parameter(Mandatory = $true)][string] $Candidate,
        [Parameter(Mandatory = $true)][string] $Root
    )

    $rootPrefix = [IO.Path]::GetFullPath($Root).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    return $Candidate.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)
}

function Assert-TemporaryOutputPath {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)][string] $Label
    )

    if ($Path -match '[\s"\r\n]') {
        throw "$Label must be an absolute temporary path without whitespace or quotes."
    }

    $allowedRoots = @([IO.Path]::GetTempPath())
    if (-not [string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
        $allowedRoots += $env:RUNNER_TEMP
    }

    if (-not ($allowedRoots | Where-Object { Test-ContainedPath -Candidate $Path -Root $_ })) {
        throw "$Label must remain below the operating-system or runner temporary directory."
    }
}

function Assert-CleanQualificationAccount {
    $existingProcess = Get-Process -Name 'sapphirus' -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -ne $existingProcess) {
        throw 'A Sapphirus process is already running. Installer qualification requires an isolated account.'
    }

    foreach ($registryPath in @(
        'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Sapphirus',
        'HKCU:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Sapphirus'
    )) {
        if (Test-Path -LiteralPath $registryPath) {
            throw 'An existing Sapphirus uninstall registration was detected. Installer qualification requires an isolated account.'
        }
    }
}

function Invoke-SilentInstaller {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)][string] $Destination
    )

    $process = Start-Process -FilePath $Path -ArgumentList @('/S', "/D=$Destination") -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) {
        throw "Installer exited with code $($process.ExitCode)."
    }
}

function Wait-ForPathState {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)][bool] $Exists,
        [int] $Attempts = 50
    )

    for ($attempt = 0; $attempt -lt $Attempts; $attempt += 1) {
        if ((Test-Path -LiteralPath $Path) -eq $Exists) {
            return
        }
        Start-Sleep -Milliseconds 200
    }

    throw "Timed out waiting for the installer lifecycle path state."
}

function Get-FoundationHashes {
    param([Parameter(Mandatory = $true)][string] $Root)

    $hashes = @{}
    foreach ($file in @('NOTICE.md', 'adoption-ledger.json', 'runtime-manifest.json', 'semantic-source-ledger.json')) {
        $path = Join-Path $Root $file
        $hashes[$file] = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    }

    foreach ($directory in @('licenses', 'normalized', 'runtime')) {
        Get-ChildItem -LiteralPath (Join-Path $Root $directory) -Recurse -File -Force | ForEach-Object {
            if ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Foundation payload contains a reparse point."
            }
            $relativePath = $_.FullName.Substring($Root.Length + 1).Replace('\', '/')
            $hashes[$relativePath] = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    }

    return $hashes
}

function Assert-ExactFoundationPayload {
    param(
        [Parameter(Mandatory = $true)][string] $ExpectedRoot,
        [Parameter(Mandatory = $true)][string] $ActualRoot
    )

    if (-not (Test-Path -LiteralPath $ActualRoot -PathType Container)) {
        throw "Installed BMAD foundation payload is missing."
    }

    $reparsePoint = Get-ChildItem -LiteralPath $ActualRoot -Recurse -Force | Where-Object {
        $_.Attributes -band [IO.FileAttributes]::ReparsePoint
    } | Select-Object -First 1
    if ($null -ne $reparsePoint) {
        throw "Installed BMAD foundation payload contains a reparse point."
    }

    $expected = Get-FoundationHashes -Root $ExpectedRoot
    $actual = @{}
    Get-ChildItem -LiteralPath $ActualRoot -Recurse -File -Force | ForEach-Object {
        $relativePath = $_.FullName.Substring($ActualRoot.Length + 1).Replace('\', '/')
        $actual[$relativePath] = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    }

    $missing = @($expected.Keys | Where-Object { -not $actual.ContainsKey($_) })
    $unexpected = @($actual.Keys | Where-Object { -not $expected.ContainsKey($_) })
    $mismatched = @($expected.Keys | Where-Object { $actual.ContainsKey($_) -and $actual[$_] -ne $expected[$_] })
    if ($missing.Count -ne 0 -or $unexpected.Count -ne 0 -or $mismatched.Count -ne 0) {
        throw "Installed BMAD foundation payload does not match the reviewed source set."
    }

    return $expected.Count
}

$installer = Resolve-ExistingFile -Path $InstallerPath
$foundation = Resolve-ExistingDirectory -Path $FoundationPath
$installDirectory = Get-AbsolutePath -Path $InstallRoot
$evidenceFile = Get-AbsolutePath -Path $EvidencePath
Assert-TemporaryOutputPath -Path $installDirectory -Label 'InstallRoot'
Assert-TemporaryOutputPath -Path $evidenceFile -Label 'EvidencePath'

if (Test-Path -LiteralPath $installDirectory) {
    throw 'InstallRoot must not exist before qualification.'
}

Assert-CleanQualificationAccount

if ([string]::IsNullOrWhiteSpace($PriorInstallerPath) -ne [string]::IsNullOrWhiteSpace($ExpectedPriorVersion)) {
    throw 'PriorInstallerPath and ExpectedPriorVersion must be supplied together.'
}

$signature = Get-AuthenticodeSignature -LiteralPath $installer
if ($RequireValidSignature -and $signature.Status -ne [Management.Automation.SignatureStatus]::Valid) {
    throw "Installer signature is not valid: $($signature.Status)."
}
if ($RequireValidSignature -and $null -eq $signature.TimeStamperCertificate) {
    throw 'Installer signature is valid but is not timestamped.'
}

$installerItem = Get-Item -LiteralPath $installer
$installerHash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
$installedExecutable = Join-Path $installDirectory 'sapphirus.exe'
$installedFoundation = Join-Path $installDirectory 'bmad-foundation'
$uninstaller = Join-Path $installDirectory 'uninstall.exe'
$launchedProcess = $null
$lifecycleComplete = $false

try {
    $priorVersion = $null
    if (-not [string]::IsNullOrWhiteSpace($PriorInstallerPath)) {
        $priorInstaller = Resolve-ExistingFile -Path $PriorInstallerPath
        Invoke-SilentInstaller -Path $priorInstaller -Destination $installDirectory
        Wait-ForPathState -Path $installedExecutable -Exists $true
        $priorVersion = (Get-Item -LiteralPath $installedExecutable).VersionInfo.ProductVersion
        if ($priorVersion -ne $ExpectedPriorVersion) {
            throw "Prior installer produced version '$priorVersion' instead of '$ExpectedPriorVersion'."
        }
    }

    Invoke-SilentInstaller -Path $installer -Destination $installDirectory
    Wait-ForPathState -Path $installedExecutable -Exists $true

    $installedItem = Get-Item -LiteralPath $installedExecutable -Force
    if ($installedItem.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw 'Installed application executable is a reparse point.'
    }
    $installedVersion = $installedItem.VersionInfo.ProductVersion
    if ($installedVersion -ne $ExpectedVersion) {
        throw "Installed product version '$installedVersion' does not match '$ExpectedVersion'."
    }

    $foundationFileCount = Assert-ExactFoundationPayload -ExpectedRoot $foundation -ActualRoot $installedFoundation
    $installedHash = (Get-FileHash -LiteralPath $installedExecutable -Algorithm SHA256).Hash.ToLowerInvariant()
    $installedSignature = Get-AuthenticodeSignature -LiteralPath $installedExecutable
    if ($RequireValidSignature -and $installedSignature.Status -ne [Management.Automation.SignatureStatus]::Valid) {
        throw "Installed application signature is not valid: $($installedSignature.Status)."
    }
    if ($RequireValidSignature -and $null -eq $installedSignature.TimeStamperCertificate) {
        throw 'Installed application signature is valid but is not timestamped.'
    }
    if (
        $RequireValidSignature
        -and $null -ne $signature.SignerCertificate
        -and $null -ne $installedSignature.SignerCertificate
        -and $signature.SignerCertificate.Thumbprint -ne $installedSignature.SignerCertificate.Thumbprint
    ) {
        throw 'Installer and installed application signatures use different publishers.'
    }

    $launchSmokePassed = $false
    if (-not $SkipLaunchSmoke) {
        $launchedProcess = Start-Process -FilePath $installedExecutable -WindowStyle Hidden -PassThru
        Start-Sleep -Seconds 5
        if ($launchedProcess.HasExited) {
            throw "Installed application exited during launch smoke with code $($launchedProcess.ExitCode)."
        }
        Stop-Process -Id $launchedProcess.Id -Force
        $launchedProcess.WaitForExit()
        $launchSmokePassed = $true
    }

    if (-not (Test-Path -LiteralPath $uninstaller -PathType Leaf)) {
        throw 'Installer did not create its uninstaller.'
    }
    $uninstallProcess = Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -Wait -PassThru
    if ($uninstallProcess.ExitCode -ne 0) {
        throw "Uninstaller exited with code $($uninstallProcess.ExitCode)."
    }
    Wait-ForPathState -Path $installDirectory -Exists $false
    $lifecycleComplete = $true

    $evidence = [ordered]@{
        schemaVersion = 1
        generatedAtUtc = [DateTimeOffset]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ss.fffZ')
        artifact = [ordered]@{
            fileName = $installerItem.Name
            byteLength = $installerItem.Length
            sha256 = $installerHash
            expectedVersion = $ExpectedVersion
            authenticodeStatus = $signature.Status.ToString()
            signerThumbprint = if ($null -ne $signature.SignerCertificate) { $signature.SignerCertificate.Thumbprint } else { $null }
            timestamperThumbprint = if ($null -ne $signature.TimeStamperCertificate) { $signature.TimeStamperCertificate.Thumbprint } else { $null }
        }
        lifecycle = [ordered]@{
            freshInstall = $true
            upgradedFromVersion = $priorVersion
            installedVersion = $installedVersion
            installedExecutableSha256 = $installedHash
            installedExecutableAuthenticodeStatus = $installedSignature.Status.ToString()
            installedExecutableSignerThumbprint = if ($null -ne $installedSignature.SignerCertificate) { $installedSignature.SignerCertificate.Thumbprint } else { $null }
            installedExecutableTimestamperThumbprint = if ($null -ne $installedSignature.TimeStamperCertificate) { $installedSignature.TimeStamperCertificate.Thumbprint } else { $null }
            launchSmoke = $launchSmokePassed
            uninstall = $true
            residueFree = $true
        }
        bundledFoundation = [ordered]@{
            exactFileCount = $foundationFileCount
            missing = 0
            unexpected = 0
            hashMismatches = 0
        }
    }

    $evidenceDirectory = Split-Path -Parent $evidenceFile
    if (-not (Test-Path -LiteralPath $evidenceDirectory)) {
        New-Item -ItemType Directory -Path $evidenceDirectory | Out-Null
    }
    $temporaryEvidence = "$evidenceFile.tmp"
    [IO.File]::WriteAllText($temporaryEvidence, (($evidence | ConvertTo-Json -Depth 6) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $temporaryEvidence -Destination $evidenceFile -Force
    Write-Output ($evidence | ConvertTo-Json -Depth 6)
}
finally {
    if ($null -ne $launchedProcess -and -not $launchedProcess.HasExited) {
        Stop-Process -Id $launchedProcess.Id -Force -ErrorAction SilentlyContinue
    }
    if (-not $lifecycleComplete -and (Test-Path -LiteralPath $uninstaller -PathType Leaf)) {
        $cleanup = Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -Wait -PassThru
        if ($cleanup.ExitCode -ne 0) {
            Write-Warning "Qualification cleanup uninstaller exited with code $($cleanup.ExitCode)."
        }
    }
}
