using System.Net;
using System.Net.Http.Headers;
using System.Text.Json;
using System.Text.Json.Serialization;
using Azure.Core;

namespace Sapphirus.DesktopSupportApi.Foundry;

/// <summary>
/// Reads the deployment inventory of one Foundry project.
///
/// Three properties are load bearing. Pagination never leaves the project it
/// started in, so a hostile <c>nextLink</c> cannot turn this into a
/// server-side request forgery primitive. Every body is bounded in aggregate,
/// so a large or compressed response cannot exhaust the host. And no raw
/// provider field, deployment name, endpoint, or token is ever logged or
/// surfaced: failures are stable error codes, and retrieval facts stay in
/// server-only evidence.
///
/// It does not know how the request is authenticated. That is
/// <see cref="IFoundryRequestAuthenticator"/>'s job, and the credential is
/// acquired once per discovery operation rather than once per page.
/// </summary>
public sealed class FoundryDeploymentDiscoveryClient
{
    /// <summary>
    /// The Entra scope the managed-identity mechanism requests. It stays here
    /// as the documented discovery scope; the single definition lives on the
    /// authenticator that actually asks for it.
    /// </summary>
    public const string TokenScope = ManagedIdentityFoundryAuthenticator.TokenScope;
    public const string ClientRequestIdHeader = "x-ms-client-request-id";
    public const string ProviderRequestIdHeader = "x-ms-request-id";
    public const int MaximumPages = 32;
    public const long MaximumAggregateResponseBytes = 1024 * 1024;

    private const string ProjectPathPrefix = "/api/projects/";
    private const string DeploymentsSegment = "deployments";
    private const string DeploymentsQuery =
        "deployments?api-version=v1&deploymentType=ModelDeployment";
    private const string ApiVersionValue = "v1";
    private const string DeploymentTypeValue = "ModelDeployment";
    private const int MaximumRequestIdLength = 128;

    private static readonly JsonSerializerOptions ProviderJson = new()
    {
        // Case-sensitive and duplicate-hostile, matching the support plane's
        // own strictness. Unknown members are skipped rather than rejected:
        // Foundry may add fields at any time, and the DTO layer is what stops
        // them from reaching the canonical contract.
        PropertyNameCaseInsensitive = false,
        AllowDuplicateProperties = false,
        UnmappedMemberHandling = JsonUnmappedMemberHandling.Skip,
        NumberHandling = JsonNumberHandling.Strict,
        MaxDepth = 16,
    };

    private readonly HttpClient _httpClient;
    private readonly IFoundryRequestAuthenticator _authenticator;
    private readonly Uri _projectEndpoint;
    private readonly TimeProvider _timeProvider;
    private readonly FoundryInventoryMode _inventoryMode;
    private readonly string _prerequisiteEvidenceHash;

    public FoundryDeploymentDiscoveryClient(
        HttpClient httpClient,
        IFoundryRequestAuthenticator authenticator,
        Uri projectEndpoint,
        TimeProvider timeProvider)
        : this(
            httpClient,
            authenticator,
            projectEndpoint,
            timeProvider,
            FoundryInventoryMode.ProjectDeploymentsApiKey,
            prerequisiteEvidenceHash: "")
    {
    }

    /// <summary>
    /// The full constructor. <paramref name="inventoryMode"/> and
    /// <paramref name="prerequisiteEvidenceHash"/> are folded into the
    /// canonical snapshot hash (delta compatibility decision 1): this client
    /// always speaks the <see cref="FoundryInventoryMode.ProjectDeploymentsApiKey"/>
    /// wire shape, so callers using other modes construct
    /// <see cref="IFoundryRouteInventorySource"/> implementations directly
    /// instead of this type.
    /// </summary>
    public FoundryDeploymentDiscoveryClient(
        HttpClient httpClient,
        IFoundryRequestAuthenticator authenticator,
        Uri projectEndpoint,
        TimeProvider timeProvider,
        FoundryInventoryMode inventoryMode,
        string prerequisiteEvidenceHash)
    {
        ArgumentNullException.ThrowIfNull(httpClient);
        ArgumentNullException.ThrowIfNull(authenticator);
        ArgumentNullException.ThrowIfNull(projectEndpoint);
        ArgumentNullException.ThrowIfNull(timeProvider);
        ArgumentNullException.ThrowIfNull(prerequisiteEvidenceHash);
        if (!IsProjectEndpointShape(projectEndpoint))
        {
            throw new ArgumentException(
                "foundry_project_endpoint_invalid",
                nameof(projectEndpoint));
        }
        _httpClient = httpClient;
        _authenticator = authenticator;
        _projectEndpoint = projectEndpoint;
        _timeProvider = timeProvider;
        _inventoryMode = inventoryMode;
        _prerequisiteEvidenceHash = prerequisiteEvidenceHash;
    }

