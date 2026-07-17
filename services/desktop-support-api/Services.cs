using System.Collections.Concurrent;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Sapphirus.DesktopSupportApi;

public sealed class SupportPlaneOptions
{
    public const string SectionName = "Sapphirus";
    public string Authority { get; init; } = "";
    public string Audience { get; init; } = "";
    public string ApprovedDesktopClientId { get; init; } = "";
    public string Region { get; init; } = "development";
    public string ReleaseChannel { get; init; } = "beta";
    public int IdempotencyMaximumEntries { get; init; } = 4096;
    public int IdempotencyRetentionMinutes { get; init; } = 15;
    public int ConnectedOperationTimeoutSeconds { get; init; } = 120;
    public bool DevelopmentSigningEnabled { get; init; }
    public bool DevelopmentModelEnabled { get; init; }
    public string DevelopmentConsentStorePath { get; init; } = "";
    public Guid TenantId { get; private set; }
    public Guid ApprovedDesktopClient { get; private set; }

    public void Validate(IHostEnvironment environment)
    {
        if (!Uri.TryCreate(Authority, UriKind.Absolute, out Uri? authority)
            || authority.Scheme != Uri.UriSchemeHttps
            || !authority.IsDefaultPort
            || authority.UserInfo.Length != 0
            || authority.Query.Length != 0
            || authority.Fragment.Length != 0
            || !string.Equals(
                authority.Host,
                "login.microsoftonline.com",
                StringComparison.OrdinalIgnoreCase)
            || !TryGetTenantId(authority, out Guid tenantId)
            || tenantId == Guid.Empty
            || !string.Equals(
                Authority,
                $"https://login.microsoftonline.com/{tenantId:D}/v2.0",
                StringComparison.Ordinal)
            || !Uri.TryCreate(Audience, UriKind.Absolute, out Uri? audience)
            || !string.Equals(audience.Scheme, "api", StringComparison.Ordinal)
            || !Guid.TryParseExact(ApprovedDesktopClientId, "D", out Guid approvedDesktopClient)
            || approvedDesktopClient == Guid.Empty
            || ReleaseChannel is not ("beta" or "stable")
            || IdempotencyMaximumEntries is < 16 or > 65536
            || IdempotencyRetentionMinutes is < 1 or > 1440
            || ConnectedOperationTimeoutSeconds is < 1 or > 600)
        {
            throw new InvalidOperationException(
                "Sapphirus configuration must name one Entra tenant, API audience, approved desktop client, release channel, and bounded idempotency policy.");
        }
        if (!environment.IsDevelopment()
            && (DevelopmentSigningEnabled
                || DevelopmentModelEnabled
                || !string.IsNullOrWhiteSpace(DevelopmentConsentStorePath)))
        {
            throw new InvalidOperationException(
                "Development signing and model adapters cannot run outside Development.");
        }
        if (!string.IsNullOrWhiteSpace(DevelopmentConsentStorePath)
            && !Path.IsPathFullyQualified(DevelopmentConsentStorePath))
        {
            throw new InvalidOperationException(
                "The development consent store path must be fully qualified.");
        }
        TenantId = tenantId;
        ApprovedDesktopClient = approvedDesktopClient;
    }

    private static bool TryGetTenantId(Uri authority, out Guid tenantId)
    {
        tenantId = Guid.Empty;
        string[] segments = authority.AbsolutePath.Trim('/').Split('/');
        return segments.Length == 2
            && string.Equals(segments[1], "v2.0", StringComparison.Ordinal)
            && Guid.TryParseExact(segments[0], "D", out tenantId);
    }
}

public interface IDeviceRegistry
{
    Task<DeviceRegistrationResponse> RegisterAsync(
        string subject,
        DeviceRegistrationRequest request,
        CancellationToken cancellationToken);
    Task<ActiveDeviceRegistration?> FindActiveAsync(
        string subject,
        string registrationId,
        CancellationToken cancellationToken);
    Task<SignedEntitlementLease> CommitLeaseIfActiveAsync(
        ActiveDeviceRegistration operationLease,
        SignedEntitlementLease lease,
        CancellationToken cancellationToken);
    Task<ModelAccessResult> CommitModelResultIfActiveAsync(
        ActiveDeviceRegistration operationLease,
        ModelAccessRequest request,
        ModelAccessResult result,
        string expectedRegion,
        CancellationToken cancellationToken);
    Task<ModelAccessReceipt?> GetReceiptAsync(
        string subject,
        string receiptId,
        CancellationToken cancellationToken);
    Task<DeviceRevocationOutcome> RevokeAsync(
        string subject,
        string registrationId,
        CancellationToken cancellationToken);
}

