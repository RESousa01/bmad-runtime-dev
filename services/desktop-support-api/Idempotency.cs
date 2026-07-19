using System.Collections.Concurrent;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Sapphirus.DesktopSupportApi;

public interface IIdempotencyStore
{
    Task<T> ExecuteAsync<T>(
        string subject,
        string key,
        string requestFingerprint,
        Func<Task<T>> operation,
        CancellationToken cancellationToken);
}

public sealed class IdempotencyConflictException : Exception
{
}

public sealed class IdempotencyCapacityException : Exception
{
}

public sealed class MemoryIdempotencyStore : IIdempotencyStore
{
    private sealed class Entry(
        string requestFingerprint,
        Lazy<Task<object>> value,
        DateTimeOffset lastAccessedAt)
    {
        public string RequestFingerprint { get; } = requestFingerprint;
        public Lazy<Task<object>> Value { get; } = value;
        public DateTimeOffset LastAccessedAt { get; set; } = lastAccessedAt;
        public bool IsCompleted => Value.IsValueCreated && Value.Value.IsCompleted;
    }

    private readonly object _gate = new();
    private readonly Dictionary<(string Subject, string Key), Entry> _values = new();
    private readonly int _maximumEntries;
    private readonly TimeSpan _retention;
    private readonly TimeProvider _timeProvider;

    public MemoryIdempotencyStore()
        : this(4096, TimeSpan.FromMinutes(15), TimeProvider.System)
    {
    }

    public MemoryIdempotencyStore(
        int maximumEntries,
        TimeSpan retention,
        TimeProvider timeProvider)
    {
        if (maximumEntries < 1)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumEntries));
        }
        if (retention <= TimeSpan.Zero || retention > TimeSpan.FromDays(1))
        {
            throw new ArgumentOutOfRangeException(nameof(retention));
        }
        _maximumEntries = maximumEntries;
        _retention = retention;
        _timeProvider = timeProvider ?? throw new ArgumentNullException(nameof(timeProvider));
    }

    internal int EntryCount
    {
        get
        {
            lock (_gate)
            {
                return _values.Count;
            }
        }
    }

    public async Task<T> ExecuteAsync<T>(
        string subject,
        string key,
        string requestFingerprint,
        Func<Task<T>> operation,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        (string Subject, string Key) entryKey = (subject, key);
        Entry entry;
        bool ownsEntry = false;
        DateTimeOffset now = _timeProvider.GetUtcNow();
        lock (_gate)
        {
            EvictExpiredAndCompleted(now);
            if (_values.TryGetValue(entryKey, out Entry? existing))
            {
                if (!string.Equals(
                    existing.RequestFingerprint,
                    requestFingerprint,
                    StringComparison.Ordinal))
                {
                    throw new IdempotencyConflictException();
                }
                existing.LastAccessedAt = now;
                entry = existing;
            }
            else
            {
                if (_values.Count >= _maximumEntries)
                {
                    throw new IdempotencyCapacityException();
                }
                entry = new Entry(
                    requestFingerprint,
                    new Lazy<Task<object>>(async () =>
                    {
                        T value = await operation().ConfigureAwait(false);
                        return (object?)value
                            ?? throw new InvalidOperationException(
                                "An idempotent operation returned null.");
                    }, LazyThreadSafetyMode.ExecutionAndPublication),
                    now);
                _values.Add(entryKey, entry);
                ownsEntry = true;
            }
        }

        Task<object> task = entry.Value.Value;
        try
        {
            object result = await CancellableOperation
                .WaitAsync(task, cancellationToken)
                .ConfigureAwait(false);
            lock (_gate)
            {
                if (_values.TryGetValue(entryKey, out Entry? current)
                    && ReferenceEquals(current, entry))
                {
                    current.LastAccessedAt = _timeProvider.GetUtcNow();
                }
            }
            return result is T typed
                ? typed
                : throw new InvalidOperationException(
                    "Idempotency key was reused for another response type.");
        }
        catch
        {
            if (task.IsCanceled
                || task.IsFaulted
                || (ownsEntry && cancellationToken.IsCancellationRequested))
            {
                lock (_gate)
                {
                    if (_values.TryGetValue(entryKey, out Entry? current)
                        && ReferenceEquals(current, entry))
                    {
                        _values.Remove(entryKey);
                    }
                }
            }
            throw;
        }
    }

    private void EvictExpiredAndCompleted(DateTimeOffset now)
    {
        foreach ((string Subject, string Key) key in _values
            .Where(pair => pair.Value.IsCompleted
                && now - pair.Value.LastAccessedAt >= _retention)
            .Select(pair => pair.Key)
            .ToArray())
        {
            _values.Remove(key);
        }
    }

}

