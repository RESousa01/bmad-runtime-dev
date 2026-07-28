using System.Globalization;
using System.Text.RegularExpressions;
using Microsoft.Extensions.Primitives;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// <c>GET /desktop/v1/model-catalog</c>.
///
/// <para>
/// The catalog is a read. It requires the approved desktop client and the
/// desktop read scope, and deliberately NOT the model-invocation scope: a user
/// must be able to see which models exist, and why one disappeared, without
/// having consented to invoke anything.
/// </para>
/// <para>
/// The response body is the sanitized projection and nothing else. Entitlement
/// filtering happens before projection, so a profile the subject may not use
/// either carries a blocked state and a recovery action or is absent — never a
/// group name, a deployment name, a project endpoint, a role assignment, or a
/// credential.
/// </para>
/// </summary>
public static partial class ModelCatalogEndpoint
{
    public const string RoutePattern = "/model-catalog";

    /// <summary>The bootstrap capability that advertises this route.</summary>
    public const string BootstrapCapability = "model_catalog";

    /// <summary>
    /// Optional client contract epoch. An installation below the minimum is
    /// told to upgrade rather than handed a catalog whose bindings its receipt
    /// handling cannot reproduce.
    /// </summary>
    public const string ContractEpochHeader = "X-Sapphirus-Contract-Epoch";

    public const int MinimumContractEpoch = 1;

    /// <summary>
    /// Optional hint naming the caller's last known-good selection. It is
    /// advisory: it is honoured only when that profile is still selectable.
    /// </summary>
    public const string SelectionHintQueryKey = "lastSelectedProfileId";

    /// <summary>
    /// Short and private. The catalog is per-subject, so a shared cache must
    /// never hold it, and revalidation is cheap because the ETag is the catalog
    /// hash the server already computed.
    /// </summary>
    public const int CacheMaximumAgeSeconds = 30;

    public static RouteHandlerBuilder MapModelCatalog(this RouteGroupBuilder group)
    {
        ArgumentNullException.ThrowIfNull(group);
        return group.MapGet(RoutePattern, HandleAsync)
            .RequireAuthorization(DesktopScopes.Access);
    }

    internal static async Task<IResult> HandleAsync(
        HttpContext context,
        EffectiveModelCatalogService catalogService,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(context);
        ArgumentNullException.ThrowIfNull(catalogService);

        if (!TryReadContractEpoch(context.Request, out bool supported))
        {
            return Problem("request_invalid", StatusCodes.Status400BadRequest);
        }
        if (!supported)
        {
            return Problem(
                ModelCatalogErrorCodes.ContractUpgradeRequired,
                StatusCodes.Status426UpgradeRequired);
        }
        if (!TryReadSelectionHint(context.Request, out string? hint))
        {
            return Problem("request_invalid", StatusCodes.Status400BadRequest);
        }

        ModelCatalogResult result = await catalogService
            .BuildAsync(
                ScopeAuthorization.SubjectPartition(context.User),
                hint,
                cancellationToken)
            .ConfigureAwait(false);
        if (!result.IsSuccess)
        {
            return Problem(result.ErrorCode, StatusFor(result.ErrorCode));
        }

        ModelCatalogProjection catalog = result.Catalog!.Projection;
        string etag = $"\"{catalog.CatalogHash}\"";
        ApplyCachePolicy(context.Response, etag, catalog);
        return Matches(context.Request, etag)
            ? Results.StatusCode(StatusCodes.Status304NotModified)
            : Results.Ok(catalog);
    }

    /// <summary>
    /// The catalog is bound to one subject and one entitlement class, so every
    /// response is private and carries a strong validator. <c>Vary</c> on the
    /// credential is what stops an intermediary keyed only on the URL from
    /// handing one subject's catalog to another.
    /// </summary>
    private static void ApplyCachePolicy(
        HttpResponse response,
        string etag,
        ModelCatalogProjection catalog)
    {
        response.Headers.ETag = etag;
        response.Headers.CacheControl =
            $"private, max-age={CacheMaximumAgeSeconds}, must-revalidate";
        response.Headers.Vary = "Authorization";
        response.Headers.LastModified = ToHttpDate(catalog.GeneratedAt);
        response.Headers.Expires = ToHttpDate(catalog.ExpiresAt);
    }

    private static string ToHttpDate(DateTimeOffset value) =>
        value.ToUniversalTime().ToString("R", CultureInfo.InvariantCulture);

    private static bool Matches(HttpRequest request, string etag)
    {
        StringValues header = request.Headers.IfNoneMatch;
        if (header.Count == 0)
        {
            return false;
        }
        foreach (string? candidate in header)
        {
            if (candidate is null)
            {
                continue;
            }
            foreach (string part in candidate.Split(','))
            {
                string trimmed = part.Trim();
                // A weak validator is not accepted: the catalog hash is exact,
                // and accepting W/ would mean honouring a comparison the client
                // declared inexact.
                if (string.Equals(trimmed, etag, StringComparison.Ordinal))
                {
                    return true;
                }
            }
        }
        return false;
    }

    private static bool TryReadContractEpoch(HttpRequest request, out bool supported)
    {
        supported = true;
        StringValues header = request.Headers[ContractEpochHeader];
        if (header.Count == 0)
        {
            return true;
        }
        if (header.Count > 1)
        {
            return false;
        }
        string value = header[0] ?? "";
        if (value.Length is < 1 or > 10
            || !int.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out int epoch))
        {
            return false;
        }
        supported = epoch >= MinimumContractEpoch;
        return true;
    }

    /// <summary>
    /// Only one query key exists on this route. An unknown one is refused
    /// rather than ignored, for the same reason the JSON reader refuses an
    /// unknown member: a parameter the server silently drops is a parameter the
    /// client believes took effect.
    /// </summary>
    private static bool TryReadSelectionHint(HttpRequest request, out string? hint)
    {
        hint = null;
        foreach (KeyValuePair<string, StringValues> entry in request.Query)
        {
            if (!string.Equals(entry.Key, SelectionHintQueryKey, StringComparison.Ordinal))
            {
                return false;
            }
            if (entry.Value.Count != 1)
            {
                return false;
            }
            string value = entry.Value[0] ?? "";
            if (value.Length is < 3 or > 64 || !ProfileId().IsMatch(value))
            {
                return false;
            }
            hint = value;
        }
        return true;
    }

    internal static int StatusFor(string errorCode) => errorCode switch
    {
        ModelCatalogErrorCodes.Stale => StatusCodes.Status503ServiceUnavailable,
        ModelCatalogErrorCodes.NoEntitledProfiles => StatusCodes.Status403Forbidden,
        ModelCatalogErrorCodes.ContractUpgradeRequired => StatusCodes.Status426UpgradeRequired,
        _ => StatusCodes.Status503ServiceUnavailable,
    };

    private static IResult Problem(string code, int status) =>
        Results.Json(RequestGuards.SafeProblem(code, status), statusCode: status);

    [GeneratedRegex("^[a-z][a-z0-9]*(-[a-z0-9]+){0,7}$")]
    private static partial Regex ProfileId();
}
