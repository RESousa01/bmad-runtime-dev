$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

. "$PSScriptRoot/publish-profiles.ps1"

$script:failures = [System.Collections.Generic.List[string]]::new()
$script:passes = 0

$script:repositoryRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $PSScriptRoot '..\..')
)
$script:examplePath = Join-Path `
    $script:repositoryRoot `
    'infra/desktop-support/model-profiles.example.json'
$script:readyDeploymentPath = Join-Path `
    $PSScriptRoot `
    'fixtures/ready-deployments.json'
$script:blockedDeploymentPath = Join-Path `
    $script:repositoryRoot `
    'artifacts/foundry/prerequisites.json'

function Assert-Equal {
    param(
        [Parameter(Mandatory)][string] $Name,
        [AllowNull()][object] $Actual,
        [AllowNull()][object] $Expected
    )

    if ($Actual -ne $Expected) {
        $script:failures.Add("$Name expected '$Expected' but received '$Actual'")
        return
    }
    $script:passes += 1
}

function Assert-True {
    param(
        [Parameter(Mandatory)][string] $Name,
        [Parameter(Mandatory)][bool] $Condition
    )

    if (-not $Condition) {
        $script:failures.Add("$Name expected true")
        return
    }
    $script:passes += 1
}

function Assert-ThrowsCode {
    param(
        [Parameter(Mandatory)][string] $Name,
        [Parameter(Mandatory)][scriptblock] $Action,
        [Parameter(Mandatory)][string] $Code
    )

    try {
        & $Action | Out-Null
        $script:failures.Add("$Name expected '$Code' but did not throw")
    }
    catch {
        $expectedMessage = "profile_publish:$Code"
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
        [Parameter(Mandatory)][string] $Name,
        [Parameter(Mandatory)][scriptblock] $Action
    )

    try {
        & $Action
    }
    catch {
        $script:failures.Add("$Name failed with '$($_.Exception.Message)'")
    }
}

<#
    An in-memory App Configuration double.

    It enforces exactly the precondition semantics the publisher relies on:
    If-None-Match: * fails when the key already exists, and If-Match fails when
    the stored ETag differs. ConflictKeys simulates an operator who changed a
    key after this run planned its writes.
#>
function New-FakeProfileStore {
    param(
        [hashtable] $Seed = @{},
        [string[]] $ConflictKeys = @()
    )

    $state = @{}
    foreach ($entry in $Seed.GetEnumerator()) {
        $state[[string] $entry.Key] = [pscustomobject]@{
            value = [string] $entry.Value
            etag = "etag-seed-$([string] $entry.Key)"
        }
    }
    $writes = [System.Collections.Generic.List[object]]::new()
    $conflicts = [System.Collections.Generic.HashSet[string]]::new(
        [string[]] $ConflictKeys,
        [System.StringComparer]::Ordinal
    )
    $counter = @{ next = 1 }

    $invoker = {
        param([hashtable] $Request)

        $operation = [string] $Request['operation']
        $label = [string] $Request['label']

        if ($operation -ceq 'list') {
            $prefix = [string] $Request['keyFilter']
            $prefix = $prefix.Substring(0, $prefix.Length - 1)
            $matched = [System.Collections.Generic.List[object]]::new()
            foreach ($entry in $state.GetEnumerator()) {
                $parts = ([string] $entry.Key).Split('|', 2)
                if ($parts[1] -cne $label) { continue }
                if (-not $parts[0].StartsWith($prefix, [System.StringComparison]::Ordinal)) {
                    continue
                }
                $matched.Add(
                    [pscustomobject]@{
                        key = $parts[0]
                        label = $parts[1]
                        value = $entry.Value.value
                        etag = $entry.Value.etag
                    }
                )
            }
            return @($matched)
        }

        $key = [string] $Request['key']
        $composite = "$key|$label"

        if ($operation -ceq 'get') {
            if (-not $state.ContainsKey($composite)) {
                return $null
            }
            return [pscustomobject]@{
                key = $key
                label = $label
                value = $state[$composite].value
                etag = $state[$composite].etag
            }
        }

        $exists = $state.ContainsKey($composite)
        if ($conflicts.Contains($composite)) {
            throw 'profile_publish:concurrent_update'
        }
        if ($Request.ContainsKey('ifNoneMatch') -and $exists) {
            throw 'profile_publish:concurrent_update'
        }
        if ($Request.ContainsKey('ifMatch')) {
            if (-not $exists -or $state[$composite].etag -cne [string] $Request['ifMatch']) {
                throw 'profile_publish:concurrent_update'
            }
        }
        $etag = "etag-write-$($counter.next)"
        $counter.next += 1
        $state[$composite] = [pscustomobject]@{
            value = [string] $Request['value']
            etag = $etag
        }
        $writes.Add(
            [pscustomobject]@{
                key = $key
                label = $label
                value = [string] $Request['value']
            }
        )
        return [pscustomobject]@{
            key = $key
            label = $label
            value = [string] $Request['value']
            etag = $etag
        }
    }.GetNewClosure()

    return [pscustomobject]@{
        invoker = $invoker
        state = $state
        writes = $writes
    }
}

