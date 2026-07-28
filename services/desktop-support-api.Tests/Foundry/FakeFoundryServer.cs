using System.Net;
using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Foundry;

public sealed class FakeFoundryServerTests
{
    private const string Marker = "forge-foundry-fake-marker-v1";
    private static readonly Uri DeploymentUri = new(
        "https://forge-fake.test/api/projects/forge-test/deployments" +
        "?api-version=v1&deploymentType=ModelDeployment");
    private static readonly Uri ResponsesUri = new(
        "https://forge-fake.test/api/projects/forge-test/openai/v1/responses");

    [Theory]
    [InlineData("")]
    [InlineData("Staging")]
    [InlineData("Production")]
    public void FakeFoundry_rejects_every_non_test_environment(string environmentName)
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);

        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => new FakeFoundryServer(environmentName, clock));

        Assert.Equal("fake_foundry_environment_forbidden", error.Message);
    }

    [Theory]
    [InlineData("Development")]
    [InlineData("Test")]
    public void FakeFoundry_allows_only_development_and_test(string environmentName)
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);

        using FakeFoundryServer server = new(environmentName, clock);

        Assert.Equal(0, server.PendingResponseCount);
    }

    [Fact]
    public async Task FakeFoundry_requires_the_exact_synthetic_bearer_marker()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = new(server, disposeHandler: false);
        server.Enqueue(FakeFoundryReply.Json(HttpStatusCode.OK, """{"value":[]}"""));

        using HttpResponseMessage missing = await client.SendAsync(
            new HttpRequestMessage(HttpMethod.Get, DeploymentUri),
            TestContext.Current.CancellationToken);
        using HttpRequestMessage wrongRequest = new(HttpMethod.Get, DeploymentUri);
        wrongRequest.Headers.Authorization =
            new AuthenticationHeaderValue("Bearer", "wrong-marker");
        using HttpResponseMessage wrong = await client.SendAsync(
            wrongRequest,
            TestContext.Current.CancellationToken);
        using HttpResponseMessage accepted = await client.SendAsync(
            CreateAuthorizedRequest(HttpMethod.Get, DeploymentUri),
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.Unauthorized, missing.StatusCode);
        Assert.Equal(HttpStatusCode.Unauthorized, wrong.StatusCode);
        Assert.Equal(HttpStatusCode.OK, accepted.StatusCode);
        Assert.Equal(0, server.PendingResponseCount);
    }

    [Fact]
    public async Task FakeFoundry_exposes_only_the_two_project_routes()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = new(server, disposeHandler: false);
        server.Enqueue(FakeFoundryReply.Json(HttpStatusCode.OK, """{"ok":true}"""));
        string queryName = string.Concat("api", "-key");

        using HttpResponseMessage response = await client.SendAsync(
            CreateAuthorizedRequest(
                HttpMethod.Get,
                new Uri(
                    $"https://forge-fake.test/healthz?{queryName}" +
                    "=query-value-must-not-leak")),
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.NotFound, response.StatusCode);
        Assert.Equal(1, server.PendingResponseCount);
        FakeFoundryRequestReceipt receipt = Assert.Single(
            server.RequestReceipts);
        Assert.Equal("unrecognized", receipt.Route);
        Assert.Matches(
            "^sha256:[0-9a-f]{64}$",
            receipt.RequestTargetSha256);
        Assert.DoesNotContain(
            "query-value-must-not-leak",
            JsonSerializer.Serialize(receipt),
            StringComparison.Ordinal);
    }

    [Fact]
    public async Task FakeFoundry_rejects_cross_origin_pagination_without_consuming_reply()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = new(server, disposeHandler: false);
        server.Enqueue(FakeFoundryReply.Json(HttpStatusCode.OK, """{"value":[]}"""));
        using HttpRequestMessage request = CreateAuthorizedRequest(
            HttpMethod.Get,
            new Uri(
                "https://attacker.invalid/api/projects/forge-test/deployments" +
                "?api-version=v1&deploymentType=ModelDeployment" +
                "&continuationToken=stolen"));

        using HttpResponseMessage response = await client.SendAsync(
            request,
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.NotFound, response.StatusCode);
        Assert.Equal(1, server.PendingResponseCount);
        Assert.Equal(
            "unrecognized",
            Assert.Single(server.RequestReceipts).Route);
    }

    [Fact]
    public async Task FakeFoundry_records_only_body_hash_and_header_names()
    {
        FakeFoundryTestClock clock = new(
            DateTimeOffset.Parse("2026-07-28T12:00:00Z"));
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = new(server, disposeHandler: false);
        server.Enqueue(
            FakeFoundryReply.Json(
                HttpStatusCode.OK,
                """{"id":"resp_fixture","status":"completed"}"""));
        const string requestBody =
            """{"model":"desktop-planning","input":"private fixture content"}""";
        using HttpRequestMessage request =
            CreateAuthorizedRequest(HttpMethod.Post, ResponsesUri);
        request.Headers.Add("X-Secret-Marker", "header-value-must-not-leak");
        request.Content = new StringContent(
            requestBody,
            Encoding.UTF8,
            "application/json");

        using HttpResponseMessage response = await client.SendAsync(
            request,
            TestContext.Current.CancellationToken);

        FakeFoundryRequestReceipt receipt = Assert.Single(server.RequestReceipts);
        string expectedHash = Convert.ToHexString(
            SHA256.HashData(Encoding.UTF8.GetBytes(requestBody)))
            .ToLowerInvariant();
        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        Assert.Equal($"sha256:{expectedHash}", receipt.BodySha256);
        Assert.Equal(
            ["Authorization", "Content-Type", "X-Secret-Marker"],
            receipt.HeaderNames);
        Assert.Equal(DateTimeOffset.Parse("2026-07-28T12:00:00Z"), receipt.ReceivedAt);
        Assert.False(receipt.BodyTruncated);
        string serialized = JsonSerializer.Serialize(receipt);
        Assert.DoesNotContain(Marker, serialized, StringComparison.Ordinal);
        Assert.DoesNotContain(
            "private fixture content",
            serialized,
            StringComparison.Ordinal);
        Assert.DoesNotContain(
            "header-value-must-not-leak",
            serialized,
            StringComparison.Ordinal);
    }

    [Fact]
    public async Task FakeFoundry_uses_a_scripted_queue_and_test_clock()
    {
        FakeFoundryTestClock clock = new(
            DateTimeOffset.Parse("2026-07-28T12:00:00Z"));
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = new(server, disposeHandler: false);
        server.Enqueue(
            FakeFoundryReply.Json(
                HttpStatusCode.TooManyRequests,
                """{"error":{"code":"rate_limit_exceeded"}}"""));
        server.Enqueue(
            FakeFoundryReply.Json(
                HttpStatusCode.OK,
                """{"id":"resp_after_retry","status":"completed"}"""));

        using HttpResponseMessage first = await client.SendAsync(
            CreateAuthorizedRequest(HttpMethod.Post, ResponsesUri),
            TestContext.Current.CancellationToken);
        clock.Advance(TimeSpan.FromSeconds(5));
        using HttpResponseMessage second = await client.SendAsync(
            CreateAuthorizedRequest(HttpMethod.Post, ResponsesUri),
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.TooManyRequests, first.StatusCode);
        Assert.Equal(HttpStatusCode.OK, second.StatusCode);
        Assert.Equal(0, server.PendingResponseCount);
        Assert.Collection(
            server.RequestReceipts,
            receipt =>
            {
                Assert.Equal(1, receipt.Sequence);
                Assert.Equal(
                    DateTimeOffset.Parse("2026-07-28T12:00:00Z"),
                    receipt.ReceivedAt);
            },
            receipt =>
            {
                Assert.Equal(2, receipt.Sequence);
                Assert.Equal(
                    DateTimeOffset.Parse("2026-07-28T12:00:05Z"),
                    receipt.ReceivedAt);
            });
    }

    [Fact]
    public async Task FakeFoundry_waiting_reply_observes_client_cancellation()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = new(server, disposeHandler: false);
        server.Enqueue(FakeFoundryReply.WaitForCancellation());
        using CancellationTokenSource cancellation = new(
            TimeSpan.FromMilliseconds(50));

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
            client.SendAsync(
                CreateAuthorizedRequest(HttpMethod.Post, ResponsesUri),
                cancellation.Token));

        Assert.Equal(0, server.PendingResponseCount);
        Assert.Single(server.RequestReceipts);
    }

    [Fact]
    public async Task FakeFoundry_rejects_oversized_requests_without_consuming_reply()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        using HttpClient client = new(server, disposeHandler: false);
        server.Enqueue(
            FakeFoundryReply.Json(
                HttpStatusCode.OK,
                """{"id":"must_remain_queued"}"""));
        using HttpRequestMessage request =
            CreateAuthorizedRequest(HttpMethod.Post, ResponsesUri);
        request.Content = new StringContent(
            new string('x', FakeFoundryServer.MaximumRequestBodyBytes + 1),
            Encoding.UTF8,
            "text/plain");

        using HttpResponseMessage response = await client.SendAsync(
            request,
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.RequestEntityTooLarge, response.StatusCode);
        Assert.Equal(1, server.PendingResponseCount);
        Assert.True(Assert.Single(server.RequestReceipts).BodyTruncated);
    }

    [Fact]
    public void FakeFoundry_rejects_unbounded_scripted_content()
    {
        FakeFoundryTestClock clock = new(DateTimeOffset.UnixEpoch);
        using FakeFoundryServer server = new("Test", clock);
        string oversized = new('x', FakeFoundryServer.MaximumFixtureBodyBytes + 1);

        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => server.Enqueue(
                FakeFoundryReply.Json(HttpStatusCode.OK, oversized)));

        Assert.Equal("fake_foundry_fixture_body_too_large", error.Message);
    }

    private static HttpRequestMessage CreateAuthorizedRequest(
        HttpMethod method,
        Uri uri)
    {
        HttpRequestMessage request = new(method, uri);
        request.Headers.Authorization =
            new AuthenticationHeaderValue("Bearer", Marker);
        return request;
    }
}

