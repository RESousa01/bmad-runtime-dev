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
