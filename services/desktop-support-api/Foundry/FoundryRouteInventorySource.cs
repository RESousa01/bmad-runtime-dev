using System.Net;
using System.Net.Http.Headers;
using System.Text.Json;

namespace Sapphirus.DesktopSupportApi.Foundry;

/// <summary>
/// The mode-selection seam (delta item 3). Every <see cref="FoundryInventoryMode"/>
/// resolves to exactly one implementation of this interface, and nothing in
/// this file or its callers ever catches one mode's failure and tries
/// another: see <see cref="FoundryRouteInventorySourceFactory"/> for the one
/// place a mode is chosen.
/// </summary>
public interface IFoundryRouteInventorySource
{
    Task<FoundryDeploymentSnapshot> GetSnapshotAsync(CancellationToken cancellationToken);
}

/// <summary>
/// Raised by every <see cref="IFoundryRouteInventorySource"/> implementation
/// instead of a raw provider exception. The message is always a stable error
/// code; it never carries a credential, endpoint, or provider response body.
/// </summary>
public sealed class FoundryInventoryUnavailableException(
    FoundryDiscoveryOutcome outcome,
    string errorCode) : Exception(errorCode)
{
    public FoundryDiscoveryOutcome Outcome { get; } = outcome;
}

/// <summary>
/// The evidence-required gate shared by every non-default mode (delta item
/// 3, negative test "each mode selected and rejected when its prerequisite
/// evidence is absent"). <see cref="FoundryInventoryMode.ConfiguredAndProbed"/>
/// is the only mode that needs no live qualification, because every route it
/// admits is qualified individually, per refresh, by its own probe.
/// </summary>
public static class FoundryInventoryModeGate
{
    public const string EvidenceRequiredCode = "foundry_inventory_mode_evidence_required";

    public static void RequireEvidenceForNonDefaultMode(
        FoundryInventoryMode mode,
        string? prerequisiteEvidenceHash)
    {
        if (mode != FoundryInventoryMode.ConfiguredAndProbed
            && string.IsNullOrEmpty(prerequisiteEvidenceHash))
        {
            throw new InvalidOperationException(EvidenceRequiredCode);
        }
    }
}

/// <summary>
/// One signed-registry candidate route the
/// <see cref="FoundryInventoryMode.ConfiguredAndProbed"/> source may qualify.
/// The registry itself (FND-024..FND-026) is out of this item's scope; this
/// is the narrow seam the registry is expected to sit behind.
/// </summary>
public sealed record FoundryCandidateRoute(
    string DeploymentName,
    string ModelPublisher,
    string ModelName,
    string ModelVersion,
    FoundryDeploymentSku Sku,
    FoundryDeploymentCapabilities Capabilities);

/// <summary>
/// Supplies the candidate deployment IDs the <c>ConfiguredAndProbed</c> mode
/// probes. Production composes this from the signed model-profile registry;
/// tests script it directly.
/// </summary>
public interface IFoundryCandidateRouteSource
{
    Task<IReadOnlyList<FoundryCandidateRoute>> GetCandidatesAsync(
        CancellationToken cancellationToken);
}

/// <summary>A candidate source with no candidates. Never a failure; an empty snapshot.</summary>
public sealed class EmptyFoundryCandidateRouteSource : IFoundryCandidateRouteSource
{
    public static readonly EmptyFoundryCandidateRouteSource Instance = new();

    public Task<IReadOnlyList<FoundryCandidateRoute>> GetCandidatesAsync(
        CancellationToken cancellationToken) =>
        Task.FromResult<IReadOnlyList<FoundryCandidateRoute>>([]);
}

/// <summary>
/// <see cref="FoundryInventoryMode.ConfiguredAndProbed"/>: candidate IDs come
/// only from <see cref="IFoundryCandidateRouteSource"/>, and each one is
/// qualified with a bounded retrieve-model probe
/// (<c>GET openai/v1/models/{id}</c>) before it may enter the snapshot. The
/// probe count is capped so a large registry can never fan out unboundedly;
/// a candidate beyond the budget is neither probed nor admitted, not treated
/// as a failure.
/// </summary>
public sealed class FoundryConfiguredAndProbedInventorySource : IFoundryRouteInventorySource
{
    public const int DefaultMaximumProbes = 25;
    public const string RetrieveModelPathPrefix = "openai/v1/models/";

    private readonly HttpClient _httpClient;
    private readonly IFoundryRequestAuthenticator _authenticator;
    private readonly Uri _projectEndpoint;
    private readonly IFoundryCandidateRouteSource _candidateSource;
    private readonly TimeProvider _timeProvider;
    private readonly int _maximumProbes;
    private int _lastProbeCount;