internal sealed class FakeFoundryTestClock
{
    private readonly object gate = new();
    private DateTimeOffset now;

    public FakeFoundryTestClock(DateTimeOffset initialValue)
    {
        now = initialValue;
    }

    public DateTimeOffset UtcNow
    {
        get
        {
            lock (gate)
            {
                return now;
            }
        }
    }

    public void Advance(TimeSpan duration)
    {
        if (duration < TimeSpan.Zero)
        {
            throw new ArgumentOutOfRangeException(
                nameof(duration),
                "fake_foundry_clock_advance_invalid");
        }
        lock (gate)
        {
            now = now.Add(duration);
        }
    }
}

internal sealed record FakeFoundryRequestReceipt(
    int Sequence,
    DateTimeOffset ReceivedAt,
    string Method,
    string Route,
    string RequestTargetSha256,
    string BodySha256,
    bool BodyTruncated,
    IReadOnlyList<string> HeaderNames);

internal sealed record FakeFoundryReply(
    HttpStatusCode StatusCode,
    string ContentType,
    string? Body,
    bool WaitsForCancellation)
{
    public static FakeFoundryReply Json(HttpStatusCode statusCode, string body) =>
        new(statusCode, "application/json", body, WaitsForCancellation: false);

    public static FakeFoundryReply EventStream(string body) =>
        new(
            HttpStatusCode.OK,
            "text/event-stream",
            body,
            WaitsForCancellation: false);

    public static FakeFoundryReply WaitForCancellation() =>
        new(
            HttpStatusCode.OK,
            "application/json",
            Body: null,
            WaitsForCancellation: true);
}