function New-TemporaryReportPath {
    return (
        Join-Path `
            ([System.IO.Path]::GetTempPath()) `
            "forge-profile-publish-$([guid]::NewGuid().ToString('N')).json"
    )
}

function Read-ReportFile {
    param([Parameter(Mandatory)][string] $Path)
    return (Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json -Depth 32)
}

function New-SeededProfileValue {
    param([Parameter(Mandatory)][string] $ProfileId)
    return '{"marker":"seeded-must-not-leak","profileId":"' + $ProfileId + '"}'
}

# --- Store operation allowlist -------------------------------------------

Invoke-TestCase -Name 'store operations are closed' -Action {
    Assert-True -Name 'list is permitted' -Condition (
        Assert-ProfileStoreOperation -Request @{
            operation = 'list'
            keyFilter = 'desktop-support:model-profile:*'
            label = 'production'
        }
    )

    Assert-ThrowsCode -Name 'delete is not reachable' `
        -Code 'store_operation_forbidden' `
        -Action {
            Assert-ProfileStoreOperation -Request @{
                operation = 'delete'
                key = 'desktop-support:model-profile:fast-response'
                label = 'production'
            }
        }

    Assert-ThrowsCode -Name 'a foreign key is rejected' `
        -Code 'store_key_forbidden' `
        -Action {
            Assert-ProfileStoreOperation -Request @{
                operation = 'get'
                key = 'desktop-support:policy:policyVersion'
                label = 'production'
            }
        }

    Assert-ThrowsCode -Name 'a wildcard outside the prefix is rejected' `
        -Code 'store_key_forbidden' `
        -Action {
            Assert-ProfileStoreOperation -Request @{
                operation = 'list'
                keyFilter = '*'
                label = 'production'
            }
        }

    Assert-ThrowsCode -Name 'an unconditional write is rejected' `
        -Code 'store_write_unconditional' `
        -Action {
            Assert-ProfileStoreOperation -Request @{
                operation = 'set'
                key = 'desktop-support:model-profile:fast-response'
                label = 'production'
                value = '{}'
            }
        }

    Assert-ThrowsCode -Name 'two preconditions are rejected' `
        -Code 'store_write_unconditional' `
        -Action {
            Assert-ProfileStoreOperation -Request @{
                operation = 'set'
                key = 'desktop-support:model-profile:fast-response'
                label = 'production'
                value = '{}'
                ifMatch = 'etag-1'
                ifNoneMatch = '*'
            }
        }
}