public sealed record ModelCallCompletionMarker(
    string ReceiptId,
    string RequestHash,
    string ResultHash);

public sealed record ModelCallIdempotencyResult(
    ModelAccessResult? Result,
    ModelCallCompletionMarker? PriorCompletion)
{
    public static ModelCallIdempotencyResult Fresh(ModelAccessResult result) =>
        new(result, null);

    public static ModelCallIdempotencyResult Replay(ModelCallCompletionMarker completion) =>
        new(null, completion);
}

public interface IModelCallIdempotencyStore
{
    Task<ModelCallIdempotencyResult> ExecuteAsync(
        string subject,
        string key,
        string requestFingerprint,
        Func<CancellationToken, Task<ModelAccessResult>> acquireResult,
        Func<ModelAccessResult, CancellationToken, Task<ModelAccessResult>> commitLocalResult,
        CancellationToken cancellationToken);
}

public sealed class MemoryModelCallIdempotencyStore : IModelCallIdempotencyStore
{
    private sealed class Entry(
        string requestFingerprint,
        Task<ModelAccessResult> inFlight,
        DateTimeOffset lastAccessedAt)
    {
        public string RequestFingerprint { get; } = requestFingerprint;
        public Task<ModelAccessResult>? InFlight { get; set; } = inFlight;
        public ModelCallCompletionMarker? Completion { get; set; }
        public DateTimeOffset LastAccessedAt { get; set; } = lastAccessedAt;
    }

    private readonly object _gate = new();
    private readonly Dictionary<(string Subject, string Key), Entry> _entries = new();
    private readonly int _maximumEntries;
    private readonly TimeSpan _retention;
    private readonly TimeProvider _timeProvider;

