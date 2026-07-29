using System.Security.Claims;
using System.Security.Cryptography;
using System.Text;
using System.Text.Encodings.Web;
using Microsoft.AspNetCore.Authentication;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Options;

namespace Sapphirus.DesktopSupportApi.Security;

/// <summary>
/// The static-bearer-token authentication mechanism for the inbound boundary
/// (desktop -&gt; plane).
///
/// <para>
/// <b>Why this exists.</b> This is an internal application, and the operator
/// running it will never hold Entra app-registration rights for it. Rather
/// than a development stand-in that is later replaced, this is the real,
/// explicitly opt-in production mechanism for that operator: a single
/// operator-supplied bearer token authenticates the desktop client instead of
/// an Entra JWT. It is gated on
/// <see cref="SupportPlaneOptions.StaticDesktopTokenEnabled"/>, which
/// defaults to <see langword="false"/> in every environment, so
/// <c>AddJwtBearer</c> remains the sole authentication path unless an
/// operator explicitly turns this on. See
/// <see cref="DesktopAuthenticationComposition"/> for the gate.
/// </para>
/// <para>
/// <b>What this is not.</b> It is not a relaxed authorization path. The
/// principal it builds carries exactly the claim shape a real Entra token
/// would (<c>tid</c>, <c>oid</c>, <c>azp</c>, <c>scp</c>), and
/// <see cref="ScopeAuthorization.IsApprovedDesktopPrincipal"/> runs against it
/// completely unmodified. A scope or client-id bug in that method still
/// surfaces when this mechanism is in use.
/// </para>
/// <para>
/// <b>If a real Entra app registration exists later.</b> Nothing here needs
/// to change to turn Entra on: an operator who gains app-registration rights
/// simply leaves <see cref="SupportPlaneOptions.StaticDesktopTokenEnabled"/>
/// false (or flips it back), and <c>AddJwtBearer</c> — registered
/// unconditionally, in every environment, unchanged — takes over as the sole
/// scheme again. This class, this configuration flag, and the seam in
/// <see cref="DesktopAuthenticationComposition"/> are exactly what an
/// Entra rollout would delete.
/// </para>
/// </summary>
public sealed class StaticDesktopTokenAuthenticator
{
    /// <summary>
    /// Configuration key for the static desktop bearer token, supplied
    /// through the <c>Sapphirus__StaticDesktopToken</c> environment
    /// variable. It must never be committed with a real value. Treated as a
    /// production credential: this is the only thing standing between the
    /// network and a plane that holds the key to a real Foundry project once
    /// <see cref="SupportPlaneOptions.StaticDesktopTokenEnabled"/> is set.
    /// </summary>
    public const string ConfigurationKey = "Sapphirus:StaticDesktopToken";

    /// <summary>The environment-variable spelling of <see cref="ConfigurationKey"/>.</summary>
    public const string EnvironmentVariableName = "Sapphirus__StaticDesktopToken";

    /// <summary>
    /// Raised when the mechanism is enabled but the configured token is
    /// missing, too short, too long, contains a non-printable character, or
    /// is an obviously weak or placeholder value. Fails at composition,
    /// never at the first request.
    /// </summary>
    public const string TokenInvalidCode = "static_desktop_token_invalid";

    /// <summary>
    /// A production credential is held to a materially higher bar than the
    /// outbound development Foundry key: this is not a development stand-in,
    /// it is the only thing authenticating every desktop request.
    /// </summary>
    internal const int MinimumTokenLength = 32;

    internal const int MaximumTokenLength = 512;

    /// <summary>
    /// Obviously weak or placeholder values, checked case-insensitively
    /// against the whole token. Kept even though
    /// <see cref="MinimumTokenLength"/> already rejects most of these on
    /// length alone, so the rejection reason stays correct if the length
    /// bound is ever revisited independently.
    /// </summary>
    internal static readonly string[] WeakTokenValues =
    [
        "changeme",
        "change-me",
        "token",
        "test",
        "password",
        "secret",
        "default",
        "admin",
        "placeholder",
    ];

