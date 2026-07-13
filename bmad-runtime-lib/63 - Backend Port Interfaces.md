---
title: "Backend Port Interfaces"
aliases:
  - "63 - Backend Port Interfaces"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 63
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: backend-port-reference
status: implementation-guide
---



# Backend Port Interfaces

## V6.17 port families

C# ports implement `web_managed` authority and Azure desktop support services. Rust traits implement local workspace, lifecycle, Airlock, model-access client, execution, checkpoint, evidence, recovery, and sync-export boundaries. Only schema-generated value types and conformance fixtures cross languages; ports that mutate state or effects do not.

Required Rust trait seams include `WorkspaceGrantStore`, `WorkspaceReader`, `ContextPackBuilder`, `LocalAirlock`, `PatchJournal`, `CheckpointStore`, `LocalCommandRunner`, `ExecutionResultImporter`, `EvidenceLedger`, `ModelAccessClient`, `PackageVerifier`, and `SyncEnvelopeExporter`. The TypeScript renderer sees only a generated `DesktopRuntimeFacade`, never these internal traits or generic Tauri plugins.

## 1. Purpose

This file turns the “modular monolith with strict internal contracts” rule into implementation-level C# interface boundaries. These ports are intentionally service-like even when implemented in-process.

## 2. Port rules

- Ports live in `Runtime.Application` or the owning domain package, not in infrastructure.
- Infrastructure implements ports; application services consume ports.
- A module cannot import another module's EF repository directly.
- Cross-module access happens through ports or domain events.
- Every method that mutates lifecycle state requires actor, correlation ID, expected state, and idempotency key where applicable.

## 3. `IRunStateStore`

```csharp
public interface IRunStateStore
{
    Task<RunAggregate> LoadForUpdateAsync(RunId runId, CancellationToken ct);
    Task<RunTransitionResult> CommitTransitionAsync(
        RunTransitionCommand command,
        EvidenceLedgerEventDraft evidence,
        IReadOnlyList<OutboxMessageDraft> deliveries,
        CancellationToken ct);
}
```

Implementation requirements:

- Uses one transaction for transition + Evidence Ledger append + outbox enqueue.
- Rejects stale rowversion.
- Rejects invalid current→next state.
- Returns idempotent result for duplicate command ID.

## 4. `IAirlockPolicy`

```csharp
public interface IAirlockPolicy
{
    Task<PolicyDecision> EvaluateAsync(ExecutionSpecCandidate candidate, PolicyContext context, CancellationToken ct);
    Task<ApprovalRequirement> CreateApprovalRequirementAsync(PolicyDecision decision, CancellationToken ct);
    Task<ApprovedExecutionSpec> IssueSpecAsync(
        ExecutionSpecCandidate candidate,
        PolicyDecision decision,
        ApprovalDecision? exactCandidateApproval,
        CancellationToken ct);
    Task<SpecConsumptionResult> ValidateAndConsumeAsync(
        ApprovedExecutionSpec spec,
        ExecutorAudience audience,
        MutableInputSnapshot currentInputs,
        CancellationToken ct);
}
```

Implementation requirements:

- Pure policy evaluation; no workspace mutation or execution dispatch.
- Fails closed if policy version cannot load.
- Every spec binds candidate/proposal/approval/policy hashes, actor/owner, audience/fixed template, all mutable inputs, issue/expiry, limits, and single-use nonce.
- Ordinary authenticated CRUD and offline Source Intake do not call this port unless they request a governed worker/external/workspace mutation.

## 5. `IWorkspaceSnapshotStore`

```csharp
public interface IWorkspaceSnapshotStore
{
    Task<WorkspaceSnapshot> CreateSnapshotAsync(CreateSnapshotCommand command, CancellationToken ct);
    Task<WorkspaceFileTree> GetTreeAsync(SnapshotId snapshotId, CancellationToken ct);
    Task<IReadOnlyList<PreimageHash>> CapturePreimagesAsync(SnapshotId snapshotId, IReadOnlyList<WorkspacePath> paths, CancellationToken ct);
    Task<PreimageVerification> VerifyPreimagesAsync(IReadOnlyList<PreimageHash> expected, CancellationToken ct);
    Task<Checkpoint> RecordCheckpointAsync(CheckpointCommand command, CancellationToken ct);
}
```

Implementation requirements:

- Snapshot archive is immutable.
- File manifest records canonical path, hash, size, media type, secret status, ignored status.
- Symlink traversal and path casing collisions are blocked.

## 6. `IExecutionDispatcher`

```csharp
public interface IExecutionDispatcher
{
    Task<ExecutionRecord> DispatchAsync(ApprovedExecutionSpec spec, CancellationToken ct);
    Task<ExecutionRecord> CancelAsync(ExecutionId executionId, Actor actor, CancellationToken ct);
    Task<ManifestImportResult> ImportManifestAsync(WorkerResultManifestImport command, CancellationToken ct);
}
```