    public FoundryConfiguredAndProbedInventorySource(
        HttpClient httpClient,
        IFoundryRequestAuthenticator authenticator,
        Uri projectEndpoint,
        IFoundryCandidateRouteSource candidateSource,
        TimeProvider timeProvider,
        int maximumProbes = DefaultMaximumProbes)
    {
        ArgumentNullException.ThrowIfNull(httpClient);
        ArgumentNullException.ThrowIfNull(authenticator);
        ArgumentNullException.ThrowIfNull(projectEndpoint);
        ArgumentNullException.ThrowIfNull(candidateSource);
        ArgumentNullException.ThrowIfNull(timeProvider);
        if (maximumProbes <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumProbes));
        }
        _httpClient = httpClient;
        _authenticator = authenticator;
        _projectEndpoint = projectEndpoint;
        _candidateSource = candidateSource;
        _timeProvider = timeProvider;
        _maximumProbes = maximumProbes;
    }

    /// <summary>
    /// Probes actually issued by the most recent <see cref="GetSnapshotAsync"/>
    /// call. Test and diagnostic use only.
    /// </summary>
    internal int LastProbeCount => Volatile.Read(ref _lastProbeCount);

    public async Task<FoundryDeploymentSnapshot> GetSnapshotAsync(
        CancellationToken cancellationToken)
    {
        DateTimeOffset retrievedAt = _timeProvider.GetUtcNow();
        IReadOnlyList<FoundryCandidateRoute> candidates;
        try
        {
            candidates = await _candidateSource
                .GetCandidatesAsync(cancellationToken)
                .ConfigureAwait(false);
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception)
        {
            throw new FoundryInventoryUnavailableException(
                FoundryDiscoveryOutcome.ProjectConfiguration,
                "foundry_configured_probe_candidates_unavailable");
        }

        IFoundryRequestAuthenticator operationCredential;
        try
        {
            operationCredential = await _authenticator
                .BeginOperationAsync(cancellationToken)
                .ConfigureAwait(false);
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception)
        {
            throw new FoundryInventoryUnavailableException(
                FoundryDiscoveryOutcome.Authorization,
                "foundry_configured_probe_token_unavailable");
        }

        int bound = Math.Min(candidates.Count, _maximumProbes);
        List<FoundryCandidateRoute> qualified = [];
        int probesIssued = 0;
        for (int index = 0; index < bound; index++)
        {
            cancellationToken.ThrowIfCancellationRequested();
            FoundryCandidateRoute candidate = candidates[index];
            probesIssued++;
            if (await ProbeAsync(operationCredential, candidate.DeploymentName, cancellationToken)
                .ConfigureAwait(false))
            {
                qualified.Add(candidate);
            }
        }
        Volatile.Write(ref _lastProbeCount, probesIssued);

        FoundryNormalizationResult normalized = FoundryDeploymentNormalizer.NormalizeQualifiedCandidates(
            qualified,
            retrievedAt,
            FoundryInventoryMode.ConfiguredAndProbed,
            prerequisiteEvidenceHash: "");
        if (!normalized.Succeeded)
        {
            throw new FoundryInventoryUnavailableException(
                FoundryDiscoveryOutcome.InvalidBody,
                normalized.ErrorCode);
        }
        return normalized.Snapshot!;
    }

    /// <summary>
    /// One bounded retrieve-model probe. A probe that fails to authenticate,
    /// times out, or is refused simply excludes the candidate; it never fails
    /// the whole source, and it never falls back to a different mode.
    /// </summary>
    private async Task<bool> ProbeAsync(
        IFoundryRequestAuthenticator operationCredential,
        string deploymentName,
        CancellationToken cancellationToken)
    {
        using HttpRequestMessage request = new(
            HttpMethod.Get,
            new Uri(_projectEndpoint, RetrieveModelPathPrefix + Uri.EscapeDataString(deploymentName)));
        try
        {
            await operationCredential.ApplyAsync(request, cancellationToken).ConfigureAwait(false);
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception)
        {
            return false;
        }

        try
        {
            using HttpResponseMessage response = await _httpClient
                .SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken)
                .ConfigureAwait(false);
            return response.IsSuccessStatusCode;
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception)
        {
            return false;
        }
    }
}

/// <summary>
/// <see cref="FoundryInventoryMode.OpenAiModels"/>: a single key-authenticated
/// <c>GET openai/v1/models</c> call. Only qualified when
/// <see cref="FoundryInventoryModeGate"/> has already confirmed a live
/// prerequisite-evidence hash; a 401/403 here is terminal and never triggers
/// a fallback to another mode.
/// </summary>
public sealed class FoundryOpenAiModelsInventorySource : IFoundryRouteInventorySource
{
    public const string ModelsPath = "openai/v1/models";
    public const long MaximumAggregateResponseBytes = 1024 * 1024;
    private const string DefaultModelPublisher = "openai";
    private const string DefaultModelVersion = "unversioned";
    private const string DefaultSkuName = "default";