public enum DeviceRegistrationState
{
    Active,
    Revoked,
}

public enum DeviceRevocationOutcome
{
    Revoked,
    AlreadyRevoked,
    Unknown,
}

public sealed record RegisteredDevice(
    string Subject,
    string RegistrationId,
    string InstallationPublicKeyHash,
    string ClientRelease,
    string Platform,
    string Architecture,
    string TenantPolicyVersion,
    DateTimeOffset CreatedAt,
    DeviceRegistrationState State,
    DateTimeOffset? RevokedAt)
{
    public bool IsActive => State == DeviceRegistrationState.Active;

    public DeviceRegistrationResponse ToResponse() => new(
        "desktop-device-registration.v1",
        RegistrationId,
        State == DeviceRegistrationState.Active ? "active" : "revoked",
        CreatedAt);
}

public sealed class ActiveDeviceRegistration
{
    internal ActiveDeviceRegistration(
        RegisteredDevice registration,
        long epoch,
        Guid registryAuthority,
        CancellationToken revocationToken)
    {
        Registration = registration;
        Epoch = epoch;
        RegistryAuthority = registryAuthority;
        RevocationToken = revocationToken;
    }

    public RegisteredDevice Registration { get; }
    public CancellationToken RevocationToken { get; }
    internal long Epoch { get; }
    internal Guid RegistryAuthority { get; }
}

public sealed class DeviceRegistrationRevokedException : Exception
{
}

public sealed class MemoryDeviceRegistry : IDeviceRegistry
{
    private sealed class DeviceEntry(
        RegisteredDevice registration,
        Guid registryAuthority,
        ConcurrentDictionary<(string Subject, string ReceiptId), ModelAccessReceipt> receipts)
    {
        private readonly SemaphoreSlim _gate = new(1, 1);
        private readonly CancellationTokenSource _revocation = new();
        private RegisteredDevice _registration = registration;
        private long _epoch = 1;

        public async Task<RegisteredDevice> SnapshotAsync(CancellationToken cancellationToken)
        {
            await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
            try
            {
                return _registration;
            }
            finally
            {
                _gate.Release();
            }
        }

        public async Task<ActiveDeviceRegistration?> TryAcquireAsync(
            CancellationToken cancellationToken)
        {
            await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
            try
            {
                return _registration.IsActive
                    ? new ActiveDeviceRegistration(
                        _registration,
                        _epoch,
                        registryAuthority,
                        _revocation.Token)
                    : null;
            }
            finally
            {
                _gate.Release();
            }
        }

        public async Task<SignedEntitlementLease> CommitLeaseIfActiveAsync(
            long expectedEpoch,
            SignedEntitlementLease lease,
            CancellationToken cancellationToken)
        {
            await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
            try
            {
                cancellationToken.ThrowIfCancellationRequested();
                if (!_registration.IsActive || _epoch != expectedEpoch)
                {
                    throw new DeviceRegistrationRevokedException();
                }
                return lease;
            }
            finally
            {
                _gate.Release();
            }
        }

        public async Task<ModelAccessResult> CommitModelResultIfActiveAsync(
            long expectedEpoch,
            ModelAccessResult result,
            CancellationToken cancellationToken)
        {
            await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
            try
            {
                cancellationToken.ThrowIfCancellationRequested();
                if (!_registration.IsActive || _epoch != expectedEpoch)
                {
                    throw new DeviceRegistrationRevokedException();
                }
                (string Subject, string ReceiptId) key = (
                    _registration.Subject,
                    result.Receipt.ReceiptId);
                if (!receipts.TryAdd(key, result.Receipt))
                {
                    throw new InvalidOperationException(
                        "A model receipt identifier collision was detected.");
                }
                return result;
            }
            finally
            {
                _gate.Release();
            }
        }

        public async Task<DeviceRevocationOutcome> RevokeAsync(
            CancellationToken cancellationToken)
        {
            bool cancelOperations = false;
            await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
            try
            {
                if (!_registration.IsActive)
                {
                    return DeviceRevocationOutcome.AlreadyRevoked;
                }
                _registration = _registration with
                {
                    State = DeviceRegistrationState.Revoked,
                    RevokedAt = DateTimeOffset.UtcNow,
                };
                _epoch = checked(_epoch + 1);
                cancelOperations = true;
            }
            finally
            {
                _gate.Release();
            }
            if (cancelOperations)
            {
                _ = ObserveCancellationAsync(_revocation.CancelAsync());
            }
            return DeviceRevocationOutcome.Revoked;
        }

        private static async Task ObserveCancellationAsync(Task cancellation)
        {
            try
            {
                await cancellation.ConfigureAwait(false);
            }
            catch (AggregateException)
            {
                // Callback failures cannot veto an already-linearized revocation.
            }
        }
    }

