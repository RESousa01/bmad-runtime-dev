# Foundry connection baseline

Date: 2026-07-28

## Frozen inputs

- Repository: `C:\Users\rodri\source\bmad-runtime-dev`
- Branch: `main`
- Authoritative pre-implementation commit:
  `7956da370b0f9f16c5aea3b21f6e39383fa3171d`
- Authoritative tree:
  `e5943003ebd3c5a3c37b914ca18a61611f0043eb`
- Plan archive:
  `C:\Users\rodri\Downloads\forge-foundry-opencode-connection-plan.zip`
- Plan archive SHA-256:
  `5d024e69957ce01160e3f72ab31074b99ecd591a316a290dfaf9a64cc6c74bc1`
- Binding implementation-plan SHA-256:
  `07ef06a719a5ecd9a4b044b00bd58d7ace43ea9d7407973c88456461391e5348`
- Package integrity: all 13 files listed by
  `evidence/package-file-checksums.txt` matched.
- Package self-verification: 12 of 12 checks report passed.

The checkout is shared with another active agent. The FND-001 lease is
restricted to this document, `.foundry-connection/run-state.json`, and
`artifacts/foundry/{baseline,tasks}`. All other dirty or newly appearing paths
are externally owned and protected.

## Source-package alignment

The package source map has 19 directly mappable Forge project files. At
preflight, 18 matched byte-for-byte. The committed
`apps/desktop-ui/src/components/SessionContextPanel.tsx` had drifted from the
package snapshot; that drift is preserved and hashed in the fresh source map.

The package records an `opencode-dev.zip` archive but does not include it. The
available clean reference checkout is:

- Path: `C:\tmp\forge-v2\library\opencode`
- Commit: `0a601cf334b9a83cc2854108a2b860f25e6e7e8e`
- Status: clean

Its four reference-file hashes differ from the package source-map hashes. Both
sets are retained in `artifacts/foundry/baseline/source-map.json`; neither is
silently substituted for the other.

## Current managed provider flow

1. `ProductionOptions` accepts one `ModelEndpoint`, one `ModelDeployment`, and
   configured provider/model/capability hashes. Endpoint validation permits
   `.openai.azure.com` and `.cognitiveservices.azure.com` roots.
2. `AzureClientRegistration` creates one `AzureOpenAIClient` using the
   production model credential.
3. `ModelAccessProfile` resolves one server-owned profile. It requires the
   configured deployment to be in the signed policy's
   `approvedModelDeployments` list and fixes the purpose/role to
   `bmad_help`/`planner`.
4. `AzureOpenAiModelAccessBroker` creates a deployment-specific
   `ChatClient` with `GetChatClient`, calls `CompleteChatAsync`, requests a
   strict JSON-schema response, disables stored output, validates the canonical
   result, and signs a content-bound receipt.
5. `POST /desktop/v1/model-access/calls` is the only managed model route.
   There is no effective model-catalog endpoint.
6. Bicep creates an Azure OpenAI account and a fixed `desktop-planning`
   deployment, then injects `Sapphirus__ModelEndpoint` and
   `Sapphirus__ModelDeployment`.

## Current desktop and renderer flow

- Rust exposes fixed support-stage routes and verifies the V1 request, consent,
  result, and receipt bindings.
- Production BMAD Help projects a fixed `azure-openai-fixed-profile` and
  `desktop-planner`; direct OpenAI/Azure Responses providers remain explicitly
  development-only.
- The renderer derives `azure`/`openai` labels from local auth state and uses
  `gpt-5-mini` as a fallback model identifier.
- `AgentSelector` receives a single model label/detail rather than a catalog.
- `SessionContextPanel` displays provider/model strings, but those values are
  not yet effective-catalog or receipt authoritative.

## OpenCode reference seams

The pinned OpenCode checkout is an architecture reference only:

- `ModelsDev.Service` supplies catalog metadata and refresh behavior.
- central `Provider.Service` merges configuration, authentication, filtering,
  model metadata, defaults, and recent selection;
- the Azure provider plugin isolates provider-specific client construction;
- the provider HTTP handler projects one client list;
- SDK/model instances are cached behind the provider service;
- configuration/auth overlays and allow/deny filters shape the effective list.

Forge does not adopt Models.dev as deployment truth, renderer-held provider
credentials, dynamic provider packages, deployment-name-equals-model-name
assumptions, or direct desktop Foundry calls.

## Intentional behavior changes

- Replace the fixed Azure OpenAI account/deployment configuration with one
  complete Microsoft Foundry project endpoint.
- Discover actual deployments and merge them with signed organization profiles,
  policy, entitlement, qualification, region, and health.
- Persist stable profile/variant IDs; keep endpoint, deployment, credentials,
  Azure resource IDs, and identity IDs server-only.
- Replace managed Chat Completions with the Foundry project Responses API.
- Add normalized streaming for ordinary chat while retaining structured BMAD
  and Builder output validation.
- Extend consent and signed receipts to bind profile, catalog, current route,
  provider metadata, usage provenance, and terminal state.
- Keep `store=false`, explicit selection, no automatic model fallback, and the
  existing device, policy, idempotency, recovery, and local-authority controls.

## Characterization and acceptance

FND-001 changes no runtime behavior, so its red signal is the pre-change
characterization itself. Exact commands and classifications are recorded in
`artifacts/foundry/baseline/test-baseline.json`. One valid V1 call fixture is
captured in `model-call-fixture.json`; current safe failures are enumerated in
`safe-failure-outcomes.json`.

The baseline is evidence for migration comparisons. It is not Azure,
deployment, release, or pilot qualification.
