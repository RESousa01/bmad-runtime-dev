using System.Net.Http.Headers;
using Azure.Core;

namespace Sapphirus.DesktopSupportApi.Foundry;

/// <summary>
/// The stable vocabulary for how a Foundry request was authenticated. These
/// two strings are the whole set.
///
/// The identifier exists so evidence can distinguish them. A receipt produced
/// under the development API key must never be mistakable for one produced
/// under managed identity, so FND-030 folds the mechanism identifier into the
/// credential-binding hash.
/// </summary>
public static class FoundryCredentialMechanisms
{
    /// <summary>
    /// Retained only so historical V1 receipts signed under managed identity
    /// still verify. No composition path builds
    /// <see cref="ManagedIdentityFoundryAuthenticator"/> for Foundry anymore;
    /// the production mechanism is <see cref="ApiKey"/>.
    /// </summary>
    public const string ManagedIdentity = "managed_identity";

    /// <summary>
    /// The production mechanism: a backend-only, secret-injected API key sent
    /// as an <c>api-key</c> header. No Entra token acquisition is involved.
    /// </summary>
    public const string ApiKey = "foundry_api_key";

    /// <summary>
    /// Development and Test only. Structurally unreachable elsewhere: the
    /// authenticator's constructor refuses to build outside those two
    /// environments.
    /// </summary>
    public const string DevelopmentApiKey = "development_api_key";
}

/// <summary>
/// The credential seam every Foundry call goes through.
///
/// It is deliberately provider-shaped rather than discovery-shaped: deployment
/// discovery and the later Responses adapter both authenticate the same way,
/// and neither of them knows which mechanism it is holding.
///
/// A caller authenticates one logical operation at a time.
/// <see cref="BeginOperationAsync"/> acquires whatever the mechanism needs
/// once, and the authenticator it returns applies that one acquisition to
/// every request of the operation. That is what keeps a paginated discovery at
/// exactly one token request rather than one per page.
/// </summary>
public interface IFoundryRequestAuthenticator
{
    /// <summary>
    /// Which mechanism this authenticator represents, from
    /// <see cref="FoundryCredentialMechanisms"/>. Never the credential itself.
    /// </summary>
    string CredentialMechanismId { get; }

    /// <summary>
    /// Applies the credential to one outbound request. Implementations set a
    /// header and never anything else; a request that could not be
    /// authenticated throws rather than being sent unauthenticated.
    /// </summary>
    ValueTask ApplyAsync(HttpRequestMessage request, CancellationToken cancellationToken);

    /// <summary>
    /// Acquires the credential for one logical operation and returns the
    /// authenticator that applies it. The result is pinned: it performs no
    /// further acquisition, so every request of the operation carries the same
    /// credential.
    /// </summary>
    ValueTask<IFoundryRequestAuthenticator> BeginOperationAsync(
        CancellationToken cancellationToken);
}

/// <summary>
/// The production mechanism. It acquires an Entra token for the Foundry scope
/// from a user-assigned managed identity, once per operation, and sends it as
/// <c>Authorization: Bearer</c>.
/// </summary>
public sealed class ManagedIdentityFoundryAuthenticator : IFoundryRequestAuthenticator
{
    /// <summary>The only scope the support plane ever requests for Foundry.</summary>
    public const string TokenScope = "https://ai.azure.com/.default";

    /// <summary>
    /// Raised when the identity produced no usable token. It carries a stable
    /// code and never the credential library's exception text, which routinely
    /// discloses tenant, client, and endpoint detail.
    /// </summary>
    public const string TokenUnavailableCode = "foundry_managed_identity_token_unavailable";

    private readonly TokenCredential _credential;

    public ManagedIdentityFoundryAuthenticator(TokenCredential credential)
    {
        ArgumentNullException.ThrowIfNull(credential);
        _credential = credential;
    }

    public string CredentialMechanismId => FoundryCredentialMechanisms.ManagedIdentity;

    public async ValueTask<IFoundryRequestAuthenticator> BeginOperationAsync(
        CancellationToken cancellationToken)
    {
        AccessToken accessToken = await _credential
            .GetTokenAsync(new TokenRequestContext([TokenScope]), cancellationToken)
            .ConfigureAwait(false);
        if (string.IsNullOrEmpty(accessToken.Token))
        {
            throw new InvalidOperationException(TokenUnavailableCode);
        }
        return new PinnedBearerToken(accessToken.Token);
    }