    /// <summary>
    /// Convenience overload for the managed-identity mechanism, retained only
    /// so historical callers and V1-era tests still compile.
    /// <see cref="ManagedIdentityFoundryAuthenticator"/> and is otherwise
    /// identical to the seam-taking constructor.
    /// </summary>
    public FoundryDeploymentDiscoveryClient(
        HttpClient httpClient,
        TokenCredential credential,
        Uri projectEndpoint,
        TimeProvider timeProvider)
        : this(
            httpClient,
            new ManagedIdentityFoundryAuthenticator(credential),
            projectEndpoint,
            timeProvider)
    {
    }

    /// <summary>
    /// How the outbound requests of this client are authenticated. It never
    /// carries the credential itself, only the mechanism identifier.
    /// </summary>
    public string CredentialMechanismId => _authenticator.CredentialMechanismId;

    public async Task<FoundryDiscoveryResult> DiscoverAsync(
        CancellationToken cancellationToken)
    {
        DateTimeOffset retrievedAt = _timeProvider.GetUtcNow();
        List<string> clientRequestIds = [];
        List<string> providerRequestIds = [];
        long totalBytes = 0;
        int pageCount = 0;

        // The credential is acquired once for the whole operation, so a
        // paginated inventory costs one acquisition rather than one per page.
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
            // Never propagate credential exception text: it routinely carries
            // tenant, client, and endpoint detail.
            return Failure(
                FoundryDiscoveryOutcome.Authorization,
                "foundry_discovery_token_unavailable",
                Evidence(retrievedAt, pageCount, totalBytes, clientRequestIds, providerRequestIds));
        }

        Uri current = new(_projectEndpoint, DeploymentsQuery);
        HashSet<string> visited = new(StringComparer.Ordinal);
        List<FoundryDeploymentDto> collected = [];

