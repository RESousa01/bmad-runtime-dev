namespace Sapphirus.DesktopSupportApi;

public sealed class SupportPlaneOptions
{
    public const string SectionName = "Sapphirus";
    public string Authority { get; init; } = "";
    public string Audience { get; init; } = "";
    public string ApprovedDesktopClientId { get; init; } = "";
    public string Region { get; init; } = "development";
    public string ReleaseChannel { get; init; } = "beta";
    public int IdempotencyMaximumEntries { get; init; } = 4096;
    public int IdempotencyRetentionMinutes { get; init; } = 15;
    public int ConnectedOperationTimeoutSeconds { get; init; } = 120;
    public bool DevelopmentSigningEnabled { get; init; }
    public bool DevelopmentModelEnabled { get; init; }
    public string DevelopmentConsentStorePath { get; init; } = "";

    /// <summary>
    /// Opt-in gate for the static desktop bearer-token authentication
    /// mechanism (see
    /// <c>Sapphirus.DesktopSupportApi.Security.DesktopAuthenticationComposition</c>).
    /// Defaults to <see langword="false"/>, in which case
    /// <c>AddJwtBearer</c> is the only registered scheme, exactly as before
    /// this feature existed. This is not restricted to Development or Test:
    /// it is a legitimate production mechanism for an operator who will
    /// never hold Entra app-registration rights for this internal
    /// application, so it must be explicit and never a silent default in any
    /// environment.
    /// </summary>
    public bool StaticDesktopTokenEnabled { get; init; }

    /// <summary>
    /// Optional explicit subject identifier for the static-token principal,
    /// expected in <c>Guid</c> <c>"D"</c> format when supplied. When left
    /// blank, the subject is instead derived deterministically from the
    /// configured token itself (see
    /// <c>Security.StaticDesktopTokenAuthenticator</c>), which keeps it
    /// stable across restarts without needing extra configuration and
    /// without ever exposing the token.
    /// </summary>
    public string StaticDesktopSubjectId { get; init; } = "";

    public Guid TenantId { get; private set; }
    public Guid ApprovedDesktopClient { get; private set; }

    public void Validate(IHostEnvironment environment)
    {
        if (!Uri.TryCreate(Authority, UriKind.Absolute, out Uri? authority)
            || authority.Scheme != Uri.UriSchemeHttps
            || !authority.IsDefaultPort
            || authority.UserInfo.Length != 0
            || authority.Query.Length != 0
            || authority.Fragment.Length != 0
            || !string.Equals(
                authority.Host,
                "login.microsoftonline.com",
                StringComparison.OrdinalIgnoreCase)
            || !TryGetTenantId(authority, out Guid tenantId)
            || tenantId == Guid.Empty
            || !string.Equals(
                Authority,
                $"https://login.microsoftonline.com/{tenantId:D}/v2.0",
                StringComparison.Ordinal)
            || !Uri.TryCreate(Audience, UriKind.Absolute, out Uri? audience)
            || !string.Equals(audience.Scheme, "api", StringComparison.Ordinal)
            || !Guid.TryParseExact(ApprovedDesktopClientId, "D", out Guid approvedDesktopClient)
            || approvedDesktopClient == Guid.Empty
            || ReleaseChannel is not ("beta" or "stable")
            || IdempotencyMaximumEntries is < 16 or > 65536
            || IdempotencyRetentionMinutes is < 1 or > 1440
            || ConnectedOperationTimeoutSeconds is < 1 or > 600)
        {
            throw new InvalidOperationException(
                "Sapphirus configuration must name one Entra tenant, API audience, approved desktop client, release channel, and bounded idempotency policy.");
        }
        if (!environment.IsDevelopment()
            && (DevelopmentSigningEnabled
                || DevelopmentModelEnabled
                || !string.IsNullOrWhiteSpace(DevelopmentConsentStorePath)))
        {
            throw new InvalidOperationException(
                "Development signing and model adapters cannot run outside Development.");
        }
        if (!string.IsNullOrWhiteSpace(DevelopmentConsentStorePath)
            && !Path.IsPathFullyQualified(DevelopmentConsentStorePath))
        {
            throw new InvalidOperationException(
                "The development consent store path must be fully qualified.");
        }
        if (!string.IsNullOrWhiteSpace(StaticDesktopSubjectId)
            && !Guid.TryParseExact(StaticDesktopSubjectId, "D", out Guid _))
        {
            throw new InvalidOperationException(
                "Sapphirus:StaticDesktopSubjectId must be a lowercase 'D'-format GUID when supplied.");
        }
        TenantId = tenantId;
        ApprovedDesktopClient = approvedDesktopClient;
    }

    private static bool TryGetTenantId(Uri authority, out Guid tenantId)
    {
        tenantId = Guid.Empty;
        string[] segments = authority.AbsolutePath.Trim('/').Split('/');
        return segments.Length == 2
            && string.Equals(segments[1], "v2.0", StringComparison.Ordinal)
            && Guid.TryParseExact(segments[0], "D", out tenantId);
    }
}
