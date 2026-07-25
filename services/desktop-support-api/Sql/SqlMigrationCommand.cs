namespace Sapphirus.DesktopSupportApi.Sql;

internal enum SqlMigrationCommandSelection
{
    WebServer,
    MigrateSql,
    Invalid,
}

/// <summary>
/// Selects and executes the one-shot production schema migration command.
/// The command is deliberately separate from web-host startup: every handled
/// outcome returns an exit code and the caller must return immediately.
/// </summary>
internal static class SqlMigrationCommand
{
    internal const string Name = "migrate-sql";
    internal const int InvalidArgumentsExitCode = 64;
    private const int FailureExitCode = 1;

    internal static SqlMigrationCommandSelection Select(IReadOnlyList<string> arguments)
    {
        ArgumentNullException.ThrowIfNull(arguments);
        if (arguments.Count == 0)
        {
            return SqlMigrationCommandSelection.WebServer;
        }
        if (string.Equals(arguments[0], Name, StringComparison.Ordinal))
        {
            return arguments.Count == 1
                ? SqlMigrationCommandSelection.MigrateSql
                : SqlMigrationCommandSelection.Invalid;
        }
        if (!arguments[0].StartsWith("-", StringComparison.Ordinal)
            || arguments.Any(static argument =>
                argument.Contains("migrate", StringComparison.OrdinalIgnoreCase)))
        {
            return SqlMigrationCommandSelection.Invalid;
        }
        return SqlMigrationCommandSelection.WebServer;
    }

    internal static async Task<int> ExecuteAsync(
        IHostEnvironment environment,
        Func<CancellationToken, Task> applyMigrations,
        TextWriter error,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(environment);
        ArgumentNullException.ThrowIfNull(applyMigrations);
        ArgumentNullException.ThrowIfNull(error);
        if (!environment.IsProduction())
        {
            await error.WriteLineAsync(
                "SQL migration refused: ASPNETCORE_ENVIRONMENT must be Production.");
            return FailureExitCode;
        }
        try
        {
            await applyMigrations(cancellationToken).ConfigureAwait(false);
            return 0;
        }
        catch (Exception)
        {
            await error.WriteLineAsync(
                "SQL migration failed; no web server was started.");
            return FailureExitCode;
        }
    }
}