Invoke-TestCase -Name 'the tool cannot deploy a model or grant a role' -Action {
    Assert-ThrowsCode -Name 'model deployment creation is refused before spawning az' `
        -Code 'azure_command_forbidden' `
        -Action {
            Invoke-ProfileAzCli -Arguments @(
                'cognitiveservices',
                'account',
                'deployment',
                'create'
            )
        }

    Assert-ThrowsCode -Name 'role assignment creation is refused before spawning az' `
        -Code 'azure_command_forbidden' `
        -Action {
            Invoke-ProfileAzCli -Arguments @('role', 'assignment', 'create')
        }

    Assert-ThrowsCode -Name 'App Configuration writes never go through the CLI' `
        -Code 'azure_command_forbidden' `
        -Action {
            Invoke-ProfileAzCli -Arguments @('appconfig', 'kv', 'set')
        }

    $source = Get-Content -Raw -LiteralPath (
        Join-Path $PSScriptRoot 'publish-profiles.ps1'
    )
    Assert-True -Name 'no ARM write verb appears in the publisher source' `
        -Condition (
            -not $source.Contains('deployment create') -and
            -not $source.Contains('assignment create') -and
            -not $source.Contains('Microsoft.Authorization') -and
            -not $source.Contains('management.azure.com')
        )
}

Invoke-TestCase -Name 'store endpoints are validated' -Action {
    $endpoint = Resolve-ProfileStoreEndpoint -Value 'https://Example-Store.azconfig.io/'
    Assert-Equal -Name 'endpoint is normalized' `
        -Actual $endpoint.AbsoluteUri `
        -Expected 'https://example-store.azconfig.io/'

    $invalid = [ordered]@{
        'http is rejected' = @(
            'http://example-store.azconfig.io/',
            'store_endpoint_scheme_invalid'
        )
        'a lookalike host is rejected' = @(
            'https://example-store.azconfig.io.evil.example/',
            'store_endpoint_host_invalid'
        )
        'a path is rejected' = @(
            'https://example-store.azconfig.io/kv',
            'store_endpoint_path_invalid'
        )
        'a query is rejected' = @(
            'https://example-store.azconfig.io/?a=1',
            'store_endpoint_invalid'
        )
        'a nondefault port is rejected' = @(
            'https://example-store.azconfig.io:444/',
            'store_endpoint_port_invalid'
        )
    }
    foreach ($case in $invalid.GetEnumerator()) {
        $value = $case.Value[0]
        Assert-ThrowsCode -Name $case.Key -Code $case.Value[1] -Action {
            Resolve-ProfileStoreEndpoint -Value $value
        }
    }
}

# --- Registry record serialization ----------------------------------------

Invoke-TestCase -Name 'the PowerShell registry record matches the validator' -Action {
    $planPath = New-TemporaryReportPath
    try {
        $result = Invoke-ProfileValidator `
            -ProfilePath $script:examplePath `
            -DeploymentPath $script:readyDeploymentPath `
            -CurrentStatePath '' `
            -PlanPath $planPath
        Assert-Equal -Name 'the example validates against the ready fixture' `
            -Actual $result.exitCode `
            -Expected 0

        $plan = Get-Content -Raw -LiteralPath $planPath |
            ConvertFrom-Json -Depth 32
        $pairs = @(
            $plan.profiles |
                ForEach-Object {
                    [pscustomobject]@{
                        profileId = $_.profileId
                        profileHash = $_.valueHash
                    }
                }
        )
        $record = New-ProfileRegistryRecord `
            -RegistryVersion ([int] $plan.registryVersion) `
            -CapabilityHash ([string] $plan.capabilityHash) `
            -ProfileHashPairs $pairs
        Assert-Equal -Name 'canonical registry value agrees byte for byte' `
            -Actual $record.value `
            -Expected ([string] $plan.registryValue)
        Assert-Equal -Name 'registry hash agrees across languages' `
            -Actual $record.registryHash `
            -Expected ([string] $plan.registryHash)

        $profileValueHash = Get-ProfileValueHash `
            -CanonicalJson ([string] $plan.profiles[0].value) `
            -Purpose $script:ProfileHashPurpose
        Assert-Equal -Name 'profile value hash agrees across languages' `
            -Actual $profileValueHash `
            -Expected ([string] $plan.profiles[0].valueHash)

        $reparsed = Read-ProfileRegistryRecord -Value $record.value
        Assert-Equal -Name 'the registry record round-trips' `
            -Actual $reparsed.registryVersion `
            -Expected ([int] $plan.registryVersion)
    }
    finally {
        if (Test-Path -LiteralPath $planPath -PathType Leaf) {
            Remove-Item -LiteralPath $planPath -Force
        }
    }
}