    /// <summary>
    /// The claim that makes the resulting principal identifiable as
    /// static-token-authenticated. A receipt or audit record produced under
    /// this mechanism must never be mistaken for one produced from a real
    /// Entra token; this claim, plus the derived (never GUID-shaped, never
    /// reversible) subject below, is what lets evidence tell them apart.
    /// </summary>
    public const string StaticTokenPrincipalClaimType = "sapphirus_static_token_principal";

    /// <summary>
    /// The development principal is granted every desktop scope except
    /// <see cref="DesktopScopes.DiagnosticsUpload"/>. The omission is
    /// deliberate: it is what lets a test prove
    /// <see cref="ScopeAuthorization.IsApprovedDesktopPrincipal"/> is
    /// genuinely being exercised against this principal rather than bypassed
    /// — a scope this principal was not granted must still be refused.
    /// </summary>
    public static readonly string[] GrantedScopes =
    [
        DesktopScopes.Access,
        DesktopScopes.ModelInvoke,
        DesktopScopes.PackageRead,
        DesktopScopes.Sync,
        DesktopScopes.Collaboration,
        DesktopScopes.RemoteJobSubmit,
    ];

    private readonly byte[] _expectedTokenBytes;
    private readonly string _subjectId;
    private readonly SupportPlaneOptions _supportPlane;

    /// <summary>
    /// The fail-closed boundary: a missing, malformed, or obviously weak
    /// token throws here, at composition, rather than producing a scheme
    /// that accepts nothing (every request fails, indistinguishable from a
    /// misconfigured Entra tenant) or — far worse — a scheme that accepts
    /// too much.
    /// </summary>
    public StaticDesktopTokenAuthenticator(
        string? configuredToken,
        string? configuredSubjectId,
        SupportPlaneOptions supportPlane)
    {
        ArgumentNullException.ThrowIfNull(supportPlane);
        if (!IsWellFormedToken(configuredToken))
        {
            // The message never quotes the supplied value.
            throw new InvalidOperationException(TokenInvalidCode);
        }
        _expectedTokenBytes = Encoding.UTF8.GetBytes(configuredToken!);
        _subjectId = string.IsNullOrWhiteSpace(configuredSubjectId)
            ? DeriveSubjectId(configuredToken!)
            : configuredSubjectId;
        _supportPlane = supportPlane;
    }

    /// <summary>
    /// Bounded, printable ASCII, long enough to be a real credential, and
    /// not one of the well-known placeholder values or a single character
    /// repeated across the whole token.
    /// </summary>
    public static bool IsWellFormedToken(string? token)
    {
        if (token is null || token.Length is < MinimumTokenLength or > MaximumTokenLength)
        {
            return false;
        }
        foreach (char character in token)
        {
            if (character is < '!' or > '~')
            {
                return false;
            }
        }
        if (IsRepeatedSingleCharacter(token))
        {
            return false;
        }
        foreach (string weakValue in WeakTokenValues)
        {
            if (string.Equals(token, weakValue, StringComparison.OrdinalIgnoreCase))
            {
                return false;
            }
        }
        return true;
    }

    private static bool IsRepeatedSingleCharacter(string token)
    {
        char first = token[0];
        foreach (char character in token)
        {
            if (character != first)
            {
                return false;
            }
        }
        return true;
    }

    /// <summary>
    /// Derives a stable, non-reversible subject from the configured token
    /// when the operator supplies no explicit
    /// <see cref="SupportPlaneOptions.StaticDesktopSubjectId"/>. It is a
    /// one-way SHA-256 digest under a domain-separated prefix, so it can
    /// never be inverted back to the token, and it is stable across restarts
    /// because it depends only on the token itself, which does not change
    /// between restarts of the same deployment.
    ///
    /// The result is deliberately not GUID-shaped (it carries a
    /// <c>static-</c> prefix), which doubles as a second, independent signal
    /// that a subject partition or receipt hash derived from it did not come
    /// from a real Entra <c>oid</c>.
    /// </summary>
    internal static string DeriveSubjectId(string token)
    {
        byte[] digest = SHA256.HashData(
            Encoding.UTF8.GetBytes("sapphirus:static-desktop-subject:v1\n" + token));
        return "static-" + Convert.ToHexStringLower(digest)[..32];
    }