    private readonly HttpClient _httpClient;
    private readonly IFoundryRequestAuthenticator _authenticator;
    private readonly Uri _projectEndpoint;
    private readonly TimeProvider _timeProvider;
    private readonly string _prerequisiteEvidenceHash;

    public FoundryOpenAiModelsInventorySource(
        HttpClient httpClient,
        IFoundryRequestAuthenticator authenticator,
        Uri projectEndpoint,
        TimeProvider timeProvider,
        string prerequisiteEvidenceHash)
    {
        ArgumentNullException.ThrowIfNull(httpClient);
        ArgumentNullException.ThrowIfNull(authenticator);
        ArgumentNullException.ThrowIfNull(projectEndpoint);
        ArgumentNullException.ThrowIfNull(timeProvider);
        ArgumentNullException.ThrowIfNull(prerequisiteEvidenceHash);
        FoundryInventoryModeGate.RequireEvidenceForNonDefaultMode(
            FoundryInventoryMode.OpenAiModels,
            prerequisiteEvidenceHash);
        _httpClient = httpClient;
        _authenticator = authenticator;
        _projectEndpoint = projectEndpoint;
        _timeProvider = timeProvider;
        _prerequisiteEvidenceHash = prerequisiteEvidenceHash;
    }

    public async Task<FoundryDeploymentSnapshot> GetSnapshotAsync(
        CancellationToken cancellationToken)
    {
        DateTimeOffset retrievedAt = _timeProvider.GetUtcNow();
        IFoundryRequestAuthenticator operationCredential;
        try
        {
            operationCredential = await _authenticator
                .BeginOperationAsync(cancellationToken)
                .ConfigureAwait(false);
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception)
        {
            throw new FoundryInventoryUnavailableException(
                FoundryDiscoveryOutcome.Authorization,
                "foundry_openai_models_token_unavailable");
        }

        using HttpRequestMessage request = new(HttpMethod.Get, new Uri(_projectEndpoint, ModelsPath));
        try
        {
            await operationCredential.ApplyAsync(request, cancellationToken).ConfigureAwait(false);
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception)
        {
            throw new FoundryInventoryUnavailableException(
                FoundryDiscoveryOutcome.Authorization,
                "foundry_openai_models_token_unavailable");
        }

        HttpResponseMessage response;
        try
        {
            response = await _httpClient
                .SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken)
                .ConfigureAwait(false);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (HttpRequestException)
        {
            throw new FoundryInventoryUnavailableException(
                FoundryDiscoveryOutcome.Transient,
                "foundry_openai_models_unreachable");
        }

        using (response)
        {
            if (!response.IsSuccessStatusCode)
            {
                (FoundryDiscoveryOutcome outcome, string errorCode) = ClassifyStatus(response.StatusCode);
                throw new FoundryInventoryUnavailableException(outcome, errorCode);
            }
            if (response.Content.Headers.ContentType?.MediaType != "application/json")
            {
                throw new FoundryInventoryUnavailableException(
                    FoundryDiscoveryOutcome.InvalidBody,
                    "foundry_openai_models_content_type_invalid");
            }

            byte[]? body = await ReadBoundedAsync(response.Content, cancellationToken).ConfigureAwait(false);
            if (body is null)
            {
                throw new FoundryInventoryUnavailableException(
                    FoundryDiscoveryOutcome.InvalidBody,
                    "foundry_openai_models_response_too_large");
            }

            List<FoundryCandidateRoute> candidates;
            try
            {
                candidates = ParseCandidates(body);
            }
            catch (JsonException)
            {
                throw new FoundryInventoryUnavailableException(
                    FoundryDiscoveryOutcome.InvalidBody,
                    "foundry_openai_models_body_malformed");
            }

            FoundryNormalizationResult normalized = FoundryDeploymentNormalizer.NormalizeQualifiedCandidates(
                candidates,
                retrievedAt,
                FoundryInventoryMode.OpenAiModels,
                _prerequisiteEvidenceHash);
            if (!normalized.Succeeded)
            {
                throw new FoundryInventoryUnavailableException(
                    FoundryDiscoveryOutcome.InvalidBody,
                    normalized.ErrorCode);
            }
            return normalized.Snapshot!;
        }
    }

