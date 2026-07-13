using System.Buffers.Binary;
using System.IO.Pipes;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.Identity.Client;
using Microsoft.Identity.Client.Broker;

namespace Sapphirus.WindowsAuthBroker;

internal static class Program
{
    private const int MaximumMessageBytes = 64 * 1024;
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web)
    {
        PropertyNameCaseInsensitive = false,
        UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow,
    };

    public static async Task<int> Main(string[] args)
    {
        if (args.Length != 1 || !IsSafePipeName(args[0]))
        {
            return 2;
        }

        try
        {
            await using NamedPipeClientStream pipe = new(
                ".",
                args[0],
                PipeDirection.InOut,
                PipeOptions.Asynchronous | PipeOptions.CurrentUserOnly);
            using CancellationTokenSource timeout = new(TimeSpan.FromMinutes(5));
            await pipe.ConnectAsync(timeout.Token).ConfigureAwait(false);
            BrokerRequest request = await ReadMessageAsync<BrokerRequest>(pipe, timeout.Token)
                .ConfigureAwait(false);
            BrokerResponse response = await HandleAsync(request, timeout.Token).ConfigureAwait(false);
            await WriteMessageAsync(pipe, response, timeout.Token).ConfigureAwait(false);
            return response.Success ? 0 : 1;
        }
        catch (OperationCanceledException)
        {
            return 3;
        }
        catch (Exception)
        {
            // The host receives only the process disposition. Raw MSAL, pipe, and OS details are
            // intentionally excluded from stdout/stderr and from the protocol response.
            return 4;
        }
    }

    private static async Task<BrokerResponse> HandleAsync(
        BrokerRequest request,
        CancellationToken cancellationToken)
    {
        string requestId = IsSafeIdentifier(request.RequestId, 8, 128)
            ? request.RequestId
            : "invalid_request";
        if (request.ProtocolVersion != ProgramProtocol.Version
            || !IsSafeIdentifier(request.RequestId, 8, 128)
            || request.Operation is not ("acquire_token" or "sign_out")
            || !Guid.TryParseExact(request.ClientId, "D", out _)
            || !Uri.TryCreate(request.Authority, UriKind.Absolute, out Uri? authority)
            || !IsAllowedAuthority(authority)
            || request.Scopes is null
            || request.Scopes.Length is < 1 or > 16
            || request.Scopes.Any(scope => !IsAllowedScope(scope))
            || !IsSafeAccountId(request.AccountId)
            || (request.Operation == "acquire_token"
                && !TryParseWindowHandle(request.ParentWindowHandle, out _)))
        {
            return BrokerResponse.Fail(requestId, "request_invalid", false);
        }

        IPublicClientApplication application = PublicClientApplicationBuilder
            .Create(request.ClientId)
            .WithAuthority(authority.AbsoluteUri)
            .WithDefaultRedirectUri()
            .WithBroker(new BrokerOptions(BrokerOptions.OperatingSystems.Windows))
            .Build();

        return request.Operation switch
        {
            "acquire_token" => await AcquireTokenAsync(application, request, cancellationToken)
                .ConfigureAwait(false),
            "sign_out" => await SignOutAsync(application, request, cancellationToken)
                .ConfigureAwait(false),
            _ => BrokerResponse.Fail(request.RequestId, "operation_unsupported", false),
        };
    }

    private static async Task<BrokerResponse> AcquireTokenAsync(
        IPublicClientApplication application,
        BrokerRequest request,
        CancellationToken cancellationToken)
    {
        AuthenticationResult? result = null;
        IEnumerable<IAccount> accounts = await application.GetAccountsAsync().ConfigureAwait(false);
        IAccount? account = accounts.FirstOrDefault(candidate =>
            request.AccountId is null
            || string.Equals(
                candidate.HomeAccountId?.Identifier,
                request.AccountId,
                StringComparison.Ordinal));

        if (account is not null)
        {
            try
            {
                result = await application
                    .AcquireTokenSilent(request.Scopes, account)
                    .ExecuteAsync(cancellationToken)
                    .ConfigureAwait(false);
            }
            catch (MsalUiRequiredException)
            {
                result = null;
            }
        }

        if (result is null)
        {
            if (!TryParseWindowHandle(request.ParentWindowHandle, out nint parentWindow))
            {
                return BrokerResponse.Fail(request.RequestId, "request_invalid", false);
            }
            try
            {
                result = await application
                    .AcquireTokenInteractive(request.Scopes)
                    .WithParentActivityOrWindow(() => parentWindow)
                    .WithUseEmbeddedWebView(false)
                    .ExecuteAsync(cancellationToken)
                    .ConfigureAwait(false);
            }
            catch (MsalClientException error) when (
                request.AllowSystemBrowserFallback && IsBrokerUnavailable(error))
            {
                IPublicClientApplication browserApplication = PublicClientApplicationBuilder
                    .Create(request.ClientId)
                    .WithAuthority(request.Authority)
                    .WithDefaultRedirectUri()
                    .Build();
                result = await browserApplication
                    .AcquireTokenInteractive(request.Scopes)
                    .WithUseEmbeddedWebView(false)
                    .ExecuteAsync(cancellationToken)
                    .ConfigureAwait(false);
            }
            catch (MsalServiceException error)
            {
                return BrokerResponse.Fail(
                    request.RequestId,
                    ClassifyServiceFailure(error),
                    IsRetryable(error));
            }
            catch (MsalClientException error)
            {
                return BrokerResponse.Fail(
                    request.RequestId,
                    ClassifyClientFailure(error),
                    IsRetryable(error));
            }
        }

        return new BrokerResponse(
            ProgramProtocol.Version,
            request.RequestId,
            true,
            null,
            false,
            result.AccessToken,
            result.ExpiresOn.ToUniversalTime(),
            result.Account?.HomeAccountId?.Identifier,
            result.TenantId);
    }

    private static async Task<BrokerResponse> SignOutAsync(
        IPublicClientApplication application,
        BrokerRequest request,
        CancellationToken cancellationToken)
    {
        IEnumerable<IAccount> accounts = await application.GetAccountsAsync().ConfigureAwait(false);
        foreach (IAccount account in accounts)
        {
            cancellationToken.ThrowIfCancellationRequested();
            if (request.AccountId is null
                || string.Equals(
                    account.HomeAccountId?.Identifier,
                    request.AccountId,
                    StringComparison.Ordinal))
            {
                await application.RemoveAsync(account).ConfigureAwait(false);
            }
        }
        return new BrokerResponse(
            ProgramProtocol.Version,
            request.RequestId,
            true,
            null,
            false,
            null,
            null,
            null,
            null);
    }

    private static bool IsBrokerUnavailable(MsalClientException error) =>
        error.ErrorCode is "wam_runtime_init_failed" or "broker_not_supported";

    private static bool IsRetryable(MsalException error) =>
        error is MsalServiceException { IsRetryable: true }
        || error.ErrorCode is "temporarily_unavailable" or "request_timeout";

    private static string ClassifyServiceFailure(MsalServiceException error) =>
        error.ErrorCode switch
        {
            "access_denied" => "access_denied",
            "temporarily_unavailable" => "service_unavailable",
            "invalid_grant" => "reauthentication_required",
            _ => "identity_service_error",
        };

    private static string ClassifyClientFailure(MsalClientException error) =>
        error.ErrorCode switch
        {
            "authentication_canceled" => "authentication_cancelled",
            "user_canceled" => "authentication_cancelled",
            "wam_runtime_init_failed" => "broker_unavailable",
            "broker_not_supported" => "broker_unavailable",
            _ => "authentication_failed",
        };

    private static bool IsSafePipeName(string value) =>
        value.Length is >= 16 and <= 128
        && value.StartsWith("sapphirus-auth-", StringComparison.Ordinal)
        && value.All(character => char.IsAsciiLetterOrDigit(character) || character is '-' or '_');

    private static bool IsSafeIdentifier(string value, int minimum, int maximum) =>
        value is not null
        && value.Length >= minimum
        && value.Length <= maximum
        && value.All(character => char.IsAsciiLetterOrDigit(character) || character is '-' or '_');

    private static bool IsSafeAccountId(string? value) =>
        value is null
        || (value.Length is >= 3 and <= 256
            && value.All(character =>
                char.IsAsciiLetterOrDigit(character) || character is '-' or '_' or '.' or ':'));

    private static bool TryParseWindowHandle(string? value, out nint handle)
    {
        handle = 0;
        if (value is null
            || value.Length is < 3 or > 18
            || !value.StartsWith("0x", StringComparison.Ordinal)
            || !ulong.TryParse(
                value.AsSpan(2),
                System.Globalization.NumberStyles.AllowHexSpecifier,
                System.Globalization.CultureInfo.InvariantCulture,
                out ulong raw)
            || raw == 0
            || raw > long.MaxValue)
        {
            return false;
        }
        handle = checked((nint)(long)raw);
        return true;
    }

    private static bool IsAllowedAuthority(Uri authority)
    {
        if (authority.Scheme != Uri.UriSchemeHttps
            || !string.Equals(authority.Host, "login.microsoftonline.com", StringComparison.OrdinalIgnoreCase)
            || !authority.IsDefaultPort
            || !string.IsNullOrEmpty(authority.UserInfo)
            || !string.IsNullOrEmpty(authority.Query)
            || !string.IsNullOrEmpty(authority.Fragment))
        {
            return false;
        }
        string[] segments = authority.AbsolutePath.Trim('/').Split('/');
        return segments.Length == 2
            && segments[1] == "v2.0"
            && Guid.TryParseExact(segments[0], "D", out Guid tenantId)
            && tenantId != Guid.Empty;
    }

    private static bool IsAllowedScope(string scope) =>
        scope.Length is >= 3 and <= 256
        && !scope.Any(char.IsWhiteSpace)
        && Uri.TryCreate(scope, UriKind.Absolute, out Uri? scopeUri)
        && string.Equals(scopeUri.Scheme, "api", StringComparison.Ordinal);

    private static async Task<T> ReadMessageAsync<T>(Stream stream, CancellationToken cancellationToken)
    {
        byte[] lengthBytes = new byte[sizeof(int)];
        await stream.ReadExactlyAsync(lengthBytes, cancellationToken).ConfigureAwait(false);
        int length = BinaryPrimitives.ReadInt32BigEndian(lengthBytes);
        if (length is <= 0 or > MaximumMessageBytes)
        {
            throw new InvalidDataException("Invalid broker message length.");
        }
        byte[] payload = new byte[length];
        await stream.ReadExactlyAsync(payload, cancellationToken).ConfigureAwait(false);
        RejectDuplicateObjectKeys(payload);
        return JsonSerializer.Deserialize<T>(payload, JsonOptions)
            ?? throw new InvalidDataException("Invalid broker message.");
    }

    private static void RejectDuplicateObjectKeys(ReadOnlySpan<byte> payload)
    {
        Utf8JsonReader reader = new(payload, new JsonReaderOptions
        {
            AllowTrailingCommas = false,
            CommentHandling = JsonCommentHandling.Disallow,
            MaxDepth = 32,
        });
        Stack<HashSet<string>> objectProperties = new();
        while (reader.Read())
        {
            switch (reader.TokenType)
            {
                case JsonTokenType.StartObject:
                    objectProperties.Push(new HashSet<string>(StringComparer.Ordinal));
                    break;
                case JsonTokenType.EndObject:
                    if (objectProperties.Count == 0)
                    {
                        throw new InvalidDataException("Invalid broker JSON object nesting.");
                    }
                    objectProperties.Pop();
                    break;
                case JsonTokenType.PropertyName:
                    string propertyName = reader.GetString()
                        ?? throw new InvalidDataException("Invalid broker JSON property name.");
                    if (objectProperties.Count == 0 || !objectProperties.Peek().Add(propertyName))
                    {
                        throw new InvalidDataException("Duplicate broker JSON property name.");
                    }
                    break;
            }
        }
        if (objectProperties.Count != 0)
        {
            throw new InvalidDataException("Invalid broker JSON object nesting.");
        }
    }

    private static async Task WriteMessageAsync<T>(
        Stream stream,
        T value,
        CancellationToken cancellationToken)
    {
        byte[] payload = JsonSerializer.SerializeToUtf8Bytes(value, JsonOptions);
        if (payload.Length > MaximumMessageBytes)
        {
            throw new InvalidDataException("Broker response exceeded its limit.");
        }
        byte[] lengthBytes = new byte[sizeof(int)];
        BinaryPrimitives.WriteInt32BigEndian(lengthBytes, payload.Length);
        await stream.WriteAsync(lengthBytes, cancellationToken).ConfigureAwait(false);
        await stream.WriteAsync(payload, cancellationToken).ConfigureAwait(false);
        await stream.FlushAsync(cancellationToken).ConfigureAwait(false);
    }
}

internal sealed record BrokerRequest(
    string ProtocolVersion,
    string RequestId,
    string Operation,
    string ClientId,
    string Authority,
    string[] Scopes,
    string? AccountId,
    string? ParentWindowHandle,
    bool AllowSystemBrowserFallback);

internal sealed record BrokerResponse(
    string ProtocolVersion,
    string RequestId,
    bool Success,
    string? ErrorCode,
    bool Retryable,
    string? AccessToken,
    DateTimeOffset? ExpiresOn,
    string? AccountId,
    string? TenantId)
{
    public static BrokerResponse Fail(string requestId, string errorCode, bool retryable) =>
        new(
            ProgramProtocol.Version,
            requestId,
            false,
            errorCode,
            retryable,
            null,
            null,
            null,
            null);
}

internal static class ProgramProtocol
{
    public const string Version = "sapphirus.auth-broker.v1";
}
