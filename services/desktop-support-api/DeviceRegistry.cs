using System.Collections.Concurrent;
using System.Security.Cryptography;
using System.Text;

namespace Sapphirus.DesktopSupportApi;

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
    string InstallationPublicKey,
    string InstallationPublicKeyHash,
    string ClientRelease,
    string Platform,
    string Architecture,
    long TenantPolicyVersion,
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
        if (!RequestGuards.TryGetInstallationPublicKeyHash(
            request.InstallationPublicKey,
            out string installationPublicKeyHash)
            || !string.Equals(
                installationPublicKeyHash,
                request.InstallationPublicKeyHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "The installation public key is invalid.",
                nameof(request));
        }
        string stableInput = $"{subject}:{installationPublicKeyHash}";
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
                    installationPublicKeyHash,
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
                request.InstallationPublicKey,
                installationPublicKeyHash,
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
