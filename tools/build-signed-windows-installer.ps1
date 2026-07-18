[CmdletBinding()]
param(
    [string] $CertificateThumbprint = $env:SAPPHIRUS_SIGNING_CERT_THUMBPRINT,

    [string] $TimestampUrl = $env:SAPPHIRUS_TIMESTAMP_URL,

    [ValidateSet('rfc3161', 'authenticode')]
    [string] $TimestampProtocol = 'rfc3161',

    [string] $ConfigPath = 'crates/desktop-app/tauri.conf.json',

    [string] $ApplicationPath = 'target/release/sapphirus.exe',

    [string] $InstallerPath,

    [string] $ReleaseMetadataScript = 'tools/resolve-release-metadata.mjs',

    [string] $ReleaseSbomScript = 'tools/generate-release-sbom.mjs',

    [string] $TauriCliScript = 'node_modules/@tauri-apps/cli/tauri.js',

    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string] $SbomPath,

    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string] $EvidencePath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Resolve-RegularFile {
    param([Parameter(Mandatory = $true)][string] $Path)

    $resolved = Resolve-Path -LiteralPath $Path
    $item = Get-Item -LiteralPath $resolved.Path -Force
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Expected a regular file: $Path"
    }
    return $item.FullName
}

function Resolve-NewEvidenceFile {
    param([Parameter(Mandatory = $true)][string] $Path)

    $absolutePath = if ([IO.Path]::IsPathRooted($Path)) {
        [IO.Path]::GetFullPath($Path)
    } else {
        [IO.Path]::GetFullPath((Join-Path (Get-Location).Path $Path))
    }
    $parentPath = Split-Path -Parent $absolutePath
    $parent = Get-Item -LiteralPath $parentPath -Force
    if (-not $parent.PSIsContainer -or ($parent.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Signed-build evidence parent must be an existing regular directory.'
    }
    if (Test-Path -LiteralPath $absolutePath) {
        throw 'Signed-build evidence path must not already exist.'
    }
    return $absolutePath
}

function Assert-ValidTimestampUrl {
    param([Parameter(Mandatory = $true)][string] $Url)

    $uri = $null
    if (-not [Uri]::TryCreate($Url, [UriKind]::Absolute, [ref] $uri)) {
        throw 'SAPPHIRUS_TIMESTAMP_URL must be an absolute URI.'
    }
    if ($uri.Scheme -ne 'https' -or [string]::IsNullOrWhiteSpace($uri.Host)) {
        throw 'SAPPHIRUS_TIMESTAMP_URL must use HTTPS and include a host.'
    }
    if (
        -not [string]::IsNullOrWhiteSpace($uri.UserInfo) -or
        -not [string]::IsNullOrWhiteSpace($uri.Query) -or
        -not [string]::IsNullOrWhiteSpace($uri.Fragment)
    ) {
        throw 'SAPPHIRUS_TIMESTAMP_URL must not contain credentials, a query, or a fragment.'
    }
    return $uri.AbsoluteUri
}

function Find-CodeSigningCertificate {
    param([Parameter(Mandatory = $true)][string] $Thumbprint)

    $matches = @()
    foreach ($storePath in @('Cert:\CurrentUser\My', 'Cert:\LocalMachine\My')) {
        if (Test-Path -LiteralPath $storePath) {
            $matches += @(Get-ChildItem -LiteralPath $storePath | Where-Object {
                $_.Thumbprint -eq $Thumbprint
            })
        }
    }

    if ($matches.Count -ne 1) {
        throw 'The configured signing certificate must resolve exactly once in the Windows certificate stores.'
    }

    $certificate = $matches[0]
    $now = [DateTime]::UtcNow
    if (-not $certificate.HasPrivateKey) {
        throw 'The configured signing certificate has no accessible private key.'
    }
    if ($certificate.NotBefore.ToUniversalTime() -gt $now -or $certificate.NotAfter.ToUniversalTime() -le $now) {
        throw 'The configured signing certificate is not currently valid.'
    }

    $codeSigningOid = '1.3.6.1.5.5.7.3.3'
    $hasCodeSigningEku = $false
    foreach ($extension in $certificate.Extensions) {
        if ($extension -is [Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]) {
            foreach ($oid in $extension.EnhancedKeyUsages) {
                if ($oid.Value -eq $codeSigningOid) {
                    $hasCodeSigningEku = $true
                }
            }
        }
    }
    if (-not $hasCodeSigningEku) {
        throw 'The configured certificate is not authorized for code signing.'
    }

    return $certificate
}

function Assert-TimestampedPublisherSignature {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)][string] $PublisherThumbprint
    )

    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne [Management.Automation.SignatureStatus]::Valid) {
        throw "Authenticode verification failed for the signed release artifact: $($signature.Status)."
    }
    if ($null -eq $signature.SignerCertificate -or $signature.SignerCertificate.Thumbprint -ne $PublisherThumbprint) {
        throw 'Signed release artifact publisher does not match the configured certificate.'
    }
    if ($null -eq $signature.TimeStamperCertificate) {
        throw 'Signed release artifact has no trusted timestamp.'
    }
    return $signature
}

