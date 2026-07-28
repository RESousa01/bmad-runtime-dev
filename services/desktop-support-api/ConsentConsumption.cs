using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Sapphirus.DesktopSupportApi;

public enum ContextConsentConsumption
{
    Consumed,
    AlreadyConsumed,
    Unavailable,
}

public sealed record ContextConsentConsumptionRequest(
    string SubjectPartition,
    string RegistrationId,
    string DecisionId,
    string RequestId,
    string ConsumptionHash,
    DateTimeOffset ConsumedAt);

public interface IContextConsentConsumptionStore
{
    ValueTask<ContextConsentConsumption> ConsumeAsync(
        ContextConsentConsumptionRequest request,
        CancellationToken cancellationToken);
}

public sealed class ContextConsentAlreadyConsumedException : Exception;

public sealed class ContextConsentConsumptionUnavailableException : Exception;

public sealed class UnavailableContextConsentConsumptionStore : IContextConsentConsumptionStore
{
    public ValueTask<ContextConsentConsumption> ConsumeAsync(
        ContextConsentConsumptionRequest request,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return ValueTask.FromResult(ContextConsentConsumption.Unavailable);
    }
}

/// <summary>
/// Process-lifetime single-use consent authority for Development.
///
/// <para>
/// It is the Development default rather than
/// <see cref="UnavailableContextConsentConsumptionStore"/>, which stays the
/// default for any composition that has no store at all. The distinction
/// matters: Production composes the durable SQL authority unconditionally, so
/// the unavailable default was never what protected a deployed plane — it only
/// made a local run unable to spend consent at all, which in turn made the
/// streaming route unusable on the one machine that can reach the real project.
/// </para>
/// <para>
/// This store enforces single use for the lifetime of the process and no
/// further. An operator who needs single use to survive a restart sets
/// <c>Sapphirus__DevelopmentConsentStorePath</c> and gets
/// <see cref="DevelopmentFileContextConsentConsumptionStore"/> instead; an
/// operator who needs it to survive across replicas is in Production and gets
/// the SQL store. It refuses the same malformed authority the durable stores
/// refuse, so a request that would be rejected there is rejected here too.
/// </para>
/// </summary>
public sealed class MemoryContextConsentConsumptionStore : IContextConsentConsumptionStore
{
    private readonly Lock _gate = new();
    private readonly HashSet<string> _consumed = new(StringComparer.Ordinal);

    public ValueTask<ContextConsentConsumption> ConsumeAsync(
        ContextConsentConsumptionRequest request,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        cancellationToken.ThrowIfCancellationRequested();
        if (!ContractIds.Is(request.RegistrationId, "dreg")
            || string.IsNullOrWhiteSpace(request.SubjectPartition)
            || string.IsNullOrWhiteSpace(request.DecisionId)
            || string.IsNullOrWhiteSpace(request.RequestId)
            || !RequestGuards.IsSha256(request.ConsumptionHash)
            || request.ConsumedAt == default)
        {
            throw new ArgumentException(
                "The consent consumption authority is invalid.",
                nameof(request));
        }
        string authority =
            $"{request.SubjectPartition}\0{request.RegistrationId}\0{request.ConsumptionHash}";
        lock (_gate)
        {
            return ValueTask.FromResult(_consumed.Add(authority)
                ? ContextConsentConsumption.Consumed
                : ContextConsentConsumption.AlreadyConsumed);
        }
    }
}

public sealed class DevelopmentFileContextConsentConsumptionStore : IContextConsentConsumptionStore
{
    private readonly string _root;

    public DevelopmentFileContextConsentConsumptionStore(string root)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(root);
        _root = Path.GetFullPath(root);
        Directory.CreateDirectory(_root);
    }

    public async ValueTask<ContextConsentConsumption> ConsumeAsync(
        ContextConsentConsumptionRequest request,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        cancellationToken.ThrowIfCancellationRequested();
        if (!ContractIds.Is(request.RegistrationId, "dreg")
            || string.IsNullOrWhiteSpace(request.SubjectPartition)
            || string.IsNullOrWhiteSpace(request.DecisionId)
            || string.IsNullOrWhiteSpace(request.RequestId)
            || !RequestGuards.IsSha256(request.ConsumptionHash)
            || request.ConsumedAt == default)
        {
            throw new ArgumentException("The consent consumption authority is invalid.", nameof(request));
        }

        string authorityHash = Hash(
            $"{request.SubjectPartition}\0{request.RegistrationId}\0{request.ConsumptionHash}");
        string markerPath = Path.Combine(_root, authorityHash["sha256:".Length..] + ".json");
        byte[] marker = JsonSerializer.SerializeToUtf8Bytes(new
        {
            schemaVersion = "sapphirus.context-consent-consumption.v1",
            subjectHash = Hash(request.SubjectPartition),
            request.RegistrationId,
            request.DecisionId,
            request.RequestId,
            request.ConsumptionHash,
            consumedAt = request.ConsumedAt.ToUniversalTime().ToString("yyyy-MM-dd'T'HH:mm:ss.fff'Z'"),
        });
        try
        {
            await using FileStream stream = new(
                markerPath,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.Read,
                4096,
                FileOptions.Asynchronous | FileOptions.WriteThrough);
            await stream.WriteAsync(marker, cancellationToken).ConfigureAwait(false);
            await stream.FlushAsync(cancellationToken).ConfigureAwait(false);
            return ContextConsentConsumption.Consumed;
        }
        catch (IOException) when (File.Exists(markerPath))
        {
            return ContextConsentConsumption.AlreadyConsumed;
        }
    }

    private static string Hash(string value) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(value)));
}
