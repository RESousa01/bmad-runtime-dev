using System.Net;
using System.Net.Http.Headers;
using Sapphirus.DesktopSupportApi.Foundry;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// One open streaming Responses call. The body is handed to the normalizer
/// unread; nothing else is exposed, and disposing the session tears the
/// provider connection down.
/// </summary>
public sealed class FoundryStreamSession(
    HttpResponseMessage response,
    Stream body) : IAsyncDisposable
{
    /// <summary>The raw server-sent event body. Never surfaced to a client.</summary>
    public Stream Body { get; } = body;

    /// <summary>The provider's correlation identifier, when it sent a safe one.</summary>
    public string? ProviderRequestId { get; } =
        FoundryResponsesProviderExecutor.ReadProviderRequestId(response);

    public async ValueTask DisposeAsync()
    {
        await Body.DisposeAsync().ConfigureAwait(false);
        response.Dispose();
    }
}

/// <summary>Opens one streaming Responses call.</summary>
public interface IModelStreamTransport
{
    Task<FoundryStreamSession> OpenAsync(
        ModelProviderRequest request,
        CancellationToken cancellationToken);
}

/// <summary>
/// The Foundry Responses adapter: <c>POST &lt;projectEndpoint&gt;/openai/v1/responses</c>.
///
/// <para>
/// It is a bounded <see cref="HttpClient"/> rather than the official SDK's
/// responses client, for three reasons. The credential must go through
/// <see cref="IFoundryRequestAuthenticator"/>, which is what lets the same code
/// path carry managed identity in Production and the development API key
/// locally without either of them being visible here; an SDK client owns its
/// own credential pipeline and would either bypass that seam or force the key
/// back into this layer. The pinned <c>Azure.AI.OpenAI</c>/<c>OpenAI</c> 2.1.0
/// pair has no responses client at all, and the version that does ships it
/// behind an experimental diagnostic this repository's
/// <c>TreatWarningsAsErrors</c> turns into a build failure. And the offline
/// evidence for this task is generated against the raw HTTP and SSE shapes the
/// fake Foundry server models, which is the wire this speaks directly.
/// </para>
/// <para>
/// Every failure becomes a stable outcome from
/// <see cref="ModelProviderOutcomes"/>. No response body, provider message,
/// endpoint, deployment name, or credential ever leaves this type.
/// </para>
/// </summary>
public sealed class FoundryResponsesProviderExecutor
    : IModelProviderExecutor, IModelStreamTransport
{
    /// <summary>
    /// The named transport. It is separate from the discovery client because a
    /// model call and an inventory read have different timeouts and different
    /// acceptable response sizes.
    /// </summary>
    public const string HttpClientName = "foundry-responses";

    /// <summary>Relative to the derived <c>/openai/v1/</c> base address.</summary>
    public const string ResponsesPath = "responses";

    public const string ClientRequestIdHeader = "x-ms-client-request-id";
    public const string ProviderRequestIdHeader = "x-ms-request-id";

    /// <summary>A structured answer larger than this is a context failure.</summary>
    public const long MaximumResponseBytes = 4L * 1024 * 1024;

    private const int MaximumProviderRequestIdLength = 128;

    private readonly HttpClient _httpClient;
    private readonly IFoundryRequestAuthenticator _authenticator;

    public FoundryResponsesProviderExecutor(
        HttpClient httpClient,
        IFoundryRequestAuthenticator authenticator)
    {
        ArgumentNullException.ThrowIfNull(httpClient);
        ArgumentNullException.ThrowIfNull(authenticator);
        if (httpClient.BaseAddress is null
            || !httpClient.BaseAddress.IsAbsoluteUri
            || httpClient.BaseAddress.Scheme != Uri.UriSchemeHttps
            || !httpClient.BaseAddress.AbsolutePath.EndsWith('/'))
        {
            throw new ArgumentException(
                "foundry_responses_base_address_invalid",
                nameof(httpClient));
        }
        _httpClient = httpClient;
        _authenticator = authenticator;
    }

    /// <summary>
    /// How outbound Responses calls are authenticated. The mechanism
    /// identifier, never the credential; it is folded into the receipt's
    /// credential-binding hash so a development-key receipt can never be
    /// mistaken for a managed-identity one.
    /// </summary>
    public string CredentialMechanismId => _authenticator.CredentialMechanismId;

    /// <summary>
    /// The non-secret credential version label the current authenticator
    /// carries. Folded into the receipt's credential-binding hash alongside
    /// <see cref="CredentialMechanismId"/>; never the credential itself.
    /// </summary>
    public string CredentialVersionId => _authenticator.CredentialVersionId;

    /// <summary>Executes one structured, non-streaming Responses call.</summary>
    public async Task<ModelProviderResponse> ExecuteAsync(
        ModelProviderRequest request,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        if (request.IsStreaming)
        {
            // A streaming request on the structured seam would silently
            // discard every delta.
            throw new ModelProviderException(
                ModelProviderOutcomes.CapabilityUnsupported,
                retryable: false);
        }

        byte[] body = FoundryResponseRequestBuilder.Build(request);
        using HttpRequestMessage message = CreateRequest(body, "application/json");
        await ApplyCredentialAsync(message, cancellationToken).ConfigureAwait(false);

        using HttpResponseMessage response = await SendAsync(
                message,
                HttpCompletionOption.ResponseContentRead,
                cancellationToken)
            .ConfigureAwait(false);

        byte[]? payload = await ReadBoundedAsync(
                response.Content,
                MaximumResponseBytes,
                cancellationToken)
            .ConfigureAwait(false);
        if (payload is null)
        {
            throw new ModelProviderException(
                ModelProviderOutcomes.ContextLimit,
                retryable: false);
        }
        if (!response.IsSuccessStatusCode)
        {
            FoundryResponseClassification classification =
                FoundryResponseParser.ClassifyStatus(response.StatusCode, payload);
            throw new ModelProviderException(
                classification.Outcome,
                classification.Retryable);
        }
        if (!IsContentType(response, "application/json"))
        {
            throw new ModelProviderException(
                ModelProviderOutcomes.MalformedOutput,
                retryable: false);
        }

        ModelProviderResponse parsed = FoundryResponseParser.Parse(payload);
        return parsed.ProviderRequestId is null
            ? parsed with { ProviderRequestId = ReadProviderRequestId(response) }
            : parsed;
    }

    /// <summary>
    /// Opens one streaming Responses call. The caller owns the returned
    /// session and must dispose it; disposing before the stream ends is how a
    /// desktop disconnect reaches the provider.
    /// </summary>
    public async Task<FoundryStreamSession> OpenAsync(
        ModelProviderRequest request,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        if (!request.IsStreaming)
        {
            throw new ModelProviderException(
                ModelProviderOutcomes.CapabilityUnsupported,
                retryable: false);
        }

        byte[] body = FoundryResponseRequestBuilder.Build(request);
        using HttpRequestMessage message = CreateRequest(body, "text/event-stream");
        await ApplyCredentialAsync(message, cancellationToken).ConfigureAwait(false);

        HttpResponseMessage response = await SendAsync(
                message,
                HttpCompletionOption.ResponseHeadersRead,
                cancellationToken)
            .ConfigureAwait(false);

        try
        {
            if (!response.IsSuccessStatusCode)
            {
                byte[] payload = await ReadBoundedAsync(
                        response.Content,
                        64 * 1024,
                        cancellationToken)
                    .ConfigureAwait(false) ?? [];
                FoundryResponseClassification classification =
                    FoundryResponseParser.ClassifyStatus(response.StatusCode, payload);
                throw new ModelProviderException(
                    classification.Outcome,
                    classification.Retryable);
            }
            if (!IsContentType(response, "text/event-stream"))
            {
                // A streaming request answered with a buffered document means
                // the transcript the plane is about to normalize is not the
                // transcript it asked for.
                throw new ModelProviderException(
                    ModelProviderOutcomes.StreamInvalid,
                    retryable: false);
            }
            Stream stream = await response.Content
                .ReadAsStreamAsync(cancellationToken)
                .ConfigureAwait(false);
            return new FoundryStreamSession(response, stream);
        }
        catch
        {
            response.Dispose();
            throw;
        }
    }

    private HttpRequestMessage CreateRequest(byte[] body, string accept)
    {
        HttpRequestMessage message = new(HttpMethod.Post, ResponsesPath)
        {
            Content = new ByteArrayContent(body),
        };
        message.Content.Headers.ContentType =
            new MediaTypeHeaderValue("application/json") { CharSet = "utf-8" };
        message.Headers.Accept.Clear();
        message.Headers.Accept.ParseAdd(accept);
        message.Headers.TryAddWithoutValidation(
            ClientRequestIdHeader,
            Guid.NewGuid().ToString("D"));
        return message;
    }

    private async Task ApplyCredentialAsync(
        HttpRequestMessage message,
        CancellationToken cancellationToken)
    {
        try
        {
            IFoundryRequestAuthenticator operation = await _authenticator
                .BeginOperationAsync(cancellationToken)
                .ConfigureAwait(false);
            await operation.ApplyAsync(message, cancellationToken).ConfigureAwait(false);
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception)
        {
            // Credential library exception text routinely carries tenant,
            // client, and endpoint detail, so none of it escapes. An
            // unauthenticated request is never sent.
            throw new ModelProviderException(
                ModelProviderOutcomes.AuthenticationFailed,
                retryable: false);
        }
    }

    private async Task<HttpResponseMessage> SendAsync(
        HttpRequestMessage message,
        HttpCompletionOption completionOption,
        CancellationToken cancellationToken)
    {
        try
        {
            return await _httpClient
                .SendAsync(message, completionOption, cancellationToken)
                .ConfigureAwait(false);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (OperationCanceledException)
        {
            // The bounded client timeout elapsed rather than the caller
            // cancelling.
            throw new ModelProviderException(ModelProviderOutcomes.Timeout, retryable: true);
        }
        catch (HttpRequestException)
        {
            throw new ModelProviderException(
                ModelProviderOutcomes.ProviderUnavailable,
                retryable: true);
        }
    }

    private static bool IsContentType(HttpResponseMessage response, string mediaType) =>
        response.Content.Headers.ContentType is { } contentType
        && string.Equals(contentType.MediaType, mediaType, StringComparison.Ordinal);

    /// <summary>
    /// Reads at most <paramref name="limit"/> bytes, returning null when the
    /// body would exceed it. The cap applies to the decompressed stream.
    /// </summary>
    private static async Task<byte[]?> ReadBoundedAsync(
        HttpContent content,
        long limit,
        CancellationToken cancellationToken)
    {
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

    /// <summary>
    /// Accepts a provider correlation header only when it is short and opaque.
    /// </summary>
    internal static string? ReadProviderRequestId(HttpResponseMessage response)
    {
        if (!response.Headers.TryGetValues(
            ProviderRequestIdHeader,
            out IEnumerable<string>? values))
        {
            return null;
        }
        foreach (string value in values)
        {
            if (value.Length is > 0 and <= MaximumProviderRequestIdLength
                && value.All(static character =>
                    char.IsAsciiLetterOrDigit(character) || character is '-' or '_'))
            {
                return value;
            }
        }
        return null;
    }
}

/// <summary>
/// Registers the Responses transport. It lives beside the executor rather than
/// in the Azure client registration because it is a Foundry project transport,
/// not an Azure SDK client, and because the discovery transport's ten-second
/// header budget is far too short for a model call.
/// </summary>
public static class FoundryResponsesTransport
{
    /// <summary>
    /// Header budget for a model call. It bounds time-to-first-byte, not the
    /// length of a stream: a streaming read completes its headers immediately
    /// and then runs under the caller's own deadline.
    /// </summary>
    public static readonly TimeSpan HeaderTimeout = TimeSpan.FromSeconds(120);

    /// <summary>A fixed product marker; no OS, machine, or SDK detail.</summary>
    internal const string RedactedUserAgent = "sapphirus-support-plane/1";

    public static IServiceCollection AddFoundryResponsesHttpClient(
        this IServiceCollection services,
        Uri responsesEndpoint)
    {
        ArgumentNullException.ThrowIfNull(services);
        ArgumentNullException.ThrowIfNull(responsesEndpoint);
        services
            .AddHttpClient(FoundryResponsesProviderExecutor.HttpClientName, client =>
            {
                client.BaseAddress = responsesEndpoint;
                client.Timeout = HeaderTimeout;
                client.DefaultRequestHeaders.UserAgent.Clear();
                client.DefaultRequestHeaders.UserAgent.ParseAdd(RedactedUserAgent);
                client.DefaultRequestHeaders.ExpectContinue = false;
            })
            .ConfigurePrimaryHttpMessageHandler(static () => new SocketsHttpHandler
            {
                // A redirect is the cheapest way to move a credential to
                // another origin, so the transport refuses to follow one.
                AllowAutoRedirect = false,
                AutomaticDecompression =
                    DecompressionMethods.GZip | DecompressionMethods.Deflate,
                MaxResponseHeadersLength = 16,
                ConnectTimeout = TimeSpan.FromSeconds(10),
                UseCookies = false,
                UseProxy = false,
                PooledConnectionLifetime = TimeSpan.FromMinutes(5),
            });
        return services;
    }
}