    /// <summary>
    /// Compares the presented token to the configured one in fixed time and,
    /// on a match, returns the static-token <see cref="ClaimsPrincipal"/>.
    /// Any mismatch — missing, wrong value, or wrong length — returns
    /// <see langword="null"/> without revealing which of those it was.
    /// </summary>
    public ClaimsPrincipal? TryAuthenticate(string? presentedToken)
    {
        if (string.IsNullOrEmpty(presentedToken))
        {
            return null;
        }
        byte[] presentedBytes = Encoding.UTF8.GetBytes(presentedToken);
        // A length mismatch is rejected before the fixed-time comparison
        // because FixedTimeEquals requires equal-length spans; this reveals
        // nothing about the token's content, only that lengths differed,
        // which carries no information a caller could use to narrow the
        // token's value.
        bool matches = presentedBytes.Length == _expectedTokenBytes.Length
            && CryptographicOperations.FixedTimeEquals(presentedBytes, _expectedTokenBytes);
        return matches ? BuildPrincipal() : null;
    }

    private ClaimsPrincipal BuildPrincipal()
    {
        List<Claim> claims =
        [
            new Claim("tid", _supportPlane.TenantId.ToString("D")),
            new Claim("oid", _subjectId),
            new Claim("azp", _supportPlane.ApprovedDesktopClient.ToString("D")),
            new Claim("scp", string.Join(' ', GrantedScopes)),
            new Claim(StaticTokenPrincipalClaimType, "true"),
        ];
        ClaimsIdentity identity = new(
            claims,
            StaticDesktopAuthenticationDefaults.AuthenticationScheme);
        return new ClaimsPrincipal(identity);
    }
}

/// <summary>Scheme name for the static desktop token authentication handler.</summary>
public static class StaticDesktopAuthenticationDefaults
{
    public const string AuthenticationScheme = "SapphirusStaticDesktopToken";
}

/// <summary>No handler-specific options; the token lives on the authenticator.</summary>
public sealed class StaticDesktopAuthenticationOptions : AuthenticationSchemeOptions;

/// <summary>
/// The ASP.NET Core authentication handler that adapts
/// <see cref="StaticDesktopTokenAuthenticator"/> to the
/// <c>Authorization: Bearer</c> convention the desktop client already uses
/// against the Entra path, so no client-side branching is needed to run
/// against this mechanism.
///
/// This class holds no guard of its own by design: ASP.NET Core constructs
/// authentication handlers per request through <c>ActivatorUtilities</c>
/// rather than once at composition, so a throwing constructor here would
/// throw on the first request instead of at startup. The token-strength
/// guard lives in <see cref="StaticDesktopTokenAuthenticator"/>'s
/// constructor instead, which composition resolves exactly once as a
/// singleton (see <see cref="DesktopAuthenticationComposition"/>), and it is
/// that resolution which fails fast at startup.
/// </summary>
public sealed class StaticDesktopAuthenticationHandler
    : AuthenticationHandler<StaticDesktopAuthenticationOptions>
{
    private const string BearerPrefix = "Bearer ";
    private readonly StaticDesktopTokenAuthenticator _authenticator;

    public StaticDesktopAuthenticationHandler(
        IOptionsMonitor<StaticDesktopAuthenticationOptions> options,
        ILoggerFactory logger,
        UrlEncoder encoder,
        StaticDesktopTokenAuthenticator authenticator)
        : base(options, logger, encoder)
    {
        ArgumentNullException.ThrowIfNull(authenticator);
        _authenticator = authenticator;
    }

    protected override Task<AuthenticateResult> HandleAuthenticateAsync()
    {
        string header = Request.Headers.Authorization.ToString();
        if (!header.StartsWith(BearerPrefix, StringComparison.Ordinal))
        {
            return Task.FromResult(AuthenticateResult.NoResult());
        }
        // Never logged, and the failure message below never quotes it.
        string presentedToken = header[BearerPrefix.Length..];
        ClaimsPrincipal? principal = _authenticator.TryAuthenticate(presentedToken);
        if (principal is null)
        {
            return Task.FromResult(
                AuthenticateResult.Fail("The static desktop token was not accepted."));
        }
        AuthenticationTicket ticket = new(
            principal,
            StaticDesktopAuthenticationDefaults.AuthenticationScheme);
        return Task.FromResult(AuthenticateResult.Success(ticket));
    }
}

