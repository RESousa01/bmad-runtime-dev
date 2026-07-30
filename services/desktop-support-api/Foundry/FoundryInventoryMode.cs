namespace Sapphirus.DesktopSupportApi.Foundry;

/// <summary>
/// The one explicit, operator-selected route-inventory mode. Each mode has
/// its own prerequisite evidence and its own qualification; there is no
/// runtime fallback between them; see FND-022.
/// </summary>
public enum FoundryInventoryMode
{
    /// <summary>
    /// Candidate route IDs come only from the signed model-profile registry
    /// and are validated with a retrieve-model or bounded Responses probe.
    /// The portable default.
    /// </summary>
    ConfiguredAndProbed = 0,

    /// <summary>
    /// Key-authenticated <c>GET &lt;projectEndpoint&gt;/openai/v1/models</c>.
    /// Only qualified when the live project proves the returned IDs are
    /// accepted as the Responses <c>model</c> value.
    /// </summary>
    OpenAiModels = 1,

    /// <summary>
    /// The existing paged <c>/deployments?api-version=v1</c> parser,
    /// authenticated with an <c>api-key</c> header instead of a bearer
    /// token. Only qualified when the live endpoint accepts the key.
    /// </summary>
    ProjectDeploymentsApiKey = 2,
}