    private readonly ConcurrentDictionary<(string Subject, string RegistrationId), DeviceEntry>
        _registrations = new();
    private readonly ConcurrentDictionary<(string Subject, string ReceiptId), ModelAccessReceipt>
        _receipts = new();
    private readonly Guid _registryAuthority = Guid.NewGuid();

    public async Task<DeviceRegistrationResponse> RegisterAsync(
        string subject,
        DeviceRegistrationRequest request,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        string stableInput = $"{subject}:{request.InstallationPublicKeyHash}";
        string registrationId = ContractIds.FromEntropy(
            "dreg",
            SHA256.HashData(Encoding.UTF8.GetBytes(stableInput)));
        (string Subject, string RegistrationId) key = (subject, registrationId);
        while (true)
        {
            if (_registrations.TryGetValue(key, out DeviceEntry? existingEntry))
            {
                RegisteredDevice existing = await existingEntry
                    .SnapshotAsync(cancellationToken)
                    .ConfigureAwait(false);
                if (!existing.IsActive)
                {
                    throw new DeviceRegistrationRevokedException();
                }
                if (!string.Equals(
                    existing.InstallationPublicKeyHash,
                    request.InstallationPublicKeyHash,
                    StringComparison.Ordinal))
                {
                    throw new InvalidOperationException(
                        "A device registration identifier collision was detected.");
                }
                return existing.ToResponse();
            }

            RegisteredDevice registration = new(
                subject,
                registrationId,
                request.InstallationPublicKeyHash,
                request.ClientRelease,
                request.Platform,
                request.Architecture,
                request.TenantPolicyVersion,
                DateTimeOffset.UtcNow,
                DeviceRegistrationState.Active,
                null);
            if (_registrations.TryAdd(
                key,
                new DeviceEntry(registration, _registryAuthority, _receipts)))
            {
                return registration.ToResponse();
            }
        }
    }

    public async Task<ActiveDeviceRegistration?> FindActiveAsync(
        string subject,
        string registrationId,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return _registrations.TryGetValue(
            (subject, registrationId),
            out DeviceEntry? registration)
            ? await registration.TryAcquireAsync(cancellationToken).ConfigureAwait(false)
            : null;
    }

    public Task<SignedEntitlementLease> CommitLeaseIfActiveAsync(
        ActiveDeviceRegistration operationLease,
        SignedEntitlementLease lease,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(lease);
        DeviceEntry entry = ResolveEntry(operationLease, cancellationToken);
        return entry.CommitLeaseIfActiveAsync(
            operationLease.Epoch,
            lease,
            cancellationToken);
    }

    public Task<ModelAccessResult> CommitModelResultIfActiveAsync(
        ActiveDeviceRegistration operationLease,
        ModelAccessRequest request,
        ModelAccessResult result,
        string expectedRegion,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(result);
        DeviceEntry entry = ResolveEntry(operationLease, cancellationToken);
        ModelResultGuards.ValidateOrThrow(
            operationLease.Registration,
            request,
            result,
            expectedRegion);
        cancellationToken.ThrowIfCancellationRequested();
        return entry.CommitModelResultIfActiveAsync(
            operationLease.Epoch,
            result,
            cancellationToken);
    }

