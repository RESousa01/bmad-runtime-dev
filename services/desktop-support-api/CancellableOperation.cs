namespace Sapphirus.DesktopSupportApi;

internal static class CancellableOperation
{
    public static async Task<T> WaitAsync<T>(
        Task<T> operation,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(operation);
        try
        {
            return await operation.WaitAsync(cancellationToken).ConfigureAwait(false);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            Observe(operation);
            throw;
        }
    }

    public static void Observe(Task operation)
    {
        ArgumentNullException.ThrowIfNull(operation);
        _ = operation.ContinueWith(
            static completed => _ = completed.Exception,
            CancellationToken.None,
            TaskContinuationOptions.OnlyOnFaulted
                | TaskContinuationOptions.ExecuteSynchronously,
            TaskScheduler.Default);
    }
}