if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'Signed Windows installer builds must run on Windows.'
}

$thumbprintInput = if ($null -eq $CertificateThumbprint) { '' } else { $CertificateThumbprint }
$timestampInput = if ($null -eq $TimestampUrl) { '' } else { $TimestampUrl }
$normalizedThumbprint = [regex]::Replace($thumbprintInput, '\s', '').ToUpperInvariant()
if ($normalizedThumbprint -notmatch '^[0-9A-F]{40}$') {
    throw 'SAPPHIRUS_SIGNING_CERT_THUMBPRINT must be exactly 40 hexadecimal SHA-1 characters.'
}
$validatedTimestampUrl = Assert-ValidTimestampUrl -Url $timestampInput
$certificate = Find-CodeSigningCertificate -Thumbprint $normalizedThumbprint
$configuration = Resolve-RegularFile -Path $ConfigPath
$metadataScript = Resolve-RegularFile -Path $ReleaseMetadataScript
$sbomScript = Resolve-RegularFile -Path $ReleaseSbomScript
$tauriCliScriptPath = Resolve-RegularFile -Path $TauriCliScript
$sbom = Resolve-RegularFile -Path $SbomPath

$node = Get-Command 'node.exe' -ErrorAction SilentlyContinue
if ($null -eq $node) {
    $node = Get-Command 'node' -ErrorAction SilentlyContinue
}
if ($null -eq $node) {
    throw 'The pinned Node executable is unavailable on PATH.'
}
$releaseMetadataOutput = @(& $node.Source $metadataScript)
if ($LASTEXITCODE -ne 0) {
    throw 'Release metadata resolution failed.'
}
$releaseMetadata = ($releaseMetadataOutput -join [Environment]::NewLine) | ConvertFrom-Json
$productVersion = [string] $releaseMetadata.product.version
$expectedInstallerName = [string] $releaseMetadata.product.installerName
$expectedApplicationName = [string] $releaseMetadata.product.applicationName
$expectedSbomName = [string] $releaseMetadata.product.sbomName

$configurationJson = Get-Content -LiteralPath $configuration -Raw | ConvertFrom-Json
if ([string] $configurationJson.version -ne $productVersion) {
    throw 'Tauri configuration disagrees with the release metadata authority.'
}
if ([string]::IsNullOrWhiteSpace($InstallerPath)) {
    $InstallerPath = Join-Path 'target/release/bundle/nsis' $expectedInstallerName
}
if ([IO.Path]::GetFileName($InstallerPath) -ne $expectedInstallerName) {
    throw 'Installer path disagrees with the release metadata authority.'
}
if ([IO.Path]::GetFileName($ApplicationPath) -ne $expectedApplicationName) {
    throw 'Application path disagrees with the release metadata authority.'
}
if ([IO.Path]::GetFileName($sbom) -ne $expectedSbomName) {
    throw 'SBOM path disagrees with the release metadata authority.'
}
$sbomJson = Get-Content -LiteralPath $sbom -Raw | ConvertFrom-Json
if (
    [string] $sbomJson.bomFormat -ne 'CycloneDX' -or
    [string] $sbomJson.specVersion -ne '1.6' -or
    [string] $sbomJson.metadata.component.version -ne $productVersion
) {
    throw 'SBOM identity disagrees with the release metadata authority.'
}
$evidenceFile = Resolve-NewEvidenceFile -Path $EvidencePath