Implementation requirements:

- Dispatch accepts only `ApprovedExecutionSpec`.
- Dispatcher maps the spec to an allowlisted fixed ACA template and cannot override image, entrypoint, identity, secrets, network, or arbitrary environment.
- Worker image digest must have remote-build lock/license/scan/SBOM/provenance/signature evidence.
- Manifest import validates candidate/spec/policy/approval/audience/attempt/lease/template/image/workspace/mutable-input/output/completion hashes and is idempotent.
- Worker cannot directly change SQL state.

## 7. `IModelGateway`

```csharp
public interface IModelGateway
{
    Task<ModelOutput<T>> CompleteStructuredAsync<T>(
        StructuredModelRequest request,
        ModelProfileRef profile,
        CanonicalJsonSchema schema,
        CancellationToken ct);
}
```

Implementation requirements:

- Returns typed model output, not platform `Proposal`.
- Resolves exact deployment capabilities, parsed-HTTPS credential binding, retention, and provider schema projection for every call.
- Sends Responses with `store=false` and hosted tools disabled in the baseline; application state remains authoritative.
- Stores redacted request/response summary, canonical/projected schema hashes, typed outcome, evaluation/profile version, and usage/cost.
- Refusal, incomplete, content-policy, capability, credential, schema, timeout, rate-limit, and provider failures return typed results; no proposal is created.

## 8. `IEvidenceLedger`, `IEvidenceMaterializer`, and `ITraceWriter`

```csharp
public interface IEvidenceLedger
{
    Task<IReadOnlyList<EvidenceLedgerEvent>> ReadAsync(
        StreamId streamId,
        long afterSequence,
        CancellationToken ct);
}

public interface IEvidenceMaterializer
{
    Task<EvidenceBundle> MaterializeAsync(
        EvidenceStreamRange range,
        EvidenceRequest request,
        CancellationToken ct);
}

public interface ITraceWriter
{
    Task AppendAsync(TraceEventEnvelope envelope, CancellationToken ct);
    Task<PayloadRef> WritePayloadAsync(TracePayload payload, RetentionClass retention, CancellationToken ct);
}
```

Implementation requirements:

- The Runtime transaction writes authoritative evidence; `IEvidenceLedger` is read-only outside that transaction owner.
- SQL stores compact trace indexes as rebuildable projections.
- Blob stores large/redacted payloads.
- Privileged raw payload access is separate and audited.
- Trace loss/sampling cannot change lifecycle state or EvidenceBundle contents.

## 9. `IWorkStore`

```csharp
public interface IWorkStore
{
    Task<WorkAttempt> CreateAttemptAsync(WorkItemId workItemId, CancellationToken ct);
    Task<WorkLeaseResult> TryAcquireOrRenewLeaseAsync(WorkLeaseCommand command, CancellationToken ct);
    Task<CompletionImportResult> CommitCompletionAsync(
        WebWorkerResultManifest manifest,
        CancellationToken ct);
}
```

`CommitCompletionAsync` atomically validates/records immutable `WorkCompletion`, authoritative lifecycle transition, `EvidenceLedgerEvent`, and `OutboxMessage`. Crash/redelivery imports the same completion nonce and never re-executes the consumed spec.

## 10. Provider and model-evaluation ports

```csharp
public interface IProviderCapabilityResolver
{
    Task<ProviderCapabilities> ResolveAsync(ModelProfileRef profile, CancellationToken ct);
}

public interface IProviderSchemaProjector
{
    ProviderSchemaProjection Project(CanonicalJsonSchema schema, ProviderCapabilities capabilities);
}

public interface IModelEvaluationGate
{
    Task<ModelProfilePromotionResult> EvaluatePromotionAsync(
        ModelProfileCandidate candidate,
        ModelEvaluationBundle bundle,
        CancellationToken ct);
}
```

Promotion requires immutable contract, task-quality, safety/privacy, and operations results, then policy/canary/rollback evidence. No model approves its own profile and no fallback crosses provider/credential/residency/retention/tool/schema/material-quality boundaries silently.

## 11. `ISourceIntake`

```csharp
public interface ISourceIntake
{
    Task<SourceSnapshot> AcquireAsync(SourceAcquisitionRequest request, CancellationToken ct);
    Task<SourceVerificationRecord> VerifyAsync(SourceSnapshotId snapshotId, CancellationToken ct);
    Task<ComponentLicenseDecision> DecideComponentLicenseAsync(
        ComponentLicenseDecisionCommand command,
        CancellationToken ct);
}
```

Source Intake never imports executable code into the Runtime API or activates a package. Promotion requires immutable identity where policy demands it, safe extraction/inventory, every component license/notice decision, copied/derived map, and fixture/dependency provenance.