    public MemoryModelCallIdempotencyStore(
        int maximumEntries,
        TimeSpan retention,
        TimeProvider timeProvider)
    {
        if (maximumEntries < 1)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumEntries));
        }
        if (retention <= TimeSpan.Zero || retention > TimeSpan.FromDays(1))
        {
            throw new ArgumentOutOfRangeException(nameof(retention));
        }
        _maximumEntries = maximumEntries;
        _retention = retention;
        _timeProvider = timeProvider ?? throw new ArgumentNullException(nameof(timeProvider));
    }

    internal int RetainedPayloadTaskCount
    {
        get
        {
            lock (_gate)
            {
                return _entries.Count(pair => pair.Value.InFlight is not null);
            }
        }
    }

    public async Task<ModelCallIdempotencyResult> ExecuteAsync(
        string subject,
        string key,
        string requestFingerprint,
        Func<CancellationToken, Task<ModelAccessResult>> acquireResult,
        Func<ModelAccessResult, CancellationToken, Task<ModelAccessResult>> commitLocalResult,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(acquireResult);
        ArgumentNullException.ThrowIfNull(commitLocalResult);
        cancellationToken.ThrowIfCancellationRequested();
        (string Subject, string Key) entryKey = (subject, key);
        TaskCompletionSource<ModelAccessResult>? owner = null;
        Entry? ownedEntry = null;
        Task<ModelAccessResult> inFlight;
        DateTimeOffset now = _timeProvider.GetUtcNow();
        lock (_gate)
        {
            EvictExpiredMarkers(now);
            if (_entries.TryGetValue(entryKey, out Entry? existing))
            {
                if (!string.Equals(
                    existing.RequestFingerprint,
                    requestFingerprint,
                    StringComparison.Ordinal))
                {
                    throw new IdempotencyConflictException();
                }
                existing.LastAccessedAt = now;
                if (existing.Completion is not null)
                {
                    return ModelCallIdempotencyResult.Replay(existing.Completion);
                }
                inFlight = existing.InFlight
                    ?? throw new InvalidOperationException(
                        "A model-call idempotency entry has no active or completed state.");
            }
            else
            {
                if (_entries.Count >= _maximumEntries)
                {
                    throw new IdempotencyCapacityException();
                }
                owner = new TaskCompletionSource<ModelAccessResult>(
                    TaskCreationOptions.RunContinuationsAsynchronously);
                inFlight = owner.Task;
                ownedEntry = new Entry(requestFingerprint, inFlight, now);
                _entries.Add(entryKey, ownedEntry);
            }
        }

        if (owner is not null)
        {
            try
            {
                Task<ModelAccessResult> acquisitionTask = acquireResult(cancellationToken)
                    ?? throw new InvalidOperationException(
                        "A model-result acquisition returned no task.");
                ModelAccessResult acquired = await CancellableOperation
                    .WaitAsync(acquisitionTask, cancellationToken)
                    .ConfigureAwait(false);
                cancellationToken.ThrowIfCancellationRequested();
                Task<ModelAccessResult> commitTask = commitLocalResult(
                    acquired,
                    cancellationToken)
                    ?? throw new InvalidOperationException(
                        "A local model-result commit returned no task.");
                ModelAccessResult result = await commitTask.ConfigureAwait(false);
                ModelCallCompletionMarker marker = new(
                    result.Receipt.ReceiptId,
                    result.Receipt.RequestHash,
                    result.Receipt.ResultHash);
                lock (_gate)
                {
                    if (_entries.TryGetValue(entryKey, out Entry? current)
                        && ReferenceEquals(current, ownedEntry))
                    {
                        current.InFlight = null;
                        current.Completion = marker;
                        current.LastAccessedAt = _timeProvider.GetUtcNow();
                    }
                    else
                    {
                        throw new InvalidOperationException(
                            "Model-call idempotency ownership was lost before completion.");
                    }
                }
                owner.TrySetResult(result);
                return ModelCallIdempotencyResult.Fresh(result);
            }
            catch (OperationCanceledException exception)
            {
                RemoveOwnedEntry(entryKey, ownedEntry!);
                owner.TrySetCanceled(exception.CancellationToken);
            }
            catch (Exception exception)
            {
                RemoveOwnedEntry(entryKey, ownedEntry!);
                owner.TrySetException(exception);
            }
        }

        ModelAccessResult completed = await CancellableOperation
            .WaitAsync(inFlight, cancellationToken)
            .ConfigureAwait(false);
        return ModelCallIdempotencyResult.Fresh(completed);
    }

    private void RemoveOwnedEntry(
        (string Subject, string Key) entryKey,
        Entry ownedEntry)
    {
        lock (_gate)
        {
            if (_entries.TryGetValue(entryKey, out Entry? current)
                && ReferenceEquals(current, ownedEntry))
            {
                _entries.Remove(entryKey);
            }
        }
    }

    private void EvictExpiredMarkers(DateTimeOffset now)
    {
        foreach ((string Subject, string Key) key in _entries
            .Where(pair => pair.Value.Completion is not null
                && now - pair.Value.LastAccessedAt >= _retention)
            .Select(pair => pair.Key)
            .ToArray())
        {
            _entries.Remove(key);
        }
    }
}