$git = Get-Command 'git.exe' -ErrorAction SilentlyContinue
if ($null -eq $git) {
    $git = Get-Command 'git' -ErrorAction SilentlyContinue
}
if ($null -eq $git) {
    throw 'Git is unavailable on PATH.'
}
$sourceRevision = (& $git.Source rev-parse --verify HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $sourceRevision -notmatch '^[0-9a-fA-F]{40}$') {
    throw 'Unable to resolve the exact source revision for the signed build.'
}
$sourceChanges = @(& $git.Source status --porcelain --untracked-files=normal)
if ($LASTEXITCODE -ne 0) {
    throw 'Unable to verify the source worktree state for the signed build.'
}
if ($sourceChanges.Count -ne 0) {
    throw 'Signed release builds require a clean source worktree.'
}

$pnpm = Get-Command 'pnpm.cmd' -ErrorAction SilentlyContinue
if ($null -eq $pnpm) {
    $pnpm = Get-Command 'pnpm' -ErrorAction SilentlyContinue
}
if ($null -eq $pnpm) {
    throw 'The pinned pnpm executable is unavailable on PATH.'
}
$rustc = Get-Command 'rustc.exe' -ErrorAction SilentlyContinue
if ($null -eq $rustc) {
    $rustc = Get-Command 'rustc' -ErrorAction SilentlyContinue
}
if ($null -eq $rustc) {
    throw 'The pinned Rust compiler is unavailable on PATH.'
}

$actualNode = (& $node.Source --version).Trim().TrimStart('v')
$actualPnpm = (& $pnpm.Source --version).Trim()
$rustcDescription = (& $rustc.Source --version).Trim()
$rustcMatch = [regex]::Match($rustcDescription, '^rustc (\d+\.\d+\.\d+) ')
$tauriDescription = (& $node.Source $tauriCliScriptPath --version).Trim()
$tauriMatch = [regex]::Match($tauriDescription, '^tauri-cli (\d+\.\d+\.\d+)$')
if (
    $actualNode -ne [string] $releaseMetadata.toolchain.node -or
    $actualPnpm -ne [string] $releaseMetadata.toolchain.pnpm -or
    -not $rustcMatch.Success -or
    $rustcMatch.Groups[1].Value -ne [string] $releaseMetadata.toolchain.rust -or
    -not $tauriMatch.Success -or
    $tauriMatch.Groups[1].Value -ne [string] $releaseMetadata.toolchain.tauriCli
) {
    throw 'Runtime release toolchain disagrees with the reviewed metadata authority.'
}