    public async ValueTask ApplyAsync(
        HttpRequestMessage request,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        IFoundryRequestAuthenticator operation = await BeginOperationAsync(cancellationToken)
            .ConfigureAwait(false);
        await operation.ApplyAsync(request, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>The mechanism name, never the credential.</summary>
    public override string ToString() => CredentialMechanismId;

    /// <summary>
    /// One acquired token, applied to every request of one operation. It holds
    /// no credential source, so it cannot acquire again.
    /// </summary>
    private sealed class PinnedBearerToken(string token) : IFoundryRequestAuthenticator
    {
        public string CredentialMechanismId =>
            FoundryCredentialMechanisms.ManagedIdentity;

        public ValueTask ApplyAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            ArgumentNullException.ThrowIfNull(request);
            cancellationToken.ThrowIfCancellationRequested();
            request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", token);
            return ValueTask.CompletedTask;
        }

        public ValueTask<IFoundryRequestAuthenticator> BeginOperationAsync(
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            return ValueTask.FromResult<IFoundryRequestAuthenticator>(this);
        }

        public override string ToString() => CredentialMechanismId;
    }
}

/// <summary>
/// The development-only mechanism. It sends the project API key in an
/// <c>api-key</c> request header.
///
/// It exists because the operator has neither Entra app-registration rights
/// nor Azure RBAC role-assignment rights, so the managed-identity path is
/// unobtainable rather than merely unsupplied. Only the credential mechanism
/// changes; discovery, normalization, the catalog, and everything downstream
/// stay exactly as designed.
///
/// The header is <c>api-key</c> and deliberately not
/// <c>Authorization: Bearer</c>: the measured result against the real project
/// used <c>api-key</c> on both the deployments surface and the Responses
/// surface.
///
/// The constructor is the fail-closed boundary. It mirrors the precedent set
/// by the test assembly's fake Foundry server: even an accidental reference
/// cannot activate this outside Development or Test.
/// </summary>
public sealed class DevelopmentFoundryApiKeyAuthenticator : IFoundryRequestAuthenticator
{
    /// <summary>The header the measured project accepted. Not <c>Authorization</c>.</summary>
    public const string ApiKeyHeaderName = "api-key";

    /// <summary>
    /// Configuration key for the development API key. It is supplied through
    /// the <c>Sapphirus__FoundryDevelopmentApiKey</c> environment variable and
    /// must never be committed with a real value.
    /// </summary>
    public const string ConfigurationKey = "Sapphirus:FoundryDevelopmentApiKey";

    /// <summary>The environment-variable spelling of <see cref="ConfigurationKey"/>.</summary>
    public const string EnvironmentVariableName = "Sapphirus__FoundryDevelopmentApiKey";

    public const string DevelopmentEnvironmentName = "Development";
    public const string TestEnvironmentName = "Test";

    /// <summary>
    /// Parallel to the fake Foundry server's
    /// <c>fake_foundry_environment_forbidden</c>.
    /// </summary>
    public const string EnvironmentForbiddenCode =
        "development_foundry_credential_environment_forbidden";

    /// <summary>
    /// A blank, over-long, or non-printable key fails closed here rather than
    /// producing an unauthenticated request later.
    /// </summary>
    public const string ApiKeyInvalidCode = "development_foundry_api_key_invalid";

    /// <summary>Raised when the transport refused to carry the header at all.</summary>
    public const string ApiKeyNotAppliedCode = "development_foundry_api_key_not_applied";

    /// <summary>
    /// A real project key is far longer than this. The bound only rejects
    /// obvious placeholder junk; the character rule below is the load-bearing
    /// one.
    /// </summary>
    internal const int MinimumApiKeyLength = 8;

    internal const int MaximumApiKeyLength = 512;

    private readonly string _apiKey;

    public DevelopmentFoundryApiKeyAuthenticator(string? environmentName, string? apiKey)
    {
        if (!IsPermittedEnvironment(environmentName))
        {
            throw new InvalidOperationException(EnvironmentForbiddenCode);
        }
        if (!IsWellFormedApiKey(apiKey))
        {
            // The message never quotes the supplied value.
            throw new InvalidOperationException(ApiKeyInvalidCode);
        }
        _apiKey = apiKey!;
    }

    public string CredentialMechanismId => FoundryCredentialMechanisms.DevelopmentApiKey;