Invoke-TestCase -Name 'registry records reject unrepresentable input' -Action {
    Assert-ThrowsCode -Name 'a quoted profile id cannot be serialized' `
        -Code 'registry_pair_invalid' `
        -Action {
            New-ProfileRegistryRecord `
                -RegistryVersion 1 `
                -CapabilityHash ('sha256:' + ('a' * 64)) `
                -ProfileHashPairs @(
                    [pscustomobject]@{
                        profileId = 'bad"id'
                        profileHash = ('sha256:' + ('b' * 64))
                    }
                )
        }

    Assert-ThrowsCode -Name 'a malformed stored record is refused' `
        -Code 'registry_record_invalid' `
        -Action {
            Read-ProfileRegistryRecord -Value '{"schemaVersion":"other.v1"}'
        }
}

# --- Dry run ---------------------------------------------------------------

Invoke-TestCase -Name 'a dry run writes nothing' -Action {
    $store = New-FakeProfileStore
    $reportPath = New-TemporaryReportPath
    try {
        $exitCode = Invoke-ProfilePublish `
            -ProfilePath $script:examplePath `
            -DeploymentPath $script:readyDeploymentPath `
            -ReportPath $reportPath `
            -Label 'production' `
            -StoreEndpoint '' `
            -EnvironmentApproval 'approved' `
            -StoreInvoker $store.invoker `
            -WhatIf
        $report = Read-ReportFile -Path $reportPath

        Assert-Equal -Name (
            'dry run exit code; blockers=' + (@($report.blockers) -join ',')
        ) -Actual $exitCode -Expected 0
        Assert-Equal -Name 'dry run is reported as a dry run' `
            -Actual $report.mode `
            -Expected 'dry_run'
        Assert-Equal -Name 'dry run performed no store writes' `
            -Actual $store.writes.Count `
            -Expected 0
        Assert-Equal -Name 'dry run left the store empty' `
            -Actual $store.state.Count `
            -Expected 0
        Assert-Equal -Name 'dry run applied no operations' `
            -Actual @($report.appliedOperations).Count `
            -Expected 0
        Assert-Equal -Name 'dry run planned every profile plus two registry writes' `
            -Actual @($report.plannedOperations).Count `
            -Expected 10
        Assert-True -Name 'dry run validated the document' `
            -Condition $report.validation.valid
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

Invoke-TestCase -Name 'a refused document stops the dry run' -Action {
    $store = New-FakeProfileStore
    $reportPath = New-TemporaryReportPath
    try {
        $exitCode = Invoke-ProfilePublish `
            -ProfilePath $script:examplePath `
            -DeploymentPath $script:blockedDeploymentPath `
            -ReportPath $reportPath `
            -Label 'production' `
            -StoreEndpoint '' `
            -StoreInvoker $store.invoker `
            -WhatIf
        $report = Read-ReportFile -Path $reportPath

        Assert-Equal -Name 'a refused document exits 2' -Actual $exitCode -Expected 2
        Assert-True -Name 'refusal is attributed to the document' `
            -Condition (@($report.blockers) -contains 'document_refused')
        Assert-True -Name 'the unready inventory is named' `
            -Condition (
                @($report.validation.violations | ForEach-Object code) -contains
                    'deployment_inventory_not_ready'
            )
        Assert-Equal -Name 'a refused document writes nothing' `
            -Actual $store.writes.Count `
            -Expected 0
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

# --- Approval --------------------------------------------------------------

Invoke-TestCase -Name 'a publish without approval writes nothing' -Action {
    $store = New-FakeProfileStore
    $reportPath = New-TemporaryReportPath
    try {
        $exitCode = Invoke-ProfilePublish `
            -ProfilePath $script:examplePath `
            -DeploymentPath $script:readyDeploymentPath `
            -ReportPath $reportPath `
            -Label 'production' `
            -StoreEndpoint '' `
            -EnvironmentApproval '' `
            -StoreInvoker $store.invoker
        $report = Read-ReportFile -Path $reportPath

        Assert-Equal -Name 'an unapproved publish exits 2' -Actual $exitCode -Expected 2
        Assert-True -Name 'the missing approval is explicit' `
            -Condition (@($report.blockers) -contains 'publish_approval_missing')
        Assert-Equal -Name 'an unapproved publish writes nothing' `
            -Actual $store.writes.Count `
            -Expected 0
        Assert-Equal -Name 'an unapproved publish is not approved' `
            -Actual $report.approved `
            -Expected $false
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

Invoke-TestCase -Name 'approval is granted by a switch or by CI, never by default' -Action {
    Assert-Equal -Name 'the operator switch approves' `
        -Actual (
            Get-ProfilePublishApproval -Approve $true -EnvironmentApproval ''
        ).source `
        -Expected 'operator_switch'
    Assert-Equal -Name 'the exact CI value approves' `
        -Actual (
            Get-ProfilePublishApproval -Approve $false -EnvironmentApproval 'approved'
        ).source `
        -Expected 'ci_environment'
    Assert-Equal -Name 'a near-miss CI value does not approve' `
        -Actual (
            Get-ProfilePublishApproval -Approve $false -EnvironmentApproval 'Approved'
        ).approved `
        -Expected $false
    Assert-Equal -Name 'a truthy CI value does not approve' `
        -Actual (
            Get-ProfilePublishApproval -Approve $false -EnvironmentApproval 'true'
        ).approved `
        -Expected $false
    Assert-Equal -Name 'nothing approves by default' `
        -Actual (
            Get-ProfilePublishApproval -Approve $false -EnvironmentApproval ''
        ).approved `
        -Expected $false
}

Invoke-TestCase -Name 'the -Approve switch publishes without a CI variable' -Action {
    $store = New-FakeProfileStore
    $reportPath = New-TemporaryReportPath
    try {
        $exitCode = Invoke-ProfilePublish `
            -ProfilePath $script:examplePath `
            -DeploymentPath $script:readyDeploymentPath `
            -ReportPath $reportPath `
            -Label 'production' `
            -StoreEndpoint '' `
            -Approve $true `
            -EnvironmentApproval '' `
            -StoreInvoker $store.invoker
        $report = Read-ReportFile -Path $reportPath

        Assert-Equal -Name (
            'switch-approved publish exit code; blockers=' +
            (@($report.blockers) -join ',')
        ) -Actual $exitCode -Expected 0
        Assert-Equal -Name 'the switch is recorded as the approval source' `
            -Actual $report.approvalSource `
            -Expected 'operator_switch'
        Assert-Equal -Name 'the switch-approved publish wrote the whole set' `
            -Actual $store.writes.Count `
            -Expected 10
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

# --- Publish ---------------------------------------------------------------

Invoke-TestCase -Name 'an approved publish writes the registry key last' -Action {
    $store = New-FakeProfileStore
    $reportPath = New-TemporaryReportPath
    try {
        $exitCode = Invoke-ProfilePublish `
            -ProfilePath $script:examplePath `
            -DeploymentPath $script:readyDeploymentPath `
            -ReportPath $reportPath `
            -Label 'production' `
            -StoreEndpoint '' `
            -EnvironmentApproval 'approved' `
            -StoreInvoker $store.invoker
        $report = Read-ReportFile -Path $reportPath

        Assert-Equal -Name (
            'approved publish exit code; blockers=' + (@($report.blockers) -join ',')
        ) -Actual $exitCode -Expected 0
        Assert-Equal -Name 'the CI variable is recorded as the approval source' `
            -Actual $report.approvalSource `
            -Expected 'ci_environment'

        $applied = @($report.appliedOperations)
        Assert-Equal -Name 'every planned operation was applied' `
            -Actual $applied.Count `
            -Expected @($report.plannedOperations).Count
        Assert-Equal -Name 'the last write is the active registry key' `
            -Actual $applied[$applied.Count - 1].kind `
            -Expected 'set_registry'
        Assert-Equal -Name 'the last write targets the active label' `
            -Actual $applied[$applied.Count - 1].label `
            -Expected 'production'

        $activeRegistryIndex = -1
        $lastProfileIndex = -1
        for ($index = 0; $index -lt $applied.Count; $index += 1) {
            if ($applied[$index].kind -ceq 'set_registry') {
                $activeRegistryIndex = $index
            }
            if ($applied[$index].kind -ceq 'set_profile') {
                $lastProfileIndex = $index
            }
        }
        Assert-True -Name 'no profile is written after the registry key' `
            -Condition ($lastProfileIndex -lt $activeRegistryIndex)

        Assert-True -Name 'the archive label was written for rollback' `
            -Condition (
                @($applied | Where-Object { $_.label -ceq 'production-v1' }).Count -eq 5
            )
        Assert-True -Name 'the published registry record is readable' `
            -Condition (
                (
                    Read-ProfileRegistryRecord -Value (
                        $store.state['desktop-support:model-profile-registry:version|production'].value
                    )
                ).registryVersion -eq 1
            )
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

# --- Registry downgrade ----------------------------------------------------

Invoke-TestCase -Name 'a registry downgrade is refused' -Action {
    $seededRecord = New-ProfileRegistryRecord `
        -RegistryVersion 5 `
        -CapabilityHash ('sha256:' + ('a' * 64)) `
        -ProfileHashPairs @(
            [pscustomobject]@{
                profileId = 'fast-response'
                profileHash = ('sha256:' + ('b' * 64))
            }
        )
    $store = New-FakeProfileStore -Seed @{
        'desktop-support:model-profile-registry:version|production' = $seededRecord.value
        'desktop-support:model-profile:fast-response|production' = (
            New-SeededProfileValue -ProfileId 'fast-response'
        )
    }
    $reportPath = New-TemporaryReportPath
    try {
        $exitCode = Invoke-ProfilePublish `
            -ProfilePath $script:examplePath `
            -DeploymentPath $script:readyDeploymentPath `
            -ReportPath $reportPath `
            -Label 'production' `
            -StoreEndpoint '' `
            -EnvironmentApproval 'approved' `
            -StoreInvoker $store.invoker
        $report = Read-ReportFile -Path $reportPath

        Assert-Equal -Name 'a downgrade exits 2' -Actual $exitCode -Expected 2
        Assert-True -Name 'the downgrade is caught while validating' `
            -Condition (
                @($report.validation.violations | ForEach-Object code) -contains
                    'registry_version_not_increasing'
            )
        Assert-True -Name 'the refusal reaches the publish report' `
            -Condition (@($report.blockers) -contains 'document_refused')
        Assert-Equal -Name 'a downgrade writes nothing' `
            -Actual $store.writes.Count `
            -Expected 0
        Assert-Equal -Name 'the published registry version is unchanged' `
            -Actual $report.currentRegistryVersion `
            -Expected 5
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

Invoke-TestCase -Name 'the publisher rechecks the registry version itself' -Action {
    # Defence in depth: even if a validator misses the downgrade, the publisher
    # compares the planned version against what the store currently serves.
    $seededRecord = New-ProfileRegistryRecord `
        -RegistryVersion 9 `
        -CapabilityHash ('sha256:' + ('a' * 64)) `
        -ProfileHashPairs @(
            [pscustomobject]@{
                profileId = 'fast-response'
                profileHash = ('sha256:' + ('b' * 64))
            }
        )
    $store = New-FakeProfileStore -Seed @{
        'desktop-support:model-profile-registry:version|production' = $seededRecord.value
    }
    $reportPath = New-TemporaryReportPath
    $plannedValue = New-SeededProfileValue -ProfileId 'fast-response'
    try {
        $exitCode = Invoke-ProfilePublish `
            -ProfilePath $script:examplePath `
            -DeploymentPath $script:readyDeploymentPath `
            -ReportPath $reportPath `
            -Label 'production' `
            -StoreEndpoint '' `
            -EnvironmentApproval 'approved' `
            -StoreInvoker $store.invoker `
            -ValidatorInvoker {
                param(
                    [string] $Profiles,
                    [string] $Deployments,
                    [string] $Current,
                    [string] $PlanOutput
                )
                $planValue = New-SeededProfileValue -ProfileId 'fast-response'
                $hash = Get-ProfileValueHash `
                    -CanonicalJson $planValue `
                    -Purpose $script:ProfileHashPurpose
                $record = New-ProfileRegistryRecord `
                    -RegistryVersion 2 `
                    -CapabilityHash ('sha256:' + ('e' * 64)) `
                    -ProfileHashPairs @(
                        [pscustomobject]@{
                            profileId = 'fast-response'
                            profileHash = $hash
                        }
                    )
                $plan = [pscustomobject]@{
                    schemaVersion = 'forge.model-profile-plan.v1'
                    registryVersion = 2
                    registryHash = $record.registryHash
                    capabilityHash = $record.capabilityHash
                    registryValue = $record.value
                    profiles = @(
                        [pscustomobject]@{
                            profileId = 'fast-response'
                            key = 'desktop-support:model-profile:fast-response'
                            value = $planValue
                            valueHash = $hash
                        }
                    )
                }
                [System.IO.File]::WriteAllText(
                    $PlanOutput,
                    ($plan | ConvertTo-Json -Depth 8),
                    [System.Text.UTF8Encoding]::new($false)
                )
                return [pscustomobject]@{ exitCode = 0; report = $null }
            }
        $report = Read-ReportFile -Path $reportPath

        Assert-Equal -Name 'the publisher-side downgrade check exits 2' `
            -Actual $exitCode `
            -Expected 2
        Assert-True -Name 'the publisher-side downgrade has a stable blocker' `
            -Condition (
                @($report.blockers) -contains 'registry_version_not_increasing'
            )
        Assert-Equal -Name 'the publisher-side downgrade writes nothing' `
            -Actual $store.writes.Count `
            -Expected 0
        Assert-True -Name 'the injected plan value never reached the store' `
            -Condition (
                -not $store.state.ContainsKey(
                    'desktop-support:model-profile:fast-response|production'
                ) -and
                $plannedValue.Length -gt 0
            )
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

# --- Concurrent update -----------------------------------------------------

Invoke-TestCase -Name 'a concurrent operator change is never overwritten' -Action {
    $store = New-FakeProfileStore -ConflictKeys @(
        'desktop-support:model-profile:deep-reasoning|production'
    )
    $reportPath = New-TemporaryReportPath
    try {
        $exitCode = Invoke-ProfilePublish `
            -ProfilePath $script:examplePath `
            -DeploymentPath $script:readyDeploymentPath `
            -ReportPath $reportPath `
            -Label 'production' `
            -StoreEndpoint '' `
            -EnvironmentApproval 'approved' `
            -StoreInvoker $store.invoker
        $report = Read-ReportFile -Path $reportPath

        Assert-Equal -Name 'a conflicting publish exits 2' -Actual $exitCode -Expected 2
        Assert-True -Name 'the conflict has a stable blocker' `
            -Condition (@($report.blockers) -contains 'concurrent_update')
        Assert-True -Name 'the active registry key was never written' `
            -Condition (
                -not $store.state.ContainsKey(
                    'desktop-support:model-profile-registry:version|production'
                )
            )
        Assert-True -Name 'the conflicting key was never written' `
            -Condition (
                -not $store.state.ContainsKey(
                    'desktop-support:model-profile:deep-reasoning|production'
                )
            )
        Assert-True -Name 'the set stopped at the conflict' `
            -Condition (
                @($report.appliedOperations).Count -lt
                    @($report.plannedOperations).Count
            )
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

Invoke-TestCase -Name 'a stale ETag fails the write' -Action {
    $store = New-FakeProfileStore -Seed @{
        'desktop-support:model-profile:fast-response|production' = (
            New-SeededProfileValue -ProfileId 'fast-response'
        )
    }
    Assert-ThrowsCode -Name 'If-Match against a stale ETag is refused' `
        -Code 'concurrent_update' `
        -Action {
            & $store.invoker @{
                operation = 'set'
                key = 'desktop-support:model-profile:fast-response'
                label = 'production'
                value = '{}'
                ifMatch = 'etag-that-is-not-current'
            }
        }
    Assert-ThrowsCode -Name 'If-None-Match against an existing key is refused' `
        -Code 'concurrent_update' `
        -Action {
            & $store.invoker @{
                operation = 'set'
                key = 'desktop-support:model-profile:fast-response'
                label = 'production'
                value = '{}'
                ifNoneMatch = '*'
            }
        }
}

# --- Rollback --------------------------------------------------------------

Invoke-TestCase -Name 'rollback restores the prior labelled version' -Action {
    $priorValue = New-SeededProfileValue -ProfileId 'fast-response'
    $priorHash = Get-ProfileValueHash `
        -CanonicalJson $priorValue `
        -Purpose $script:ProfileHashPurpose
    $priorRecord = New-ProfileRegistryRecord `
        -RegistryVersion 1 `
        -CapabilityHash ('sha256:' + ('c' * 64)) `
        -ProfileHashPairs @(
            [pscustomobject]@{ profileId = 'fast-response'; profileHash = $priorHash }
        )

    $currentValue = '{"marker":"current-must-not-leak","profileId":"fast-response"}'
    $currentHash = Get-ProfileValueHash `
        -CanonicalJson $currentValue `
        -Purpose $script:ProfileHashPurpose
    $currentRecord = New-ProfileRegistryRecord `
        -RegistryVersion 2 `
        -CapabilityHash ('sha256:' + ('d' * 64)) `
        -ProfileHashPairs @(
            [pscustomobject]@{ profileId = 'fast-response'; profileHash = $currentHash }
        )

    $store = New-FakeProfileStore -Seed @{
        'desktop-support:model-profile:fast-response|production-v1' = $priorValue
        'desktop-support:model-profile-registry:version|production-v1' = $priorRecord.value
        'desktop-support:model-profile:fast-response|production' = $currentValue
        'desktop-support:model-profile-registry:version|production' = $currentRecord.value
    }
    $reportPath = New-TemporaryReportPath
    try {
        $exitCode = Invoke-ProfilePublish `
            -ProfilePath $script:examplePath `
            -DeploymentPath $script:readyDeploymentPath `
            -ReportPath $reportPath `
            -Label 'production' `
            -StoreEndpoint '' `
            -RollbackToRegistryVersion 1 `
            -EnvironmentApproval 'approved' `
            -StoreInvoker $store.invoker
        $report = Read-ReportFile -Path $reportPath

        Assert-Equal -Name (
            'rollback exit code; blockers=' + (@($report.blockers) -join ',')
        ) -Actual $exitCode -Expected 0
        Assert-Equal -Name 'rollback is reported as a rollback' `
            -Actual $report.mode `
            -Expected 'rollback'
        Assert-Equal -Name 'rollback restores the prior value' `
            -Actual $store.state['desktop-support:model-profile:fast-response|production'].value `
            -Expected $priorValue
        Assert-Equal -Name 'rollback moves the registry version forward, never back' `
            -Actual $report.registryVersion `
            -Expected 3
        Assert-Equal -Name 'rollback keeps the restored capability surface' `
            -Actual $report.capabilityHash `
            -Expected ('sha256:' + ('c' * 64))

        $applied = @($report.appliedOperations)
        Assert-Equal -Name 'rollback writes the registry key last' `
            -Actual $applied[$applied.Count - 1].kind `
            -Expected 'set_registry'
        Assert-Equal -Name 'the rolled-back registry record is version 3' `
            -Actual (
                Read-ProfileRegistryRecord -Value (
                    $store.state['desktop-support:model-profile-registry:version|production'].value
                )
            ).registryVersion `
            -Expected 3
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

Invoke-TestCase -Name 'rollback refuses a target that is not prior' -Action {
    $value = New-SeededProfileValue -ProfileId 'fast-response'
    $hash = Get-ProfileValueHash `
        -CanonicalJson $value `
        -Purpose $script:ProfileHashPurpose
    $record = New-ProfileRegistryRecord `
        -RegistryVersion 2 `
        -CapabilityHash ('sha256:' + ('c' * 64)) `
        -ProfileHashPairs @(
            [pscustomobject]@{ profileId = 'fast-response'; profileHash = $hash }
        )
    $store = New-FakeProfileStore -Seed @{
        'desktop-support:model-profile:fast-response|production-v2' = $value
        'desktop-support:model-profile-registry:version|production-v2' = $record.value
        'desktop-support:model-profile:fast-response|production' = $value
        'desktop-support:model-profile-registry:version|production' = $record.value
    }
    $reportPath = New-TemporaryReportPath
    try {
        $exitCode = Invoke-ProfilePublish `
            -ProfilePath $script:examplePath `
            -DeploymentPath $script:readyDeploymentPath `
            -ReportPath $reportPath `
            -Label 'production' `
            -StoreEndpoint '' `
            -RollbackToRegistryVersion 2 `
            -EnvironmentApproval 'approved' `
            -StoreInvoker $store.invoker
        $report = Read-ReportFile -Path $reportPath

        Assert-Equal -Name 'rolling back to the current version exits 2' `
            -Actual $exitCode `
            -Expected 2
        Assert-True -Name 'the refusal is explicit' `
            -Condition (@($report.blockers) -contains 'rollback_target_not_prior')
        Assert-Equal -Name 'a refused rollback writes nothing' `
            -Actual $store.writes.Count `
            -Expected 0
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

Invoke-TestCase -Name 'rollback refuses a missing archive' -Action {
    $value = New-SeededProfileValue -ProfileId 'fast-response'
    $hash = Get-ProfileValueHash `
        -CanonicalJson $value `
        -Purpose $script:ProfileHashPurpose
    $record = New-ProfileRegistryRecord `
        -RegistryVersion 4 `
        -CapabilityHash ('sha256:' + ('c' * 64)) `
        -ProfileHashPairs @(
            [pscustomobject]@{ profileId = 'fast-response'; profileHash = $hash }
        )
    $store = New-FakeProfileStore -Seed @{
        'desktop-support:model-profile:fast-response|production' = $value
        'desktop-support:model-profile-registry:version|production' = $record.value
    }
    $reportPath = New-TemporaryReportPath
    try {
        $exitCode = Invoke-ProfilePublish `
            -ProfilePath $script:examplePath `
            -DeploymentPath $script:readyDeploymentPath `
            -ReportPath $reportPath `
            -Label 'production' `
            -StoreEndpoint '' `
            -RollbackToRegistryVersion 3 `
            -EnvironmentApproval 'approved' `
            -StoreInvoker $store.invoker
        $report = Read-ReportFile -Path $reportPath

        Assert-Equal -Name 'a missing archive exits 2' -Actual $exitCode -Expected 2
        Assert-True -Name 'the missing archive is explicit' `
            -Condition (@($report.blockers) -contains 'rollback_target_missing')
        Assert-Equal -Name 'a missing archive writes nothing' `
            -Actual $store.writes.Count `
            -Expected 0
    }
    finally {
        if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
            Remove-Item -LiteralPath $reportPath -Force
        }
    }
}

# --- Redaction -------------------------------------------------------------

Invoke-TestCase -Name 'the report carries no values, ETags, or endpoints' -Action {
    $store = New-FakeProfileStore -Seed @{
        'desktop-support:model-profile:fast-response|production' = (
            New-SeededProfileValue -ProfileId 'fast-response'
        )
    }
    $reportPath = New-TemporaryReportPath
    try {
        $exitCode = Invoke-ProfilePublish `
            -ProfilePath $script:examplePath `
            -DeploymentPath $script:readyDeploymentPath `
            -ReportPath $reportPath `
            -Label 'production' `
            -StoreEndpoint 'https://example-store.azconfig.io/' `
            -EnvironmentApproval 'approved' `
            -StoreInvoker $store.invoker
        $report = Read-ReportFile -Path $reportPath
        $reportJson = Get-Content -Raw -LiteralPath $reportPath

        Assert-Equal -Name (
            'redaction publish exit code; blockers=' + (@($report.blockers) -join ',')
        ) -Actual $exitCode -Expected 0
        Assert-True -Name 'no stored value reaches the report' `
            -Condition (-not $reportJson.Contains('must-not-leak'))
        Assert-True -Name 'no ETag reaches the report in clear' `
            -Condition (
                -not $reportJson.Contains('etag-seed') -and
                -not $reportJson.Contains('etag-write')
            )
        Assert-True -Name 'the endpoint appears only as a digest' `
            -Condition (
                -not $reportJson.Contains('example-store.azconfig.io') -and
                $report.storeEndpointHash -match '^sha256:[0-9a-f]{64}$'
            )
        Assert-True -Name 'the ETag precondition is recorded as a digest' `
            -Condition (
                @(
                    $report.plannedOperations |
                        Where-Object { $_.condition -ceq 'if-match' }
                ).Count -ge 1
            )
        Assert-True -Name 'the redacted diff carries identifiers and digests only' `
            -Condition (
                $null -ne $report.diff -and
                @($report.diff.changed).Count -eq 1 -and
                $report.diff.changed[0].profileId -ceq 'fast-response'
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
    throw "Foundry profile publish tests failed: $($script:failures.Count)"
}

Write-Output "Foundry profile publish tests passed: $script:passes"