        while (true)
        {
            if (!visited.Add(current.AbsoluteUri))
            {
                return Failure(
                    FoundryDiscoveryOutcome.PaginationIntegrity,
                    "foundry_pagination_cycle",
                    Evidence(retrievedAt, pageCount, totalBytes, clientRequestIds, providerRequestIds));
            }
            if (pageCount >= MaximumPages)
            {
                return Failure(
                    FoundryDiscoveryOutcome.PaginationIntegrity,
                    "foundry_pagination_page_limit",
                    Evidence(retrievedAt, pageCount, totalBytes, clientRequestIds, providerRequestIds));
            }

            using HttpRequestMessage request = new(HttpMethod.Get, current);
            try
            {
                await operationCredential
                    .ApplyAsync(request, cancellationToken)
                    .ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                throw;
            }
            catch (Exception)
            {
                // An unauthenticated request is never sent, and the credential
                // failure text never escapes.
                return Failure(
                    FoundryDiscoveryOutcome.Authorization,
                    "foundry_discovery_token_unavailable",
                    Evidence(retrievedAt, pageCount, totalBytes, clientRequestIds, providerRequestIds));
            }
            string clientRequestId = Guid.NewGuid().ToString("D");
            clientRequestIds.Add(clientRequestId);
            request.Headers.TryAddWithoutValidation(ClientRequestIdHeader, clientRequestId);

            HttpResponseMessage response;
            try
            {
                response = await _httpClient
                    .SendAsync(
                        request,
                        HttpCompletionOption.ResponseHeadersRead,
                        cancellationToken)
                    .ConfigureAwait(false);
            }
            catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
            {
                throw;
            }
            catch (OperationCanceledException)
            {
                // The bounded client timeout elapsed.
                return Failure(
                    FoundryDiscoveryOutcome.Transient,
                    "foundry_discovery_timeout",
                    Evidence(retrievedAt, pageCount, totalBytes, clientRequestIds, providerRequestIds));
            }
            catch (HttpRequestException)
            {
                return Failure(
                    FoundryDiscoveryOutcome.Transient,
                    "foundry_discovery_unreachable",
                    Evidence(retrievedAt, pageCount, totalBytes, clientRequestIds, providerRequestIds));
            }

            using (response)
            {
                pageCount++;
                CollectProviderRequestId(response, providerRequestIds);

                if (!response.IsSuccessStatusCode)
                {
                    (FoundryDiscoveryOutcome outcome, string errorCode) =
                        ClassifyStatus(response.StatusCode);
                    return Failure(
                        outcome,
                        errorCode,
                        Evidence(retrievedAt, pageCount, totalBytes, clientRequestIds, providerRequestIds));
                }
                if (!IsJsonResponse(response.Content.Headers.ContentType))
                {
                    return Failure(
                        FoundryDiscoveryOutcome.InvalidBody,
                        "foundry_discovery_content_type_invalid",
                        Evidence(retrievedAt, pageCount, totalBytes, clientRequestIds, providerRequestIds));
                }

                long remaining = MaximumAggregateResponseBytes - totalBytes;
                if (response.Content.Headers.ContentLength is long declared
                    && declared > remaining)
                {
                    return Failure(
                        FoundryDiscoveryOutcome.InvalidBody,
                        "foundry_discovery_response_too_large",
                        Evidence(retrievedAt, pageCount, totalBytes, clientRequestIds, providerRequestIds));
                }

                byte[]? body = await ReadBoundedAsync(
                        response.Content,
                        remaining,
                        cancellationToken)
                    .ConfigureAwait(false);
                if (body is null)
                {
                    return Failure(
                        FoundryDiscoveryOutcome.InvalidBody,
                        "foundry_discovery_response_too_large",
                        Evidence(retrievedAt, pageCount, totalBytes, clientRequestIds, providerRequestIds));
                }
                totalBytes += body.Length;

                FoundryDeploymentPageDto? page;
                try
                {
                    page = JsonSerializer.Deserialize<FoundryDeploymentPageDto>(
                        body,
                        ProviderJson);
                }
                catch (JsonException)
                {
                    return Failure(
                        FoundryDiscoveryOutcome.InvalidBody,
                        "foundry_discovery_body_malformed",
                        Evidence(retrievedAt, pageCount, totalBytes, clientRequestIds, providerRequestIds));
                }
                if (page?.Value is null)
                {
                    return Failure(
                        FoundryDiscoveryOutcome.InvalidBody,
                        "foundry_discovery_body_malformed",
                        Evidence(retrievedAt, pageCount, totalBytes, clientRequestIds, providerRequestIds));
                }

                collected.AddRange(page.Value);
                if (collected.Count > FoundryDeploymentSnapshot.MaximumDeployments)
                {
                    return Failure(
                        FoundryDiscoveryOutcome.InvalidBody,
                        "foundry_deployment_count_exceeded",
                        Evidence(retrievedAt, pageCount, totalBytes, clientRequestIds, providerRequestIds));
                }

                if (string.IsNullOrEmpty(page.NextLink))
                {
                    break;
                }
                if (!TryResolveNextLink(
                    _projectEndpoint,
                    page.NextLink,
                    out Uri? next,
                    out string nextLinkError))
                {
                    return Failure(
                        FoundryDiscoveryOutcome.PaginationIntegrity,
                        nextLinkError,
                        Evidence(retrievedAt, pageCount, totalBytes, clientRequestIds, providerRequestIds));
                }
                current = next!;
            }
        }

