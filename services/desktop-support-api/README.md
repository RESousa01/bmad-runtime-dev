# Desktop Support API

This directory is a frozen, non-integrated D2 support-plane scaffold. No service is deployed or
called by the current D1 desktop slice. When the lane is explicitly resumed, the API is intended to
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
lifetime. The current request contract carries only an opaque consent receipt hash and omits the
signed receipt fields needed to bind model, deployment, region, and profile. Consequently the
default consent verifier returns `consent_binding_unavailable` and model calls do not reach a broker.
This is intentional until the canonical contract and a device-bound verifier are implemented.
