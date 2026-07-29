using System.Net;
using System.Net.Http.Headers;
using System.Security.Claims;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;
using Sapphirus.DesktopSupportApi;
using Sapphirus.DesktopSupportApi.Security;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Security;

/// <summary>
/// The static desktop bearer-token mechanism is a real, explicitly opt-in
/// production authentication path for an operator who will never hold Entra
/// app-registration rights for this internal application — not a
/// Development-only stand-in. These tests pin: it is absent unless
/// explicitly enabled, in every environment; a weak or missing token is
/// refused at composition rather than at the first request;
/// <see cref="ScopeAuthorization.IsApprovedDesktopPrincipal"/> genuinely runs
/// against the principal it builds; and the token itself never appears in
/// any observable output.
/// </summary>
public sealed class DevelopmentDesktopAuthenticationTests
{
    private const string TenantId = "11111111-1111-1111-1111-111111111111";
    private const string ApprovedClientId = "22222222-2222-2222-2222-222222222222";
    private const string ValidToken = "sapphirus-static-token-test-fixture-000001";
    private const string OtherValidToken = "sapphirus-static-token-test-fixture-000002";

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData("short")]
    [InlineData("changeme")]
    [InlineData("CHANGEME")]
    [InlineData("token")]
    [InlineData("test")]
    [InlineData("password")]
    public void A_missing_short_or_placeholder_token_is_rejected_at_construction(string? token)
    {
        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => new StaticDesktopTokenAuthenticator(token, null, CreateSupportPlane()));

        Assert.Equal("static_desktop_token_invalid", error.Message);
    }

    [Fact]
    public void A_token_of_one_repeated_character_is_rejected_even_at_a_legal_length()
    {
        string repeated = new('a', 40);

        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => new StaticDesktopTokenAuthenticator(repeated, null, CreateSupportPlane()));

        Assert.Equal("static_desktop_token_invalid", error.Message);
    }

    [Fact]
    public void A_well_formed_token_constructs_without_error()
    {
        StaticDesktopTokenAuthenticator authenticator =
            new(ValidToken, null, CreateSupportPlane());

        Assert.NotNull(authenticator);
    }

    [Fact]
    public void An_explicit_subject_configuration_error_is_rejected_at_validate_time()
    {
        SupportPlaneOptions options = new()
        {
            Authority = $"https://login.microsoftonline.com/{TenantId}/v2.0",
            Audience = "api://66666666-6666-6666-6666-666666666666",
            ApprovedDesktopClientId = ApprovedClientId,
            Region = "westeurope",
            StaticDesktopSubjectId = "not-a-guid",
        };

        Assert.Throws<InvalidOperationException>(
            () => options.Validate(new StubHostEnvironment("Production")));
    }

    [Fact]
    public void Two_authenticators_built_from_the_same_token_derive_the_same_subject()
    {
        ClaimsPrincipal? first = new StaticDesktopTokenAuthenticator(
                ValidToken, null, CreateSupportPlane())
            .TryAuthenticate(ValidToken);
        ClaimsPrincipal? second = new StaticDesktopTokenAuthenticator(
                ValidToken, null, CreateSupportPlane())
            .TryAuthenticate(ValidToken);

        Assert.NotNull(first);
        Assert.NotNull(second);
        Assert.Equal(first!.FindFirstValue("oid"), second!.FindFirstValue("oid"));
        // Never the token itself, and never a real Entra-shaped GUID.
        Assert.DoesNotContain(ValidToken, first.FindFirstValue("oid") ?? "", StringComparison.Ordinal);
        Assert.False(Guid.TryParse(first.FindFirstValue("oid"), out _));
    }

    [Fact]
    public void A_different_token_derives_a_different_subject()
    {
        string? subjectA = new StaticDesktopTokenAuthenticator(ValidToken, null, CreateSupportPlane())
            .TryAuthenticate(ValidToken)?.FindFirstValue("oid");
        string? subjectB = new StaticDesktopTokenAuthenticator(OtherValidToken, null, CreateSupportPlane())
            .TryAuthenticate(OtherValidToken)?.FindFirstValue("oid");

        Assert.NotEqual(subjectA, subjectB);
    }

    [Fact]
    public void An_explicit_subject_is_used_verbatim_instead_of_the_derived_one()
    {
        const string explicitSubject = "99999999-9999-9999-9999-999999999999";

        ClaimsPrincipal? principal = new StaticDesktopTokenAuthenticator(
                ValidToken, explicitSubject, CreateSupportPlane())
            .TryAuthenticate(ValidToken);

        Assert.Equal(explicitSubject, principal!.FindFirstValue("oid"));
    }

    [Fact]
    public void The_wrong_token_and_a_missing_token_are_both_refused()
    {
        StaticDesktopTokenAuthenticator authenticator =
            new(ValidToken, null, CreateSupportPlane());

        Assert.Null(authenticator.TryAuthenticate(OtherValidToken));
        Assert.Null(authenticator.TryAuthenticate(""));
        Assert.Null(authenticator.TryAuthenticate(null));
        // A same-length near-miss is refused too, not just a shorter one.
        Assert.Null(authenticator.TryAuthenticate(
            ValidToken[..^1] + (ValidToken[^1] == '1' ? '2' : '1')));
    }

    [Fact]
    public void The_principal_carries_the_development_marker_and_satisfies_scope_authorization_for_granted_scopes()
    {
        SupportPlaneOptions options = CreateSupportPlane();
        ClaimsPrincipal principal = new StaticDesktopTokenAuthenticator(
                ValidToken, null, options)
            .TryAuthenticate(ValidToken)!;

        Assert.Equal("true", principal.FindFirstValue(
            StaticDesktopTokenAuthenticator.StaticTokenPrincipalClaimType));
        Assert.True(ScopeAuthorization.IsApprovedDesktopPrincipal(
            principal, DesktopScopes.Access, options));
        Assert.True(ScopeAuthorization.IsApprovedDesktopPrincipal(
            principal, DesktopScopes.ModelInvoke, options));
    }

    /// <summary>
    /// The one scope deliberately withheld from
    /// <see cref="StaticDesktopTokenAuthenticator.GrantedScopes"/>. If this
    /// ever passed, scope enforcement would not actually be running against
    /// this principal.
    /// </summary>
    [Fact]
    public void The_principal_is_refused_a_scope_it_was_not_granted()
    {
        SupportPlaneOptions options = CreateSupportPlane();
        ClaimsPrincipal principal = new StaticDesktopTokenAuthenticator(
                ValidToken, null, options)
            .TryAuthenticate(ValidToken)!;

        Assert.DoesNotContain(
            DesktopScopes.DiagnosticsUpload,
            StaticDesktopTokenAuthenticator.GrantedScopes);
        Assert.False(ScopeAuthorization.IsApprovedDesktopPrincipal(
            principal, DesktopScopes.DiagnosticsUpload, options));
    }

    [Fact]
    public void A_principal_from_an_unapproved_client_is_still_refused()
    {
        SupportPlaneOptions options = CreateSupportPlane();
        ClaimsPrincipal principal = new StaticDesktopTokenAuthenticator(
                ValidToken, null, options)
            .TryAuthenticate(ValidToken)!;
        Assert.True(ScopeAuthorization.IsApprovedDesktopPrincipal(
            principal, DesktopScopes.Access, options));

        // Rebuilding against a support-plane configuration with a different
        // approved client proves the azp check is genuinely evaluated,
        // rather than the static path silently satisfying it.
        SupportPlaneOptions differentClient = CreateSupportPlane(
            approvedDesktopClientId: "77777777-7777-7777-7777-777777777777");
        ClaimsPrincipal otherPrincipal = new StaticDesktopTokenAuthenticator(
                ValidToken, null, differentClient)
            .TryAuthenticate(ValidToken)!;
        Assert.False(ScopeAuthorization.IsApprovedDesktopPrincipal(
            principal, DesktopScopes.Access, differentClient));
        Assert.True(ScopeAuthorization.IsApprovedDesktopPrincipal(
            otherPrincipal, DesktopScopes.Access, differentClient));
    }

    [Fact]
    public async Task Disabled_by_default_composition_registers_only_jwt_bearer()
    {
        ServiceCollection services = new();
        SupportPlaneOptions options = CreateSupportPlane();
        services.AddDesktopAuthentication(
            new ConfigurationBuilder().Build(),
            options,
            ConfigureFakeJwtBearer);

        using ServiceProvider provider = services.BuildServiceProvider();
        IAuthenticationSchemeProvider schemes =
            provider.GetRequiredService<IAuthenticationSchemeProvider>();
        IEnumerable<AuthenticationScheme> allSchemes = await schemes.GetAllSchemesAsync();

        Assert.DoesNotContain(
            allSchemes,
            scheme => scheme.Name == StaticDesktopAuthenticationDefaults.AuthenticationScheme);
        Assert.DoesNotContain(
            services,
            descriptor => descriptor.ServiceType == typeof(StaticDesktopTokenAuthenticator));
        AuthenticationScheme? defaultScheme = await schemes.GetDefaultAuthenticateSchemeAsync();
        Assert.Equal(
            Microsoft.AspNetCore.Authentication.JwtBearer.JwtBearerDefaults.AuthenticationScheme,
            defaultScheme?.Name);
    }

    [Fact]
    public async Task Enabled_composition_registers_both_schemes_and_defaults_to_the_static_one()
    {
        ServiceCollection services = new();
        SupportPlaneOptions options = CreateSupportPlane(staticTokenEnabled: true);
        IConfiguration configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?>(StringComparer.Ordinal)
            {
                [StaticDesktopTokenAuthenticator.ConfigurationKey] = ValidToken,
            })
            .Build();

        services.AddDesktopAuthentication(configuration, options, ConfigureFakeJwtBearer);

        using ServiceProvider provider = services.BuildServiceProvider();
        IAuthenticationSchemeProvider schemes =
            provider.GetRequiredService<IAuthenticationSchemeProvider>();
        IEnumerable<AuthenticationScheme> allSchemes = await schemes.GetAllSchemesAsync();

        Assert.Contains(
            allSchemes,
            scheme => scheme.Name == StaticDesktopAuthenticationDefaults.AuthenticationScheme);
        Assert.Contains(
            allSchemes,
            scheme => scheme.Name
                == Microsoft.AspNetCore.Authentication.JwtBearer.JwtBearerDefaults.AuthenticationScheme);
        AuthenticationScheme? defaultScheme = await schemes.GetDefaultAuthenticateSchemeAsync();
        Assert.Equal(
            StaticDesktopAuthenticationDefaults.AuthenticationScheme,
            defaultScheme?.Name);
        Assert.Contains(
            services,
            descriptor => descriptor.ServiceType == typeof(StaticDesktopTokenAuthenticator));
    }

    /// <summary>
    /// The fail-closed boundary at the composition call site itself: enabling
    /// the mechanism with no token configured throws immediately, during
    /// <c>AddDesktopAuthentication</c>, not on first resolution and not on
    /// the first request.
    /// </summary>
    [Fact]
    public void Enabling_the_mechanism_with_no_configured_token_fails_closed_at_composition()
    {
        ServiceCollection services = new();
        SupportPlaneOptions options = CreateSupportPlane(staticTokenEnabled: true);

        InvalidOperationException error = Assert.Throws<InvalidOperationException>(
            () => services.AddDesktopAuthentication(
                new ConfigurationBuilder().Build(),
                options,
                ConfigureFakeJwtBearer));

        Assert.Equal("static_desktop_token_invalid", error.Message);
    }

    [Fact]
    public async Task A_correct_static_token_reaches_a_desktop_route_end_to_end()
    {
        await using StaticTokenApiFactory factory = new(ValidToken, staticTokenEnabled: true);
        using HttpClient client = factory.CreateSecureClient();
        client.DefaultRequestHeaders.Authorization =
            new AuthenticationHeaderValue("Bearer", ValidToken);

        using HttpResponseMessage response = await client.GetAsync(
            "/desktop/v1/bootstrap",
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
    }

    [Fact]
    public async Task A_wrong_static_token_is_refused_end_to_end()
    {
        await using StaticTokenApiFactory factory = new(ValidToken, staticTokenEnabled: true);
        using HttpClient client = factory.CreateSecureClient();
        client.DefaultRequestHeaders.Authorization =
            new AuthenticationHeaderValue("Bearer", OtherValidToken);

        using HttpResponseMessage response = await client.GetAsync(
            "/desktop/v1/bootstrap",
            TestContext.Current.CancellationToken);
        string body = await response.Content.ReadAsStringAsync(
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
        Assert.DoesNotContain(ValidToken, body, StringComparison.Ordinal);
    }

    [Fact]
    public async Task A_missing_authorization_header_is_refused_end_to_end()
    {
        await using StaticTokenApiFactory factory = new(ValidToken, staticTokenEnabled: true);
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await client.GetAsync(
            "/desktop/v1/bootstrap",
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    /// <summary>
    /// With the mechanism disabled (the default in every environment), the
    /// static token is not accepted even when it is presented, and the host
    /// falls back to challenging for the Entra scheme exactly as before this
    /// feature existed.
    /// </summary>
    [Fact]
    public async Task With_the_mechanism_disabled_a_static_token_does_not_authenticate()
    {
        await using StaticTokenApiFactory factory = new(ValidToken, staticTokenEnabled: false);
        using HttpClient client = factory.CreateSecureClient();
        client.DefaultRequestHeaders.Authorization =
            new AuthenticationHeaderValue("Bearer", ValidToken);

        using HttpResponseMessage response = await client.GetAsync(
            "/desktop/v1/bootstrap",
            TestContext.Current.CancellationToken);

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    private static void ConfigureFakeJwtBearer(
        Microsoft.AspNetCore.Authentication.JwtBearer.JwtBearerOptions options)
    {
        options.Authority = $"https://login.microsoftonline.com/{TenantId}/v2.0";
        options.Audience = "api://66666666-6666-6666-6666-666666666666";
        options.RequireHttpsMetadata = true;
    }

    private static SupportPlaneOptions CreateSupportPlane(
        bool staticTokenEnabled = false,
        string approvedDesktopClientId = ApprovedClientId)
    {
        SupportPlaneOptions options = new()
        {
            Authority = $"https://login.microsoftonline.com/{TenantId}/v2.0",
            Audience = "api://66666666-6666-6666-6666-666666666666",
            ApprovedDesktopClientId = approvedDesktopClientId,
            Region = "westeurope",
            StaticDesktopTokenEnabled = staticTokenEnabled,
        };
        options.Validate(new StubHostEnvironment("Production"));
        return options;
    }

    private sealed class StubHostEnvironment(string environmentName)
        : Microsoft.Extensions.Hosting.IHostEnvironment
    {
        public string EnvironmentName { get; set; } = environmentName;
        public string ApplicationName { get; set; } = "Sapphirus.DesktopSupportApi.Tests";
        public string ContentRootPath { get; set; } = AppContext.BaseDirectory;
        public Microsoft.Extensions.FileProviders.IFileProvider ContentRootFileProvider
        {
            get;
            set;
        } = new Microsoft.Extensions.FileProviders.NullFileProvider();
    }

    /// <summary>
    /// A real end-to-end host, unlike <c>SupportApiSecurityTests.SupportApiFactory</c>,
    /// which replaces authentication entirely with a test handler. Here the
    /// real <see cref="DesktopAuthenticationComposition"/> path runs, which is
    /// the only way to prove the static token reaches an actual route through
    /// the real pipeline rather than through a stand-in.
    /// </summary>
    private sealed class StaticTokenApiFactory(
        string configuredToken,
        bool staticTokenEnabled) : WebApplicationFactory<Program>
    {
        protected override void ConfigureWebHost(IWebHostBuilder builder)
        {
            builder.UseEnvironment("Development");
            builder.ConfigureLogging(logging => logging.ClearProviders());
            builder.UseSetting("Sapphirus:Authority", $"https://login.microsoftonline.com/{TenantId}/v2.0");
            builder.UseSetting("Sapphirus:Audience", "api://66666666-6666-6666-6666-666666666666");
            builder.UseSetting("Sapphirus:ApprovedDesktopClientId", ApprovedClientId);
            builder.UseSetting("Sapphirus:Region", "test-region");
            builder.UseSetting("Sapphirus:ReleaseChannel", "beta");
            builder.UseSetting("Sapphirus:DevelopmentSigningEnabled", "true");
            builder.UseSetting("Sapphirus:DevelopmentModelEnabled", "true");
            builder.UseSetting(
                "Sapphirus:StaticDesktopTokenEnabled",
                staticTokenEnabled ? "true" : "false");
            builder.UseSetting("Sapphirus:StaticDesktopToken", configuredToken);
        }

        public HttpClient CreateSecureClient() => CreateClient(new WebApplicationFactoryClientOptions
        {
            AllowAutoRedirect = false,
            BaseAddress = new Uri("https://localhost"),
        });
    }
}