    public Task<ModelAccessReceipt?> GetReceiptAsync(
        string subject,
        string receiptId,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        _receipts.TryGetValue((subject, receiptId), out ModelAccessReceipt? receipt);
        return Task.FromResult(receipt);
    }

    public async Task<DeviceRevocationOutcome> RevokeAsync(
        string subject,
        string registrationId,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if (!_registrations.TryGetValue((subject, registrationId), out DeviceEntry? registration))
        {
            return DeviceRevocationOutcome.Unknown;
        }
        return await registration.RevokeAsync(cancellationToken).ConfigureAwait(false);
    }

    private DeviceEntry ResolveEntry(
        ActiveDeviceRegistration operationLease,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(operationLease);
        cancellationToken.ThrowIfCancellationRequested();
        RegisteredDevice registration = operationLease.Registration;
        if (operationLease.RegistryAuthority != _registryAuthority
            || !_registrations.TryGetValue(
                (registration.Subject, registration.RegistrationId),
                out DeviceEntry? entry))
        {
            throw new DeviceRegistrationRevokedException();
        }
        return entry;
    }
}

internal static class CancellableOperation
{
    public static async Task<T> WaitAsync<T>(
        Task<T> operation,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(operation);
        try
        {
            return await operation.WaitAsync(cancellationToken).ConfigureAwait(false);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            Observe(operation);
            throw;
        }
    }

    public static void Observe(Task operation)
    {
        ArgumentNullException.ThrowIfNull(operation);
        _ = operation.ContinueWith(
            static completed => _ = completed.Exception,
            CancellationToken.None,
            TaskContinuationOptions.OnlyOnFaulted
                | TaskContinuationOptions.ExecuteSynchronously,
            TaskScheduler.Default);
    }
}

public interface IIdempotencyStore
{
    Task<T> ExecuteAsync<T>(
        string subject,
        string key,
        string requestFingerprint,
        Func<Task<T>> operation,
        CancellationToken cancellationToken);
}

public sealed class IdempotencyConflictException : Exception
{
}

public sealed class IdempotencyCapacityException : Exception
{
}

public sealed class MemoryIdempotencyStore : IIdempotencyStore
{
    private sealed class Entry(
        string requestFingerprint,
        Lazy<Task<object>> value,
        DateTimeOffset lastAccessedAt)
    {
        public string RequestFingerprint { get; } = requestFingerprint;
        public Lazy<Task<object>> Value { get; } = value;
        public DateTimeOffset LastAccessedAt { get; set; } = lastAccessedAt;
        public bool IsCompleted => Value.IsValueCreated && Value.Value.IsCompleted;
    }

    private readonly object _gate = new();
    private readonly Dictionary<(string Subject, string Key), Entry> _values = new();
    private readonly int _maximumEntries;
    private readonly TimeSpan _retention;
    private readonly TimeProvider _timeProvider;

    public MemoryIdempotencyStore()
        : this(4096, TimeSpan.FromMinutes(15), TimeProvider.System)
    {
    }

