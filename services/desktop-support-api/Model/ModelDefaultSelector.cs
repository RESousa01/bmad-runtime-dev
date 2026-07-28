namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// Deterministic default-profile selection.
///
/// <para>
/// The order is: the explicit organization default, then the caller's
/// last-valid user selection, then the deterministic organization order —
/// ascending <c>priority</c>, ties broken on ordinal profile ID. Every step
/// draws only from the set that is selectable right now, so a blocked profile
/// can never become the default. That is the whole point: silently defaulting
/// to a profile the user is not entitled to, or that the deployment no longer
/// backs, is how someone ends up talking to a different model than the one the
/// screen says.
/// </para>
/// <para>
/// There is no fallback to a different family, a cheaper profile, or "the
/// closest thing available". When nothing is selectable the answer is null and
/// the desktop is told why through <c>unavailableProfiles</c>.
/// </para>
/// </summary>
public static class ModelDefaultSelector
{
    /// <summary>
    /// The organization's deterministic listing order. It is stable across
    /// refreshes, so the picker does not reshuffle when an unrelated profile
    /// changes.
    /// </summary>
    public static IReadOnlyList<ModelProfile> Order(IEnumerable<ModelProfile> profiles)
    {
        ArgumentNullException.ThrowIfNull(profiles);
        return
        [
            .. profiles
                .OrderBy(static profile => profile.Priority)
                .ThenBy(static profile => profile.ProfileId, StringComparer.Ordinal),
        ];
    }

    /// <summary>
    /// Chooses the default from <paramref name="selectable"/>, which must
    /// already exclude every blocked profile.
    /// </summary>
    /// <param name="selectable">Profiles the subject may invoke right now.</param>
    /// <param name="organizationDefaultProfileId">
    /// The organization's explicit default, or null when it published none.
    /// </param>
    /// <param name="lastValidUserSelectionProfileId">
    /// The caller's last known-good selection, supplied as a hint. It is only
    /// ever honoured when it is still selectable.
    /// </param>
    public static string? Select(
        IReadOnlyList<ModelProfile> selectable,
        string? organizationDefaultProfileId,
        string? lastValidUserSelectionProfileId)
    {
        ArgumentNullException.ThrowIfNull(selectable);
        if (selectable.Count == 0)
        {
            return null;
        }
        HashSet<string> available =
            new(selectable.Select(static profile => profile.ProfileId), StringComparer.Ordinal);

        if (organizationDefaultProfileId is not null
            && available.Contains(organizationDefaultProfileId))
        {
            return organizationDefaultProfileId;
        }
        if (lastValidUserSelectionProfileId is not null
            && available.Contains(lastValidUserSelectionProfileId))
        {
            return lastValidUserSelectionProfileId;
        }
        return Order(selectable)[0].ProfileId;
    }
}
