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
    public bool DevelopmentSigningEnabled { get; init; }
    public bool DevelopmentModelEnabled { get; init; }
    public Guid TenantId { get; private set; }
    public Guid ApprovedDesktopClient { get; private set; }

    public void Validate(IHostEnvironment environment)
    {
        if (!Uri.TryCreate(Authority, UriKind.Absolute, out Uri? authority)
            || authority.Scheme != Uri.UriSchemeHttps
            || !string.Equals(
                authority.Host,
                "login.microsoftonline.com",
                StringComparison.OrdinalIgnoreCase)
            || !TryGetTenantId(authority, out Guid tenantId)
            || tenantId == Guid.Empty
            || !Uri.TryCreate(Audience, UriKind.Absolute, out Uri? audience)
            || !string.Equals(audience.Scheme, "api", StringComparison.Ordinal)
            || !Guid.TryParseExact(ApprovedDesktopClientId, "D", out Guid approvedDesktopClient)
            || approvedDesktopClient == Guid.Empty
            || ReleaseChannel is not ("beta" or "stable")
            || IdempotencyMaximumEntries is < 16 or > 65536
            || IdempotencyRetentionMinutes is < 1 or > 1440)
        {
            throw new InvalidOperationException(
                "Sapphirus configuration must name one Entra tenant, API audience, approved desktop client, release channel, and bounded idempotency policy.");
        }
        if (!environment.IsDevelopment()
            && (DevelopmentSigningEnabled || DevelopmentModelEnabled))
        {
            throw new InvalidOperationException(
                "Development signing and model adapters cannot run outside Development.");
        }
        TenantId = tenantId;
        ApprovedDesktopClient = approvedDesktopClient;
    }

    private static bool TryGetTenantId(Uri authority, out Guid tenantId)
    {
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

public sealed record ActiveDeviceRegistration(
    RegisteredDevice Registration,
    CancellationToken RevocationToken);

public sealed class DeviceRegistrationRevokedException : Exception
{
}

public sealed class MemoryDeviceRegistry : IDeviceRegistry
{
    private sealed class DeviceEntry(RegisteredDevice registration)
    {
        private readonly object _gate = new();
        private readonly CancellationTokenSource _revocation = new();
        private RegisteredDevice _registration = registration;

        public RegisteredDevice Snapshot()
        {
            lock (_gate)
            {
                return _registration;
            }
        }

        public ActiveDeviceRegistration? TryAcquire()
        {
            lock (_gate)
            {
                return _registration.IsActive
                    ? new ActiveDeviceRegistration(_registration, _revocation.Token)
                    : null;
            }
        }

        public DeviceRevocationOutcome Revoke()
        {
            lock (_gate)
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
            }
            _revocation.Cancel();
            return DeviceRevocationOutcome.Revoked;
        }
    }

    private readonly ConcurrentDictionary<(string Subject, string RegistrationId), DeviceEntry>
        _registrations = new();

    public Task<DeviceRegistrationResponse> RegisterAsync(
        string subject,
        DeviceRegistrationRequest request,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        string stableInput = $"{subject}:{request.InstallationPublicKeyHash}";
        string registrationId = "dreg_" + Convert.ToHexStringLower(
            SHA256.HashData(Encoding.UTF8.GetBytes(stableInput)))[..26];
        (string Subject, string RegistrationId) key = (subject, registrationId);
        while (true)
        {
            if (_registrations.TryGetValue(key, out DeviceEntry? existingEntry))
            {
                RegisteredDevice existing = existingEntry.Snapshot();
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
                return Task.FromResult(existing.ToResponse());
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
            if (_registrations.TryAdd(key, new DeviceEntry(registration)))
            {
                return Task.FromResult(registration.ToResponse());
            }
        }
    }

    public Task<ActiveDeviceRegistration?> FindActiveAsync(
        string subject,
        string registrationId,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return Task.FromResult(
            _registrations.TryGetValue((subject, registrationId), out DeviceEntry? registration)
                ? registration.TryAcquire()
                : null);
    }

    public Task<DeviceRevocationOutcome> RevokeAsync(
        string subject,
        string registrationId,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if (!_registrations.TryGetValue((subject, registrationId), out DeviceEntry? registration))
        {
            return Task.FromResult(DeviceRevocationOutcome.Unknown);
        }
        return Task.FromResult(registration.Revoke());
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
            }
        }

        Task<object> task = entry.Value.Value;
        try
        {
            object result = await task.WaitAsync(cancellationToken).ConfigureAwait(false);
            return result is T typed
                ? typed
                : throw new InvalidOperationException(
                    "Idempotency key was reused for another response type.");
        }
        catch
        {
            if (task.IsCanceled || task.IsFaulted)
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
/// The current wire shape carries only an opaque consent receipt hash. Without the signed receipt
/// fields, deployment/model/region/profile binding, or device proof, the service cannot validate
/// that the user approved these bytes. The default therefore fails closed until the canonical
/// contract and a verifier are implemented.
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

public sealed class DevelopmentModelAccessBroker(SupportPlaneOptions options) : IModelAccessBroker
{
    public Task<ModelAccessResult> CompleteAsync(
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
        string requestHash = Hash(JsonSerializer.Serialize(request));
        string resultHash = Hash(payload);
        ModelAccessReceipt receipt = new(
            "desktop-model-access-receipt.v1",
            "receipt_" + Guid.NewGuid().ToString("N"),
            requestHash,
            resultHash,
            request.LocalEgressManifestHash,
            request.ConsentReceiptHash,
            Hash($"development-profile:{subject}"),
            "transient_no_store",
            options.Region,
            request.Items.Sum(item => item.ByteCount),
            Encoding.UTF8.GetByteCount(payload),
            startedAt,
            DateTimeOffset.UtcNow,
            "succeeded");
        return Task.FromResult(new ModelAccessResult(
            "desktop-model-access-result.v1",
            request.RequestId,
            request.CanonicalOutputSchemaId,
            payload,
            resultHash,
            receipt));
    }

    private static string Hash(string value) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(value)));
}

public interface IReceiptStore
{
    Task AddAsync(string subject, ModelAccessReceipt receipt, CancellationToken cancellationToken);
    Task<ModelAccessReceipt?> GetAsync(
        string subject,
        string receiptId,
        CancellationToken cancellationToken);
}

public sealed class MemoryReceiptStore : IReceiptStore
{
    private readonly ConcurrentDictionary<(string Subject, string ReceiptId), ModelAccessReceipt>
        _receipts = new();

    public Task AddAsync(
        string subject,
        ModelAccessReceipt receipt,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        _receipts[(subject, receipt.ReceiptId)] = receipt;
        return Task.CompletedTask;
    }

    public Task<ModelAccessReceipt?> GetAsync(
        string subject,
        string receiptId,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        _receipts.TryGetValue((subject, receiptId), out ModelAccessReceipt? receipt);
        return Task.FromResult(receipt);
    }
}