/// <summary>
/// Composes desktop-facing authentication from one call site, so
/// <c>Program.cs</c> cannot register the two schemes with divergent defaults
/// by accident.
///
/// <para>
/// <b><c>AddJwtBearer</c> is registered unconditionally, in every
/// environment, unchanged.</b> It is, and remains, the default production
/// path and the seam a future Entra rollout uses without any code change
/// here. The static-token scheme is registered <i>alongside</i> it, never
/// instead of it, and only when
/// <see cref="SupportPlaneOptions.StaticDesktopTokenEnabled"/> is explicitly
/// <see langword="true"/> — in any environment, since this operator will
/// never hold Entra app-registration rights for this application regardless
/// of which environment they are running. Only the <i>default</i> scheme —
/// which one is selected when a route does not name a scheme explicitly —
/// differs by that flag, and every <c>/desktop/v1</c> route already requires
/// one of the named <see cref="DesktopScopes"/> policies, so the default
/// selection is the only thing this composition controls.
/// </para>
/// </summary>
public static class DesktopAuthenticationComposition
{
    public static void AddDesktopAuthentication(
        this IServiceCollection services,
        IConfiguration configuration,
        SupportPlaneOptions supportPlane,
        Action<Microsoft.AspNetCore.Authentication.JwtBearer.JwtBearerOptions> configureJwtBearer)
    {
        ArgumentNullException.ThrowIfNull(services);
        ArgumentNullException.ThrowIfNull(configuration);
        ArgumentNullException.ThrowIfNull(supportPlane);
        ArgumentNullException.ThrowIfNull(configureJwtBearer);

        string defaultScheme = supportPlane.StaticDesktopTokenEnabled
            ? StaticDesktopAuthenticationDefaults.AuthenticationScheme
            : Microsoft.AspNetCore.Authentication.JwtBearer.JwtBearerDefaults.AuthenticationScheme;

        Microsoft.AspNetCore.Authentication.AuthenticationBuilder authentication =
            services.AddAuthentication(defaultScheme);

        // Registered for every environment and every value of
        // StaticDesktopTokenEnabled, so the real Entra path is never removed
        // by this composition. An operator who gains Entra app-registration
        // rights later turns this mechanism off by leaving the flag false
        // (its default); nothing else has to change.
        //
        // TODO(entra-rollout): when a real Entra app registration and tenant
        // exist for this application, delete StaticDesktopTokenEnabled, this
        // branch, StaticDesktopTokenAuthenticator, and the two authentication
        // handler types above. AddJwtBearer already needs nothing else.
        authentication.AddJwtBearer(configureJwtBearer);

        if (supportPlane.StaticDesktopTokenEnabled)
        {
            // The fail-closed boundary: a missing, malformed, or obviously
            // weak token throws here, at composition — constructed eagerly,
            // synchronously, right now, rather than behind a lazily
            // resolved factory, so an operator who mistypes or omits the
            // token learns at startup, never on the first request. There is
            // no environment guard around this branch — unlike the outbound
            // development Foundry credential, this mechanism is a
            // deliberate, explicitly opted-into production path, not a
            // development stand-in restricted to Development or Test.
            StaticDesktopTokenAuthenticator authenticator = new(
                configuration[StaticDesktopTokenAuthenticator.ConfigurationKey],
                configuration[StaticDesktopSubjectConfiguration.ConfigurationKey],
                supportPlane);
            services.AddSingleton(authenticator);
            authentication.AddScheme<
                StaticDesktopAuthenticationOptions,
                StaticDesktopAuthenticationHandler>(
                StaticDesktopAuthenticationDefaults.AuthenticationScheme,
                _ => { });
        }
    }
}

/// <summary>
/// The configuration key for the optional explicit static-token subject,
/// kept as its own small type so the key spelling has exactly one owner.
/// </summary>
public static class StaticDesktopSubjectConfiguration
{
    public const string ConfigurationKey = "Sapphirus:StaticDesktopSubjectId";
    public const string EnvironmentVariableName = "Sapphirus__StaticDesktopSubjectId";
}