    public MemoryIdempotencyStore(
        int maximumEntries,
        TimeSpan retention,
        TimeProvider timeProvider)
    {
        if (maximumEntries < 1)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumEntries));
        }
        if (retention <= TimeSpan.Zero || retention > TimeSpan.FromDays(1))
        {
            throw new ArgumentOutOfRangeException(nameof(retention));
        }
        _maximumEntries = maximumEntries;
        _retention = retention;
        _timeProvider = timeProvider ?? throw new ArgumentNullException(nameof(timeProvider));
    }

    internal int EntryCount
    {
        get
        {
            lock (_gate)
            {
                return _values.Count;
            }
        }
    }

    public async Task<T> ExecuteAsync<T>(
        string subject,
        string key,
        string requestFingerprint,
        Func<Task<T>> operation,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        (string Subject, string Key) entryKey = (subject, key);
        Entry entry;
        bool ownsEntry = false;
        DateTimeOffset now = _timeProvider.GetUtcNow();
        lock (_gate)
        {
            EvictExpiredAndCompleted(now);
            if (_values.TryGetValue(entryKey, out Entry? existing))
            {
                if (!string.Equals(
                    existing.RequestFingerprint,
                    requestFingerprint,
                    StringComparison.Ordinal))
                {
                    throw new IdempotencyConflictException();
                }
                existing.LastAccessedAt = now;
                entry = existing;
            }
            else
            {
                if (_values.Count >= _maximumEntries)
                {
                    throw new IdempotencyCapacityException();
                }
                entry = new Entry(
                    requestFingerprint,
                    new Lazy<Task<object>>(async () =>
                    {
                        T value = await operation().ConfigureAwait(false);
                        return (object?)value
                            ?? throw new InvalidOperationException(
                                "An idempotent operation returned null.");
                    }, LazyThreadSafetyMode.ExecutionAndPublication),
                    now);
                _values.Add(entryKey, entry);
                ownsEntry = true;
            }
        }

        Task<object> task = entry.Value.Value;
        try
        {
            object result = await CancellableOperation
                .WaitAsync(task, cancellationToken)
                .ConfigureAwait(false);
            lock (_gate)
            {
                if (_values.TryGetValue(entryKey, out Entry? current)
                    && ReferenceEquals(current, entry))
                {
                    current.LastAccessedAt = _timeProvider.GetUtcNow();
                }
            }
            return result is T typed
                ? typed
                : throw new InvalidOperationException(
                    "Idempotency key was reused for another response type.");
        }
        catch
        {
            if (task.IsCanceled
                || task.IsFaulted
                || (ownsEntry && cancellationToken.IsCancellationRequested))
            {
                lock (_gate)
                {
                    if (_values.TryGetValue(entryKey, out Entry? current)
                        && ReferenceEquals(current, entry))
                    {
                        _values.Remove(entryKey);
                    }
                }
            }
            throw;
        }
    }

    private void EvictExpiredAndCompleted(DateTimeOffset now)
    {
        foreach ((string Subject, string Key) key in _values
            .Where(pair => pair.Value.IsCompleted
                && now - pair.Value.LastAccessedAt >= _retention)
            .Select(pair => pair.Key)
            .ToArray())
        {
            _values.Remove(key);
        }
    }

}

public sealed record ModelCallCompletionMarker(
    string ReceiptId,
    string RequestHash,
    string ResultHash);

public sealed record ModelCallIdempotencyResult(
    ModelAccessResult? Result,
    ModelCallCompletionMarker? PriorCompletion)
{
    public static ModelCallIdempotencyResult Fresh(ModelAccessResult result) =>
        new(result, null);

    public static ModelCallIdempotencyResult Replay(ModelCallCompletionMarker completion) =>
        new(null, completion);
}

public interface IModelCallIdempotencyStore
{
    Task<ModelCallIdempotencyResult> ExecuteAsync(
        string subject,
        string key,
        string requestFingerprint,
        Func<CancellationToken, Task<ModelAccessResult>> acquireResult,
        Func<ModelAccessResult, CancellationToken, Task<ModelAccessResult>> commitLocalResult,
        CancellationToken cancellationToken);
}

public sealed class MemoryModelCallIdempotencyStore : IModelCallIdempotencyStore
{
    private sealed class Entry(
        string requestFingerprint,
        Task<ModelAccessResult> inFlight,
        DateTimeOffset lastAccessedAt)
    {
        public string RequestFingerprint { get; } = requestFingerprint;
        public Task<ModelAccessResult>? InFlight { get; set; } = inFlight;
        public ModelCallCompletionMarker? Completion { get; set; }
        public DateTimeOffset LastAccessedAt { get; set; } = lastAccessedAt;
    }

    private readonly object _gate = new();
    private readonly Dictionary<(string Subject, string Key), Entry> _entries = new();
    private readonly int _maximumEntries;
    private readonly TimeSpan _retention;
    private readonly TimeProvider _timeProvider;

