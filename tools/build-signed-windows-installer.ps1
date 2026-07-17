[CmdletBinding()]
param(
    [string] $CertificateThumbprint = $env:SAPPHIRUS_SIGNING_CERT_THUMBPRINT,

    [string] $TimestampUrl = $env:SAPPHIRUS_TIMESTAMP_URL,

    [ValidateSet('rfc3161', 'authenticode')]
    [string] $TimestampProtocol = 'rfc3161',

    [string] $ConfigPath = 'crates/desktop-app/tauri.conf.json',

    [string] $ApplicationPath = 'target/release/sapphirus.exe',

    [string] $InstallerPath = 'target/release/bundle/nsis/Sapphirus_0.1.0_x64-setup.exe'
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
        -not [string]::IsNullOrWhiteSpace($uri.UserInfo)
        -or -not [string]::IsNullOrWhiteSpace($uri.Query)
        -or -not [string]::IsNullOrWhiteSpace($uri.Fragment)
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

$pnpm = Get-Command 'pnpm.cmd' -ErrorAction SilentlyContinue
if ($null -eq $pnpm) {
    $pnpm = Get-Command 'pnpm' -ErrorAction SilentlyContinue
}
if ($null -eq $pnpm) {
    throw 'The pinned pnpm executable is unavailable on PATH.'
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

    $application = Resolve-RegularFile -Path $ApplicationPath
    $installer = Resolve-RegularFile -Path $InstallerPath
    $applicationSignature = Assert-TimestampedPublisherSignature -Path $application -PublisherThumbprint $normalizedThumbprint
    $installerSignature = Assert-TimestampedPublisherSignature -Path $installer -PublisherThumbprint $normalizedThumbprint

    [ordered]@{
        schemaVersion = 1
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
    } | ConvertTo-Json -Depth 5
}
finally {
    if (Test-Path -LiteralPath $overlayPath -PathType Leaf) {
        Remove-Item -LiteralPath $overlayPath -Force
    }
}