    /// <summary>
    /// Development and Test, byte-for-byte, and nothing else. An unset, empty,
    /// Staging, or Production environment name is refused.
    /// </summary>
    public static bool IsPermittedEnvironment(string? environmentName) =>
        string.Equals(environmentName, DevelopmentEnvironmentName, StringComparison.Ordinal)
        || string.Equals(environmentName, TestEnvironmentName, StringComparison.Ordinal);

    /// <summary>
    /// Bounded, printable ASCII only. A key carrying a space, a control
    /// character, or a line break could otherwise split the request or make the
    /// header library throw an exception that quotes the key.
    /// </summary>
    public static bool IsWellFormedApiKey(string? apiKey)
    {
        if (apiKey is null
            || apiKey.Length is < MinimumApiKeyLength or > MaximumApiKeyLength)
        {
            return false;
        }
        foreach (char character in apiKey)
        {
            if (character is < '!' or > '~')
            {
                return false;
            }
        }
        return true;
    }

    public ValueTask ApplyAsync(
        HttpRequestMessage request,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        cancellationToken.ThrowIfCancellationRequested();
        if (!request.Headers.TryAddWithoutValidation(ApiKeyHeaderName, _apiKey))
        {
            // Fail closed. An unauthenticated request is never sent.
            throw new InvalidOperationException(ApiKeyNotAppliedCode);
        }
        return ValueTask.CompletedTask;
    }

    /// <summary>
    /// Nothing to acquire: the key was validated at construction. The
    /// operation authenticator is this instance.
    /// </summary>
    public ValueTask<IFoundryRequestAuthenticator> BeginOperationAsync(
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return ValueTask.FromResult<IFoundryRequestAuthenticator>(this);
    }

    /// <summary>The mechanism name, never the key.</summary>
    public override string ToString() => CredentialMechanismId;
}

/// <summary>
/// The production Foundry mechanism. It sends the backend-only, secret
/// injected project API key in an <c>api-key</c> header. This is the same
/// header shape measured against the real project (200 on
/// <c>GET &lt;projectEndpoint&gt;/deployments</c>, auth-passed 400 on an empty
/// <c>POST &lt;projectEndpoint&gt;/openai/v1/responses</c> body); the only
/// difference from <see cref="DevelopmentFoundryApiKeyAuthenticator"/> is that
/// this mechanism is not environment-gated, because it is the one Production
/// composes.
///
/// The key is validated once at construction (shape only, never logged), and
/// rotation is handled by building a new application revision with a new
/// instance rather than mutating a live one mid-request.
/// </summary>
public sealed class FoundryApiKeyAuthenticator : IFoundryRequestAuthenticator
{
    /// <summary>The header the measured project accepted. Not <c>Authorization</c>.</summary>
    public const string ApiKeyHeaderName = DevelopmentFoundryApiKeyAuthenticator.ApiKeyHeaderName;

    /// <summary>
    /// A blank, over-long, or non-printable key fails closed here rather than
    /// producing an unauthenticated request later.
    /// </summary>
    public const string ApiKeyInvalidCode = "foundry_api_key_invalid";

    /// <summary>Raised when the transport refused to carry the header at all.</summary>
    public const string ApiKeyNotAppliedCode = "foundry_api_key_not_applied";

    private readonly string _apiKey;

    public FoundryApiKeyAuthenticator(string? apiKey)
    {
        if (!DevelopmentFoundryApiKeyAuthenticator.IsWellFormedApiKey(apiKey))
        {
            // The message never quotes the supplied value.
            throw new InvalidOperationException(ApiKeyInvalidCode);
        }
        _apiKey = apiKey!;
    }

    public string CredentialMechanismId => FoundryCredentialMechanisms.ApiKey;

    public ValueTask ApplyAsync(
        HttpRequestMessage request,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        cancellationToken.ThrowIfCancellationRequested();
        if (!request.Headers.TryAddWithoutValidation(ApiKeyHeaderName, _apiKey))
        {
            // Fail closed. An unauthenticated request is never sent.
            throw new InvalidOperationException(ApiKeyNotAppliedCode);
        }
        return ValueTask.CompletedTask;
    }

    /// <summary>
    /// Nothing to acquire: the key was validated at construction. The
    /// operation authenticator is this instance.
    /// </summary>
    public ValueTask<IFoundryRequestAuthenticator> BeginOperationAsync(
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return ValueTask.FromResult<IFoundryRequestAuthenticator>(this);
    }

    /// <summary>The mechanism name, never the key.</summary>
    public override string ToString() => CredentialMechanismId;
}