/// <summary>
/// In-memory Foundry surface for the test assembly only. The constructor is a
/// second fail-closed boundary: even an accidental reference cannot activate
/// it outside Development or Test.
/// </summary>
internal sealed class FakeFoundryServer : HttpMessageHandler
{
    public const int MaximumFixtureBodyBytes = 1_048_577;
    public const int MaximumRequestBodyBytes = 65_536;
    public const int MaximumQueueDepth = 64;
    private const string SyntheticBearerMarker =
        "forge-foundry-fake-marker-v1";
    private const string ExpectedFakeHost = "forge-fake.test";
    private readonly object gate = new();
    private readonly FakeFoundryTestClock clock;
    private readonly Queue<FakeFoundryReply> replies = new();
    private readonly List<FakeFoundryRequestReceipt> receipts = [];

    public FakeFoundryServer(
        string environmentName,
        FakeFoundryTestClock clock)
    {
        if (
            !string.Equals(
                environmentName,
                "Development",
                StringComparison.Ordinal) &&
            !string.Equals(environmentName, "Test", StringComparison.Ordinal)
        )
        {
            throw new InvalidOperationException(
                "fake_foundry_environment_forbidden");
        }
        this.clock = clock ?? throw new ArgumentNullException(nameof(clock));
    }

    public int PendingResponseCount
    {
        get
        {
            lock (gate)
            {
                return replies.Count;
            }
        }
    }

    public IReadOnlyList<FakeFoundryRequestReceipt> RequestReceipts
    {
        get
        {
            lock (gate)
            {
                return receipts.ToArray();
            }
        }
    }