    public MemoryModelCallIdempotencyStore(
        int maximumEntries,
        TimeSpan retention,
        TimeProvider timeProvider)
    {
        if (maximumEntries < 1)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumEntries));
        }
        if (retention <= TimeSpan.Zero || retention > TimeSpan.FromDays(1))
        {
            throw new ArgumentOutOfRangeException(nameof(retention));
        }
        _maximumEntries = maximumEntries;
        _retention = retention;
        _timeProvider = timeProvider ?? throw new ArgumentNullException(nameof(timeProvider));
    }

    internal int RetainedPayloadTaskCount
    {
        get
        {
            lock (_gate)
            {
                return _entries.Count(pair => pair.Value.InFlight is not null);
            }
        }
    }

    public async Task<ModelCallIdempotencyResult> ExecuteAsync(
        string subject,
        string key,
        string requestFingerprint,
        Func<CancellationToken, Task<ModelAccessResult>> acquireResult,
        Func<ModelAccessResult, CancellationToken, Task<ModelAccessResult>> commitLocalResult,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(acquireResult);
        ArgumentNullException.ThrowIfNull(commitLocalResult);
        cancellationToken.ThrowIfCancellationRequested();
        (string Subject, string Key) entryKey = (subject, key);
        TaskCompletionSource<ModelAccessResult>? owner = null;
        Entry? ownedEntry = null;
        Task<ModelAccessResult> inFlight;
        DateTimeOffset now = _timeProvider.GetUtcNow();
        lock (_gate)
        {
            EvictExpiredMarkers(now);
            if (_entries.TryGetValue(entryKey, out Entry? existing))
            {
                if (!string.Equals(
                    existing.RequestFingerprint,
                    requestFingerprint,
                    StringComparison.Ordinal))
                {
                    throw new IdempotencyConflictException();
                }
                existing.LastAccessedAt = now;
                if (existing.Completion is not null)
                {
                    return ModelCallIdempotencyResult.Replay(existing.Completion);
                }
                inFlight = existing.InFlight
                    ?? throw new InvalidOperationException(
                        "A model-call idempotency entry has no active or completed state.");
            }
            else
            {
                if (_entries.Count >= _maximumEntries)
                {
                    throw new IdempotencyCapacityException();
                }
                owner = new TaskCompletionSource<ModelAccessResult>(
                    TaskCreationOptions.RunContinuationsAsynchronously);
                inFlight = owner.Task;
                ownedEntry = new Entry(requestFingerprint, inFlight, now);
                _entries.Add(entryKey, ownedEntry);
            }
        }

        if (owner is not null)
        {
            try
            {
                Task<ModelAccessResult> acquisitionTask = acquireResult(cancellationToken)
                    ?? throw new InvalidOperationException(
                        "A model-result acquisition returned no task.");
                ModelAccessResult acquired = await CancellableOperation
                    .WaitAsync(acquisitionTask, cancellationToken)
                    .ConfigureAwait(false);
                cancellationToken.ThrowIfCancellationRequested();
                Task<ModelAccessResult> commitTask = commitLocalResult(
                    acquired,
                    cancellationToken)
                    ?? throw new InvalidOperationException(
                        "A local model-result commit returned no task.");
                ModelAccessResult result = await commitTask.ConfigureAwait(false);
                ModelCallCompletionMarker marker = new(
                    result.Receipt.ReceiptId,
                    result.Receipt.RequestHash,
                    result.Receipt.ResultHash);
                lock (_gate)
                {
                    if (_entries.TryGetValue(entryKey, out Entry? current)
                        && ReferenceEquals(current, ownedEntry))
                    {
                        current.InFlight = null;
                        current.Completion = marker;
                        current.LastAccessedAt = _timeProvider.GetUtcNow();
                    }
                    else
                    {
                        throw new InvalidOperationException(
                            "Model-call idempotency ownership was lost before completion.");
                    }
                }
                owner.TrySetResult(result);
                return ModelCallIdempotencyResult.Fresh(result);
            }
            catch (OperationCanceledException exception)
            {
                RemoveOwnedEntry(entryKey, ownedEntry!);
                owner.TrySetCanceled(exception.CancellationToken);
            }
            catch (Exception exception)
            {
                RemoveOwnedEntry(entryKey, ownedEntry!);
                owner.TrySetException(exception);
            }
        }

        ModelAccessResult completed = await CancellableOperation
            .WaitAsync(inFlight, cancellationToken)
            .ConfigureAwait(false);
        return ModelCallIdempotencyResult.Fresh(completed);
    }

    private void RemoveOwnedEntry(
        (string Subject, string Key) entryKey,
        Entry ownedEntry)
    {
        lock (_gate)
        {
            if (_entries.TryGetValue(entryKey, out Entry? current)
                && ReferenceEquals(current, ownedEntry))
            {
                _entries.Remove(entryKey);
            }
        }
    }

    private void EvictExpiredMarkers(DateTimeOffset now)
    {
        foreach ((string Subject, string Key) key in _entries
            .Where(pair => pair.Value.Completion is not null
                && now - pair.Value.LastAccessedAt >= _retention)
            .Select(pair => pair.Key)
            .ToArray())
        {
            _entries.Remove(key);
        }
    }
}