    private static List<FoundryCandidateRoute> ParseCandidates(byte[] body)
    {
        using JsonDocument document = JsonDocument.Parse(body);
        List<FoundryCandidateRoute> candidates = [];
        if (!document.RootElement.TryGetProperty("data", out JsonElement data)
            || data.ValueKind != JsonValueKind.Array)
        {
            return candidates;
        }
        foreach (JsonElement item in data.EnumerateArray())
        {
            if (item.ValueKind != JsonValueKind.Object
                || !item.TryGetProperty("id", out JsonElement idElement)
                || idElement.ValueKind != JsonValueKind.String)
            {
                continue;
            }
            string? id = idElement.GetString();
            if (string.IsNullOrEmpty(id))
            {
                continue;
            }
            candidates.Add(new FoundryCandidateRoute(
                id,
                DefaultModelPublisher,
                id,
                DefaultModelVersion,
                new FoundryDeploymentSku(DefaultSkuName, Tier: null, Family: null, Size: null, Capacity: null),
                FoundryDeploymentCapabilities.None));
        }
        return candidates;
    }

    private static (FoundryDiscoveryOutcome Outcome, string ErrorCode) ClassifyStatus(
        HttpStatusCode statusCode) => statusCode switch
        {
            HttpStatusCode.Unauthorized or HttpStatusCode.Forbidden =>
                (FoundryDiscoveryOutcome.Authorization, "foundry_openai_models_unauthorized"),
            HttpStatusCode.NotFound =>
                (FoundryDiscoveryOutcome.ProjectConfiguration, "foundry_openai_models_not_found"),
            HttpStatusCode.RequestTimeout or HttpStatusCode.TooManyRequests =>
                (FoundryDiscoveryOutcome.Transient, "foundry_openai_models_throttled"),
            >= HttpStatusCode.InternalServerError =>
                (FoundryDiscoveryOutcome.Transient, "foundry_openai_models_provider_unavailable"),
            _ => (FoundryDiscoveryOutcome.InvalidBody, "foundry_openai_models_status_unexpected"),
        };

    private static async Task<byte[]?> ReadBoundedAsync(
        HttpContent content,
        CancellationToken cancellationToken)
    {
        await using Stream stream = await content.ReadAsStreamAsync(cancellationToken).ConfigureAwait(false);
        using MemoryStream buffer = new();
        byte[] chunk = new byte[8_192];
        while (true)
        {
            int bytesRead = await stream.ReadAsync(chunk, cancellationToken).ConfigureAwait(false);
            if (bytesRead == 0)
            {
                return buffer.ToArray();
            }
            if (buffer.Length + bytesRead > MaximumAggregateResponseBytes)
            {
                return null;
            }
            buffer.Write(chunk, 0, bytesRead);
        }
    }
}

/// <summary>
/// The one place a <see cref="FoundryInventoryMode"/> is turned into an
/// <see cref="IFoundryRouteInventorySource"/>. It is also the one place the
/// evidence-required gate is enforced for the two non-default modes, so a
/// mode selection either produces a working source or throws before any
/// network call is attempted; it never silently substitutes another mode.
/// </summary>
public static class FoundryRouteInventorySourceFactory
{
    public static IFoundryRouteInventorySource Create(
        FoundryInventoryMode mode,
        string prerequisiteEvidenceHash,
        HttpClient httpClient,
        IFoundryRequestAuthenticator authenticator,
        Uri projectEndpoint,
        TimeProvider timeProvider,
        IFoundryCandidateRouteSource candidateSource,
        int maximumConfiguredAndProbedProbes =
            FoundryConfiguredAndProbedInventorySource.DefaultMaximumProbes)
    {
        ArgumentNullException.ThrowIfNull(prerequisiteEvidenceHash);
        FoundryInventoryModeGate.RequireEvidenceForNonDefaultMode(mode, prerequisiteEvidenceHash);
        return mode switch
        {
            FoundryInventoryMode.ConfiguredAndProbed => new FoundryConfiguredAndProbedInventorySource(
                httpClient,
                authenticator,
                projectEndpoint,
                candidateSource,
                timeProvider,
                maximumConfiguredAndProbedProbes),
            FoundryInventoryMode.OpenAiModels => new FoundryOpenAiModelsInventorySource(
                httpClient,
                authenticator,
                projectEndpoint,
                timeProvider,
                prerequisiteEvidenceHash),
            FoundryInventoryMode.ProjectDeploymentsApiKey => new FoundryDeploymentDiscoveryClient(
                httpClient,
                authenticator,
                projectEndpoint,
                timeProvider,
                mode,
                prerequisiteEvidenceHash),
            _ => throw new ArgumentOutOfRangeException(nameof(mode)),
        };
    }
}