        FoundryNormalizationResult normalized =
            FoundryDeploymentNormalizer.Normalize(
                collected,
                retrievedAt,
                _inventoryMode,
                _prerequisiteEvidenceHash);
        FoundryDiscoveryEvidence evidence = Evidence(
            retrievedAt,
            pageCount,
            totalBytes,
            clientRequestIds,
            providerRequestIds);
        return normalized.Succeeded
            ? new FoundryDiscoveryResult(
                FoundryDiscoveryOutcome.Success,
                "",
                normalized.Snapshot,
                evidence)
            : Failure(
                FoundryDiscoveryOutcome.InvalidBody,
                normalized.ErrorCode,
                evidence);
    }

    /// <summary>
    /// The SSRF gate. A continuation link is followed only when it stays on
    /// the same origin and the same project deployment path, carries no
    /// credentials or fragment, and keeps the pinned API version and
    /// deployment type. Everything else is an integrity failure, not a retry.
    /// </summary>
    internal static bool TryResolveNextLink(
        Uri projectEndpoint,
        string nextLink,
        out Uri? next,
        out string errorCode)
    {
        next = null;
        errorCode = "";
        if (nextLink.Length > 2048
            || !Uri.TryCreate(nextLink, UriKind.Absolute, out Uri? candidate))
        {
            errorCode = "foundry_pagination_uri_invalid";
            return false;
        }
        if (candidate.Scheme != Uri.UriSchemeHttps
            || !candidate.IsDefaultPort
            || candidate.Port != 443
            || candidate.UserInfo.Length != 0
            || candidate.Fragment.Length != 0
            || !string.Equals(
                candidate.Host,
                projectEndpoint.Host,
                StringComparison.OrdinalIgnoreCase))
        {
            errorCode = "foundry_pagination_origin_invalid";
            return false;
        }
        if (!string.Equals(
            candidate.AbsolutePath,
            projectEndpoint.AbsolutePath + DeploymentsSegment,
            StringComparison.Ordinal))
        {
            errorCode = "foundry_pagination_path_invalid";
            return false;
        }
        if (!IsAllowedPaginationQuery(candidate.Query))
        {
            errorCode = "foundry_pagination_query_invalid";
            return false;
        }
        next = candidate;
        return true;
    }

    private static bool IsAllowedPaginationQuery(string query)
    {
        int apiVersions = 0;
        int continuationTokens = 0;
        foreach (string part in query
            .TrimStart('?')
            .Split('&', StringSplitOptions.RemoveEmptyEntries))
        {
            string[] pair = part.Split('=', 2);
            string name = Uri.UnescapeDataString(pair[0]);
            string value = pair.Length == 2 ? Uri.UnescapeDataString(pair[1]) : "";
            switch (name)
            {
                case "api-version":
                    if (!string.Equals(value, ApiVersionValue, StringComparison.Ordinal))
                    {
                        return false;
                    }
                    apiVersions++;
                    break;
                case "deploymentType":
                    if (!string.Equals(value, DeploymentTypeValue, StringComparison.Ordinal))
                    {
                        return false;
                    }
                    break;
                case "continuationToken":
                    if (value.Length is 0 or > 1024)
                    {
                        return false;
                    }
                    continuationTokens++;
                    break;
                default:
                    return false;
            }
        }
        return apiVersions == 1 && continuationTokens <= 1;
    }

    private static (FoundryDiscoveryOutcome Outcome, string ErrorCode) ClassifyStatus(
        HttpStatusCode statusCode) => statusCode switch
        {
            HttpStatusCode.Unauthorized or HttpStatusCode.Forbidden =>
                (FoundryDiscoveryOutcome.Authorization, "foundry_discovery_unauthorized"),
            HttpStatusCode.NotFound =>
                (FoundryDiscoveryOutcome.ProjectConfiguration, "foundry_discovery_project_not_found"),
            HttpStatusCode.RequestTimeout or HttpStatusCode.TooManyRequests =>
                (FoundryDiscoveryOutcome.Transient, "foundry_discovery_throttled"),
            >= HttpStatusCode.InternalServerError =>
                (FoundryDiscoveryOutcome.Transient, "foundry_discovery_provider_unavailable"),
            HttpStatusCode.BadRequest =>
                (FoundryDiscoveryOutcome.ProjectConfiguration, "foundry_discovery_request_rejected"),
            _ => (FoundryDiscoveryOutcome.InvalidBody, "foundry_discovery_status_unexpected"),
        };

    private static bool IsJsonResponse(MediaTypeHeaderValue? contentType) =>
        contentType is not null
        && string.Equals(contentType.MediaType, "application/json", StringComparison.Ordinal);

    /// <summary>
    /// Reads at most <paramref name="limit"/> bytes and returns null when the
    /// body would exceed the aggregate budget. The cap is applied to the
    /// decompressed stream, which is what makes bounded decompression real.
    /// </summary>
    private static async Task<byte[]?> ReadBoundedAsync(
        HttpContent content,
        long limit,
        CancellationToken cancellationToken)
    {
        if (limit <= 0)
        {
            return null;
        }
        await using Stream stream = await content
            .ReadAsStreamAsync(cancellationToken)
            .ConfigureAwait(false);
        using MemoryStream buffer = new();
        byte[] chunk = new byte[8_192];
        while (true)
        {
            int bytesRead = await stream
                .ReadAsync(chunk, cancellationToken)
                .ConfigureAwait(false);
            if (bytesRead == 0)
            {
                return buffer.ToArray();
            }
            if (buffer.Length + bytesRead > limit)
            {
                return null;
            }
            buffer.Write(chunk, 0, bytesRead);
        }
    }

    private static void CollectProviderRequestId(
        HttpResponseMessage response,
        List<string> providerRequestIds)
    {
        if (!response.Headers.TryGetValues(ProviderRequestIdHeader, out IEnumerable<string>? values))
        {
            return;
        }
        foreach (string value in values)
        {
            if (value.Length is > 0 and <= MaximumRequestIdLength
                && value.All(static character =>
                    char.IsAsciiLetterOrDigit(character) || character is '-' or '_'))
            {
                providerRequestIds.Add(value);
            }
        }
    }

    private static bool IsProjectEndpointShape(Uri endpoint)
    {
        if (!endpoint.IsAbsoluteUri
            || endpoint.Scheme != Uri.UriSchemeHttps
            || !endpoint.IsDefaultPort
            || endpoint.UserInfo.Length != 0
            || endpoint.Query.Length != 0
            || endpoint.Fragment.Length != 0
            || endpoint.AbsolutePath.Length <= ProjectPathPrefix.Length
            || !endpoint.AbsolutePath.StartsWith(ProjectPathPrefix, StringComparison.Ordinal)
            || !endpoint.AbsolutePath.EndsWith('/'))
        {
            return false;
        }
        string project = endpoint.AbsolutePath[ProjectPathPrefix.Length..^1];
        return project.Length is > 0 and <= 128
            && !project.Contains('/', StringComparison.Ordinal);
    }

    private static FoundryDiscoveryEvidence Evidence(
        DateTimeOffset retrievedAt,
        int pageCount,
        long totalBytes,
        List<string> clientRequestIds,
        List<string> providerRequestIds) =>
        new(
            retrievedAt,
            pageCount,
            totalBytes,
            [.. clientRequestIds],
            [.. providerRequestIds]);

    private static FoundryDiscoveryResult Failure(
        FoundryDiscoveryOutcome outcome,
        string errorCode,
        FoundryDiscoveryEvidence evidence) =>
        new(outcome, errorCode, Snapshot: null, evidence);
}

/// <summary>
/// The discovery seam the catalog cache depends on. Production binds the real
/// client; tests bind a scripted fake without a socket in sight.
/// </summary>
public interface IFoundryDeploymentDiscovery
{
    Task<FoundryDiscoveryResult> DiscoverAsync(CancellationToken cancellationToken);
}

public sealed class FoundryDeploymentDiscovery(FoundryDeploymentDiscoveryClient client)
    : IFoundryDeploymentDiscovery
{
    public Task<FoundryDiscoveryResult> DiscoverAsync(CancellationToken cancellationToken) =>
        client.DiscoverAsync(cancellationToken);
}