    public void Enqueue(FakeFoundryReply reply)
    {
        ArgumentNullException.ThrowIfNull(reply);
        if (
            reply.WaitsForCancellation &&
            reply.Body is not null
        )
        {
            throw new InvalidOperationException(
                "fake_foundry_waiting_fixture_has_body");
        }
        if (
            !reply.WaitsForCancellation &&
            (
                reply.Body is null ||
                Encoding.UTF8.GetByteCount(reply.Body) >
                    MaximumFixtureBodyBytes
            )
        )
        {
            throw new InvalidOperationException(
                "fake_foundry_fixture_body_too_large");
        }
        if (
            reply.ContentType is not "application/json" and
                not "text/event-stream"
        )
        {
            throw new InvalidOperationException(
                "fake_foundry_fixture_content_type_invalid");
        }
        lock (gate)
        {
            if (replies.Count >= MaximumQueueDepth)
            {
                throw new InvalidOperationException(
                    "fake_foundry_queue_limit_exceeded");
            }
            replies.Enqueue(reply);
        }
    }

    protected override async Task<HttpResponseMessage> SendAsync(
        HttpRequestMessage request,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        RequestEvidence evidence = await ReadEvidenceAsync(
            request.Content,
            cancellationToken);
        string[] headerNames = request.Headers
            .Select(header => header.Key)
            .Concat(
                request.Content?.Headers.Select(header => header.Key) ??
                [])
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .Order(StringComparer.Ordinal)
            .ToArray();
        string route = ClassifyRoute(request);
        string requestTarget = request.RequestUri?.PathAndQuery ?? string.Empty;
        string requestTargetSha256 =
            $"sha256:{Convert.ToHexString(SHA256.HashData(
                Encoding.UTF8.GetBytes(requestTarget))).ToLowerInvariant()}";
        FakeFoundryRequestReceipt receipt;
        lock (gate)
        {
            receipt = new FakeFoundryRequestReceipt(
                receipts.Count + 1,
                clock.UtcNow,
                request.Method.Method,
                route,
                requestTargetSha256,
                evidence.BodySha256,
                evidence.BodyTruncated,
                headerNames);
            receipts.Add(receipt);
        }

        if (evidence.BodyTruncated)
        {
            return CreateError(
                HttpStatusCode.RequestEntityTooLarge,
                "fake_foundry_request_body_too_large",
                request);
        }
        if (!IsAuthorized(request.Headers.Authorization))
        {
            return CreateError(
                HttpStatusCode.Unauthorized,
                "fake_foundry_unauthorized",
                request);
        }
        if (route == "unrecognized")
        {
            return CreateError(
                HttpStatusCode.NotFound,
                "fake_foundry_route_not_found",
                request);
        }

        FakeFoundryReply reply;
        lock (gate)
        {
            if (!replies.TryDequeue(out reply!))
            {
                return CreateError(
                    HttpStatusCode.Conflict,
                    "fake_foundry_queue_empty",
                    request);
            }
        }
        if (reply.WaitsForCancellation)
        {
            await Task.Delay(Timeout.InfiniteTimeSpan, cancellationToken);
            throw new InvalidOperationException(
                "fake_foundry_waiting_reply_completed");
        }

        HttpResponseMessage response = new(reply.StatusCode)
        {
            Content = new StringContent(
                reply.Body!,
                Encoding.UTF8,
                reply.ContentType),
            RequestMessage = request,
        };
        return response;
    }

    private static async Task<RequestEvidence> ReadEvidenceAsync(
        HttpContent? content,
        CancellationToken cancellationToken)
    {
        using IncrementalHash hash = IncrementalHash.CreateHash(
            HashAlgorithmName.SHA256);
        if (content is null)
        {
            return new RequestEvidence(
                $"sha256:{Convert.ToHexString(hash.GetHashAndReset()).ToLowerInvariant()}",
                BodyTruncated: false);
        }

        await using Stream stream = await content.ReadAsStreamAsync(
            cancellationToken);
        byte[] buffer = new byte[8_192];
        int totalBytes = 0;
        bool bodyTruncated = false;
        while (true)
        {
            int remainingBytes = MaximumRequestBodyBytes - totalBytes;
            int readLimit = Math.Min(buffer.Length, remainingBytes + 1);
            int bytesRead = await stream.ReadAsync(
                buffer.AsMemory(0, readLimit),
                cancellationToken);
            if (bytesRead == 0)
            {
                break;
            }
            hash.AppendData(buffer, 0, bytesRead);
            totalBytes += bytesRead;
            if (totalBytes > MaximumRequestBodyBytes)
            {
                bodyTruncated = true;
                break;
            }
        }
        return new RequestEvidence(
            $"sha256:{Convert.ToHexString(hash.GetHashAndReset()).ToLowerInvariant()}",
            bodyTruncated);
    }

    private static bool IsAuthorized(
        AuthenticationHeaderValue? authorization) =>
        authorization is not null &&
        string.Equals(
            authorization.Scheme,
            "Bearer",
            StringComparison.OrdinalIgnoreCase) &&
        string.Equals(
            authorization.Parameter,
            SyntheticBearerMarker,
            StringComparison.Ordinal);

    private static string ClassifyRoute(HttpRequestMessage request)
    {
        Uri? uri = request.RequestUri;
        if (
            uri is null ||
            !uri.IsAbsoluteUri ||
            !string.Equals(uri.Scheme, "https", StringComparison.Ordinal) ||
            !uri.IsDefaultPort ||
            uri.Port != 443 ||
            !string.Equals(
                uri.DnsSafeHost,
                ExpectedFakeHost,
                StringComparison.OrdinalIgnoreCase) ||
            !string.IsNullOrEmpty(uri.UserInfo) ||
            !string.IsNullOrEmpty(uri.Fragment)
        )
        {
            return "unrecognized";
        }
        if (
            request.Method == HttpMethod.Post &&
            IsProjectRoute(uri.AbsolutePath, "/openai/v1/responses") &&
            string.IsNullOrEmpty(uri.Query)
        )
        {
            return "responses";
        }
        if (
            request.Method != HttpMethod.Get ||
            !IsProjectRoute(uri.AbsolutePath, "/deployments")
        )
        {
            return "unrecognized";
        }

        Dictionary<string, List<string>> query;
        try
        {
            query = ParseQuery(uri.Query);
        }
        catch (UriFormatException)
        {
            return "unrecognized";
        }
        if (
            query.Keys.Any(key =>
                key is not "api-version" and
                    not "deploymentType" and
                    not "continuationToken") ||
            !query.TryGetValue("api-version", out List<string>? versions) ||
            versions.Count != 1 ||
            versions[0] != "v1"
        )
        {
            return "unrecognized";
        }
        if (
            query.TryGetValue(
                "deploymentType",
                out List<string>? deploymentTypes) &&
            (
                deploymentTypes.Count != 1 ||
                deploymentTypes[0] != "ModelDeployment"
            )
        )
        {
            return "unrecognized";
        }
        return (
            !query.TryGetValue(
                "continuationToken",
                out List<string>? continuationTokens) ||
            continuationTokens.Count == 1)
            ? "deployments"
            : "unrecognized";
    }

    private static bool IsProjectRoute(string path, string suffix)
    {
        const string prefix = "/api/projects/";
        if (!path.StartsWith(prefix, StringComparison.Ordinal))
        {
            return false;
        }
        string remainder = path[prefix.Length..];
        int separator = remainder.IndexOf('/');
        return (
            separator > 0 &&
            separator <= 128 &&
            remainder[(separator)..].Equals(
                suffix,
                StringComparison.Ordinal)
        );
    }

    private static Dictionary<string, List<string>> ParseQuery(string query)
    {
        Dictionary<string, List<string>> values = new(StringComparer.Ordinal);
        foreach (
            string part in
                query.TrimStart('?').Split(
                    '&',
                    StringSplitOptions.RemoveEmptyEntries)
        )
        {
            string[] pair = part.Split('=', 2);
            string name = Uri.UnescapeDataString(pair[0]);
            string value = pair.Length == 2
                ? Uri.UnescapeDataString(pair[1])
                : string.Empty;
            if (!values.TryGetValue(name, out List<string>? matches))
            {
                matches = [];
                values.Add(name, matches);
            }
            matches.Add(value);
        }
        return values;
    }

    private static HttpResponseMessage CreateError(
        HttpStatusCode statusCode,
        string code,
        HttpRequestMessage request)
    {
        string body = JsonSerializer.Serialize(new
        {
            error = new
            {
                code,
            },
        });
        return new HttpResponseMessage(statusCode)
        {
            Content = new StringContent(
                body,
                Encoding.UTF8,
                "application/json"),
            RequestMessage = request,
        };
    }

    private sealed record RequestEvidence(
        string BodySha256,
        bool BodyTruncated);
}