public interface ISignedPolicyService
{
    Task<SignedDesktopPolicy> CurrentPolicyAsync(CancellationToken cancellationToken);
    Task<SignedEntitlementLease> CreateLeaseAsync(
        string subject,
        string registrationId,
        CancellationToken cancellationToken);
}

public sealed class DevelopmentSignedPolicyService(SupportPlaneOptions options) : ISignedPolicyService
{
    public Task<SignedDesktopPolicy> CurrentPolicyAsync(CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        EnsureEnabled();
        SignedDesktopPolicy policy = new(
            "desktop-policy.v1",
            "development-1",
            Hash("development-policy"),
            true,
            512 * 1024,
            64,
            [options.Region],
            "development-ephemeral-not-production",
            "development-signature");
        return Task.FromResult(policy);
    }

    public Task<SignedEntitlementLease> CreateLeaseAsync(
        string subject,
        string registrationId,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        EnsureEnabled();
        DateTimeOffset now = DateTimeOffset.UtcNow;
        SignedEntitlementLease lease = new(
            "desktop-entitlement-lease.v1",
            "lease_" + Guid.NewGuid().ToString("N"),
            registrationId,
            Hash(subject),
            "windows_local",
            now,
            now.AddMinutes(-2),
            now.AddHours(24),
            now.AddHours(96),
            ["local_runtime", "model_access"],
            Hash("development-policy"),
            "0.1.0-beta.1",
            "development-ephemeral-not-production",
            "development-signature");
        return Task.FromResult(lease);
    }

    private void EnsureEnabled()
    {
        if (!options.DevelopmentSigningEnabled)
        {
            throw new InvalidOperationException("A production signing provider is not configured.");
        }
    }

    private static string Hash(string value) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(value)));
}

public interface IModelAccessBroker
{
    Task<ModelAccessResult> CompleteAsync(
        string subject,
        ModelAccessRequest request,
        CancellationToken cancellationToken);
}

public sealed record UnsignedModelAccessResult(
    string OutputSchemaId,
    string PayloadJson,
    string PayloadHash,
    string SchemaProjectionHash,
    string CredentialBindingHash,
    ModelAccessUsage Usage,
    int RetryCount,
    ModelFallbackEvent[] FallbackEvents,
    string? ProviderRequestId,
    DateTimeOffset StartedAt,
    DateTimeOffset CompletedAt,
    string TerminalStatus);

public interface IModelReceiptSigner
{
    Task<ModelAccessResult> SignAsync(
        string subject,
        ModelAccessRequest request,
        UnsignedModelAccessResult result,
        CancellationToken cancellationToken);
}

public enum ContextConsentVerification
{
    Verified,
    Rejected,
    Unavailable,
}

public sealed record ContextConsentVerificationRequest(
    string Subject,
    RegisteredDevice Device,
    ModelAccessRequest Request,
    string RecomputedManifestHash);

public interface IContextConsentVerifier
{
    ValueTask<ContextConsentVerification> VerifyAsync(
        ContextConsentVerificationRequest request,
        CancellationToken cancellationToken);
}

/// <summary>
/// The canonical consent envelope is present and structurally bound to the exact request. The
/// default still fails closed because installation-key signature verification and durable
/// single-use consumption have no production provider until the external key resources exist.
/// </summary>
public sealed class UnavailableContextConsentVerifier : IContextConsentVerifier
{
    public ValueTask<ContextConsentVerification> VerifyAsync(
        ContextConsentVerificationRequest request,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return ValueTask.FromResult(ContextConsentVerification.Unavailable);
    }
}