$temporaryRoot = if (-not [string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
    $env:RUNNER_TEMP
} else {
    [IO.Path]::GetTempPath()
}
if (-not (Test-Path -LiteralPath $temporaryRoot -PathType Container)) {
    throw 'The signing overlay temporary root does not exist.'
}

$overlayPath = Join-Path $temporaryRoot "sapphirus-signing-$([Guid]::NewGuid().ToString('N')).json"
$overlay = [ordered]@{
    bundle = [ordered]@{
        windows = [ordered]@{
            certificateThumbprint = $normalizedThumbprint
            digestAlgorithm = 'sha256'
            timestampUrl = $validatedTimestampUrl
            tsp = $TimestampProtocol -eq 'rfc3161'
        }
    }
}

try {
    [IO.File]::WriteAllText(
        $overlayPath,
        (($overlay | ConvertTo-Json -Depth 5) + [Environment]::NewLine),
        [Text.UTF8Encoding]::new($false)
    )

    $arguments = @(
        'exec',
        'tauri',
        'build',
        '--config',
        $configuration,
        '--config',
        $overlayPath,
        '--features',
        'deterministic-help',
        '--ci'
    )
    & $pnpm.Source @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Signed Tauri build failed with exit code $LASTEXITCODE."
    }

    $postBuildRevision = (& $git.Source rev-parse --verify HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $postBuildRevision -ne $sourceRevision) {
        throw 'Source revision changed during the signed build.'
    }
    $postBuildChanges = @(& $git.Source status --porcelain --untracked-files=normal)
    if ($LASTEXITCODE -ne 0) {
        throw 'Unable to verify the post-build source worktree state.'
    }
    if ($postBuildChanges.Count -ne 0) {
        throw 'The signed build mutated the source worktree.'
    }
    $postBuildMetadataOutput = @(& $node.Source $metadataScript)
    if ($LASTEXITCODE -ne 0) {
        throw 'Post-build release metadata resolution failed.'
    }
    $postBuildMetadata = ($postBuildMetadataOutput -join [Environment]::NewLine) | ConvertFrom-Json
    $preBuildMetadataJson = $releaseMetadata | ConvertTo-Json -Depth 8 -Compress
    $postBuildMetadataJson = $postBuildMetadata | ConvertTo-Json -Depth 8 -Compress
    if ($postBuildMetadataJson -cne $preBuildMetadataJson) {
        throw 'Release metadata or lock identity changed during the signed build.'
    }
    & $node.Source $sbomScript --verify $sbom
    if ($LASTEXITCODE -ne 0) {
        throw 'SBOM verification against the post-build lock inventory failed.'
    }
    $sbomHash = (Get-FileHash -LiteralPath $sbom -Algorithm SHA256).Hash.ToLowerInvariant()

    $application = Resolve-RegularFile -Path $ApplicationPath
    $installer = Resolve-RegularFile -Path $InstallerPath
    $applicationSignature = Assert-TimestampedPublisherSignature -Path $application -PublisherThumbprint $normalizedThumbprint
    $installerSignature = Assert-TimestampedPublisherSignature -Path $installer -PublisherThumbprint $normalizedThumbprint

    $evidence = [ordered]@{
        schemaVersion = 2
        sourceRevision = $sourceRevision.ToLowerInvariant()
        sourceTreeState = 'clean'
        productVersion = $productVersion
        releaseMetadata = $releaseMetadata
        toolchain = [ordered]@{
            node = $actualNode
            pnpm = $actualPnpm
            rustc = $rustcDescription
            tauriCli = $tauriDescription
        }
        sbom = [ordered]@{
            fileName = $expectedSbomName
            sha256 = $sbomHash
            format = 'CycloneDX'
            specVersion = '1.6'
        }
        certificateThumbprint = $certificate.Thumbprint
        certificateNotAfterUtc = $certificate.NotAfter.ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ss.fffZ')
        timestampProtocol = $TimestampProtocol
        application = [ordered]@{
            sha256 = (Get-FileHash -LiteralPath $application -Algorithm SHA256).Hash.ToLowerInvariant()
            authenticodeStatus = $applicationSignature.Status.ToString()
            timestamperThumbprint = $applicationSignature.TimeStamperCertificate.Thumbprint
        }
        installer = [ordered]@{
            sha256 = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
            authenticodeStatus = $installerSignature.Status.ToString()
            timestamperThumbprint = $installerSignature.TimeStamperCertificate.Thumbprint
        }
    }
    $evidenceJson = $evidence | ConvertTo-Json -Depth 8
    [IO.File]::WriteAllText(
        $evidenceFile,
        ($evidenceJson + [Environment]::NewLine),
        [Text.UTF8Encoding]::new($false)
    )
    $evidenceJson
}
finally {
    if (Test-Path -LiteralPath $overlayPath -PathType Leaf) {
        Remove-Item -LiteralPath $overlayPath -Force
    }
}
