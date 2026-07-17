# Desktop Support API

This directory is an undeployed D2 support-plane implementation seam. No service is called by the
current deterministic/offline desktop path. When production composition is added, the API is intended to
authenticate and license `windows_local` clients, broker transient no-store model calls, and return
signed policy/release metadata. It must never accept local paths, expose file or command routes,
create local proposals, or apply local changes.

The service is single-tenant for internal organization use. `Sapphirus__Authority` must identify the
organization's concrete Entra tenant GUID and `Sapphirus__Audience` must identify its registered API;
`Sapphirus__ApprovedDesktopClientId` must be the one approved desktop application registration.
`common`, `organizations`, consumer, multi-tenant authorities, and tokens whose `azp`/`appid` does
not match that client fail closed. `Sapphirus__ReleaseChannel` accepts only `beta` or `stable`.

The in-memory device, idempotency, receipt, signing, and model implementations are development-only
composition seams. Development signing/model behavior is disabled by default and is rejected outside
the ASP.NET Development environment. Production composition must replace them with Azure SQL,
managed-identity Foundry access, and purpose-specific HSM signing providers.

Device registrations are tenant-and-subject partitioned and revocation is retained for the process
lifetime. The request now carries the canonical device-signed consent envelope and the response uses
the canonical support-plane receipt shape, including exact request, manifest, consumption, profile,
deployment, schema, region, retention, usage, and proof bindings. The default consent verifier still
returns `consent_binding_unavailable`: production composition must provide installation-key proof
verification and a durable single-use consumption store before any model broker can be reached.
Development-only proof material is structural test evidence and is not a production signature.
The development broker returns unsigned completion evidence to a separate receipt signer; production
composition must preserve that separation when replacing both adapters.

For local restart/replay testing only, `Sapphirus__DevelopmentConsentStorePath` may name a fully
qualified directory while the host runs in `Development`. The adapter atomically creates one marker
per subject, registration, and consumption hash, stores only a subject hash plus bounded authority
metadata, and treats interrupted writes as consumed. The setting is rejected outside Development;
production still requires a transactional shared store.