public sealed class DevelopmentModelReceiptSigner(SupportPlaneOptions options) : IModelReceiptSigner
{
    public Task<ModelAccessResult> SignAsync(
        string subject,
        ModelAccessRequest request,
        UnsignedModelAccessResult result,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if (!options.DevelopmentSigningEnabled)
        {
            throw new InvalidOperationException(
                "A production model-receipt signing provider is not configured.");
        }
        string recomputedResultHash = Hash(result.PayloadJson);
        if (!string.Equals(recomputedResultHash, result.PayloadHash, StringComparison.Ordinal))
        {
            throw new InvalidOperationException(
                "The unsigned model result did not match its declared payload hash.");
        }
        string requestHash = RequestGuards.Fingerprint(request);
        string receiptHash = Hash(
            $"development-receipt:{requestHash}:{result.PayloadHash}:{request.Consent.ConsentEnvelopeHash}");
        ModelAccessReceipt receipt = new(
            "sapphirus.model-access-receipt.v1",
            ContractIds.FromEntropy("receipt", RandomNumberGenerator.GetBytes(16)),
            request.RequestId,
            requestHash,
            result.PayloadHash,
            "windows_local",
            request.Consent.TenantHash,
            request.Consent.SubjectHash,
            request.RegistrationId,
            request.LocalEgressManifestHash,
            request.Consent.InvocationBindingHash,
            request.Consent.ConsumptionHash,
            request.Consent.ConsentEnvelopeHash,
            request.Consent.ConsentDisclosureHash,
            request.Consent.ProviderProfileHash,
            request.Consent.ModelProfileHash,
            request.Consent.ModelCapabilityHash,
            request.Consent.DeploymentHash,
            request.CanonicalOutputSchemaId,
            request.CanonicalOutputSchemaHash,
            result.SchemaProjectionHash,
            result.CredentialBindingHash,
            request.RetentionMode,
            options.Region,
            request.Items.Sum(item => item.ByteCount),
            Encoding.UTF8.GetByteCount(result.PayloadJson),
            result.Usage,
            result.RetryCount,
            result.FallbackEvents,
            result.ProviderRequestId,
            result.StartedAt,
            result.CompletedAt,
            result.TerminalStatus,
            receiptHash,
            new ModelAccessReceiptProof(
                "support_plane_signature",
                "ES256",
                "https://development.invalid/",
                options.Audience,
                "development-model-receipt-key",
                receiptHash,
                "ZGV2ZWxvcG1lbnQtb25seS1uby10cnVzdA"));
        return Task.FromResult(new ModelAccessResult(
            "desktop-model-access-result.v1",
            request.RequestId,
            result.OutputSchemaId,
            result.PayloadJson,
            result.PayloadHash,
            receipt));
    }

    private static string Hash(string value) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(value)));
}

public sealed class DevelopmentModelAccessBroker(
    SupportPlaneOptions options,
    IModelReceiptSigner receiptSigner) : IModelAccessBroker
{
    public async Task<ModelAccessResult> CompleteAsync(
        string subject,
        ModelAccessRequest request,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if (!options.DevelopmentModelEnabled)
        {
            throw new InvalidOperationException("A managed-identity model broker is not configured.");
        }
        DateTimeOffset startedAt = DateTimeOffset.UtcNow;
        string payload = JsonSerializer.Serialize(new
        {
            summary = "Review the selected context and prepare a bounded change proposal.",
            steps = new[] { "Understand context", "Plan changes", "Review before applying" },
            proposedChanges = Array.Empty<object>(),
        });
        string resultHash = Hash(payload);
        UnsignedModelAccessResult unsigned = new(
            request.CanonicalOutputSchemaId,
            payload,
            resultHash,
            Hash("development-schema-projection"),
            Hash($"development-credential-binding:{subject}"),
            new ModelAccessUsage(0, 0, 0, "EUR"),
            0,
            [],
            null,
            startedAt,
            DateTimeOffset.UtcNow,
            "succeeded");
        return await receiptSigner.SignAsync(subject, request, unsigned, cancellationToken)
            .ConfigureAwait(false);
    }

    private static string Hash(string value) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(value)));
}
