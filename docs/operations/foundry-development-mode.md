# Microsoft Foundry development credential mode

Date: 2026-07-28

**This is not a production path.** It cannot become one, and a run in this mode
does not satisfy any deployed qualification gate. Read
[What a run in this mode proves](#what-a-run-in-this-mode-proves) before citing
any evidence produced here.

## Why this mode exists

The operator has neither Entra app-registration rights nor Azure RBAC
role-assignment rights. The managed-identity path is therefore *unobtainable*,
not merely unsupplied: there is no sequence of values anyone can type that
produces a user-assigned identity attached to a Container App with a Foundry
data-plane role assignment. Without an alternative, nothing in this repository
could ever reach the real Foundry project.

This mode swaps exactly one thing: the credential mechanism. Deployment
discovery, the deployment normalizer, the profile registry, the effective
catalog, route resolution, the Responses adapter, output validation, and signed
receipts all stay exactly as designed and are exercised unchanged. The
managed-identity path is still built and remains the only production path.

The swap is viable because an API key in an `api-key` request header
authenticates both surfaces the support plane uses. Measured against the real
project:

| Surface | Request | Result |
|---|---|---|
| Project deployments | `GET <projectEndpoint>/deployments?api-version=v1` | `200` |
| Responses | `POST <projectEndpoint>/openai/v1/responses` with an empty body | `400`, that is, authentication passed and the body was rejected |

No declared-inventory fallback is needed. The catalog is built from a real
discovered inventory, exactly as in production.

The header is `api-key`. It is deliberately **not** `Authorization: Bearer` —
that is the managed-identity mechanism's header and the measured result used
`api-key`.

## What the operator sets

Two environment variables, both in the `Sapphirus` configuration section.

| Environment variable | Configuration key | Value |
|---|---|---|
| `Sapphirus__FoundryProjectEndpoint` | `Sapphirus:FoundryProjectEndpoint` | The complete project endpoint, `https://<account>.services.ai.azure.com/api/projects/<project>`. The documented `_project` alias is accepted. |
| `Sapphirus__FoundryDevelopmentApiKey` | `Sapphirus:FoundryDevelopmentApiKey` | The project API key. |

```powershell
$env:ASPNETCORE_ENVIRONMENT = "Development"
$env:Sapphirus__FoundryProjectEndpoint = "<complete project endpoint>"
$env:Sapphirus__FoundryDevelopmentApiKey = "<project api key>"
dotnet run --project services/desktop-support-api
```

The endpoint is validated by the same validator Production uses,
`ProductionOptions.TryValidateFoundryProjectEndpoint`. Development does not get
a looser endpoint rule: mixed case, percent-encoding, dot-segment traversal, an
explicit `:443`, a query, a fragment, a punycode or Unicode homograph host, and
any extra trailing path are all rejected here too. See
[`foundry-configuration.md`](foundry-configuration.md) for the full rejection
table.

`appsettings.Development.json` names both keys with **empty** values. That is
the whole of its content for this feature. The API key must never appear in a
committed `appsettings*.json` with a real value; a test enumerates every
`appsettings*.json` in the service and asserts the configured value is empty.

### Both values, or neither

`TryAddDevelopmentFoundryCatalog` treats the pair as all-or-nothing.

- Neither set: the mode was not requested. Nothing is registered, the host runs
  exactly as it did before, and the caller is told the mode is off.
- Both set: the mode is composed.
- Exactly one set: `development_foundry_configuration_incomplete`. There is no
  path where a half-configured development mode silently produces
  unauthenticated requests.

A blank, over-long, or non-printable key is rejected at composition with
`development_foundry_api_key_invalid`, not at the first provider call. The key
must be bounded printable ASCII, which is what stops a key carrying a line
break from splitting the request or making the header library raise an
exception that quotes the key back.

## It cannot run in Production, by construction

Four independent guards, any one of which is sufficient.

1. **The authenticator's constructor.** `DevelopmentFoundryApiKeyAuthenticator`
   throws `development_foundry_credential_environment_forbidden` unless the
   environment name is exactly `Development` or `Test`. This mirrors the fake
   Foundry server's `fake_foundry_environment_forbidden` precedent: even an
   accidental reference cannot activate it.
2. **The composition entry point.** `AddDevelopmentFoundryCatalog` refuses the
   same way, with `development_foundry_composition_environment_forbidden`,
   before it constructs anything.
3. **The production graph never registers it.** `ProductionComposition` and
   `AzureClientRegistration` bind `IFoundryRequestAuthenticator` to
   `ManagedIdentityFoundryAuthenticator` and nothing else.
4. **A structural assertion.** `ProductionComposition` scans its own service
   descriptors and throws
   `production_foundry_development_credential_forbidden` if a development
   authenticator is present under any registration shape. The development
   composition registers the concrete type as well as the seam precisely so
   this scan can see it without building the container.

FND-065's acceptance criterion survives unchanged: there is still no production
API-key path.

## Where the key is, and is not

The key is read from configuration once, at composition, and held in a private
field of one singleton. It is never logged, never placed in telemetry, never
put in a problem document, never returned in a health response, and never
placed in a receipt.

- `CredentialMechanismId` is the only public surface of the authenticator, and
  it returns `development_api_key` — the mechanism name, never the credential.
- `ToString()` is overridden on both authenticators to return the mechanism
  name.
- Discovery evidence already carries only a timestamp, a page count, a byte
  count, and request IDs.
- The health renderer emits `{status, dependencies:[{dependency, status}]}` and
  nothing else.

A test serializes the authenticator, the catalog status, every service
descriptor instance, and the rendered health response, and asserts the key
value is absent from all of them.

## Credential mechanism in evidence

`IFoundryRequestAuthenticator.CredentialMechanismId` carries exactly two
values:

| Value | Mechanism |
|---|---|
| `managed_identity` | Production. An Entra token for `https://ai.azure.com/.default`, acquired once per operation, sent as `Authorization: Bearer`. |
| `development_api_key` | Development and Test only. The project API key, sent in `api-key`. |

`FoundryDeploymentDiscoveryClient.CredentialMechanismId` exposes the mechanism
the client was bound to.

**Required follow-up, FND-030.** The mechanism identifier must be folded into
the receipt's credential-binding hash, so a receipt produced under the
development API key can never be mistaken for one produced under managed
identity. That change is not in this extension's scope and the signing code was
not touched. See [Follow-ups](#follow-ups).

## What a run in this mode proves

**Proved:**

- The project endpoint is real, correctly shaped, and reachable.
- The real deployment inventory is returned, paginated within the project, and
  normalizes into the canonical `sapphirus.foundry-deployment.v1` contract.
- The catalog cache, its staleness policy, its background refresh, and its
  health check behave against a real provider rather than a fake.
- The real response shapes the normalizer must handle, including fields the
  fixtures could only guess at.

**Not proved, and not provable here:**

- Managed-identity authentication. It is not exercised at all in this mode.
- Identity attachment to a Container App.
- Any Foundry data-plane role assignment.
- Private-link or network egress posture.
- Anything about the deployed support plane. This runs on a development
  machine.

Consequently:

- The prerequisite checker (`artifacts/foundry/prerequisites.json`) can never
  reach `"ready": true` from this mode, and must not be reported as if it had.
- FND-002 stays blocked. FND-064 (deployed smoke) and FND-065 (final
  certification) stay blocked. The deployed posture requires Entra and RBAC
  regardless of this extension.
- No scorecard may emit `complete=true` on the strength of a development-mode
  run.
- Substituting a development-mode result for real deployed proof is prohibited
  by the same policy that prohibits substituting fake-server evidence.

The desktop-to-support-API leg used to be described here as unaffected by
this extension, needing an app registration for production. That is no
longer accurate for this deployment: see
[Static desktop token authentication](#static-desktop-token-authentication)
below for the inbound mechanism this operator uses instead of Entra.

## Follow-ups

1. **Host wiring.** `Program.cs` must call the composition inside its existing
   `builder.Environment.IsDevelopment()` branch:

   ```csharp
   builder.Services.TryAddDevelopmentFoundryCatalog(
       builder.Environment,
       builder.Configuration);
   ```

   Until that line exists, the development catalog is composable but not
   composed by the host, and the two environment variables have no effect on
   `dotnet run`. The call is safe to add unconditionally: with neither variable
   set it registers nothing and returns `false`.

2. **FND-030 credential binding.** Fold `CredentialMechanismId` into the
   receipt's credential-binding hash.

## Static desktop token authentication

Date: 2026-07-29

**This is a production mechanism, not a development stand-in.** Unlike the
Foundry credential above, it is not restricted to Development or Test, and a
run under it is a real production run of the desktop-facing surface, subject
to the same caveats an operator with no Entra rights already accepts for
everything else in this repository.

### Why this mechanism exists

`Program.cs` registered desktop authentication unconditionally against Entra:

```csharp
.AddAuthentication(JwtBearerDefaults.AuthenticationScheme)
.AddJwtBearer(jwt => { jwt.Authority = options.Authority; jwt.Audience = options.Audience; ... })
```

and every `/desktop/v1` route requires
`ScopeAuthorization.IsApprovedDesktopPrincipal`, which demands a real Entra
`tid`/`azp` claim shape. The operator running this service has no Entra app
registration for it and will not get one: this is an internal application,
and the managed-identity precedent above (RBAC role assignment) does not
apply to app registrations, which require a different kind of directory
access this operator also lacks. Without an alternative, nothing could ever
call `/desktop/v1` at all.

The static token is the desktop-side mirror of the Foundry API key above: one
operator-supplied credential stands in for the mechanism the operator cannot
obtain. The two differ in one important way. The Foundry key is restricted to
Development and Test because a real managed identity is *installed in
production by the platform*, so there is no reason a production process would
ever need the substitute. The static desktop token has no such alternative
waiting in production: this operator will never hold Entra app-registration
rights for this application in any environment, so the substitute has to be
usable in production, and is designed to be exactly that — explicit,
auditable, and no weaker than the path it replaces.

### It is opt-in and loud, in every environment

`Program.cs` always registers `AddJwtBearer`, in every environment,
unconditionally and unchanged — it remains the default authentication path.
`SupportPlaneOptions.StaticDesktopTokenEnabled` (config key
`Sapphirus:StaticDesktopTokenEnabled`) defaults to `false`. With it `false`,
nothing about authentication differs from before this feature existed. Only
when an operator explicitly sets it to `true` does
`Sapphirus.DesktopSupportApi.Security.DesktopAuthenticationComposition`
additionally register the static-token scheme and make it the *default*
scheme (the one selected when a route names no scheme explicitly, which every
`/desktop/v1` route relies on, since they authorize by named scope policy
rather than by scheme).

There is no environment gate around this flag: it can be `true` in
Production, because that is precisely the case this mechanism exists for.

An active run logs the fact once, at startup, and nothing else:

```
warn: Static desktop token authentication is active. Sapphirus:StaticDesktopTokenEnabled=true replaces Entra JWT validation as the desktop authentication mechanism for this run.
```

### What the operator sets

| Environment variable | Configuration key | Value |
|---|---|---|
| `Sapphirus__StaticDesktopTokenEnabled` | `Sapphirus:StaticDesktopTokenEnabled` | `true` to activate the mechanism. Omit or `false` for Entra-only, the default. |
| `Sapphirus__StaticDesktopToken` | `Sapphirus:StaticDesktopToken` | The bearer token the desktop client must present as `Authorization: Bearer <token>`. Required when the flag is `true`; ignored otherwise. |
| `Sapphirus__StaticDesktopSubjectId` | `Sapphirus:StaticDesktopSubjectId` | Optional. A `Guid` in `"D"` format to use as the principal's stable subject. If omitted, the subject is derived deterministically from the token itself (see below). |

`Sapphirus:Authority`, `Sapphirus:Audience`, and
`Sapphirus:ApprovedDesktopClientId` are still required regardless of this
flag — `AddJwtBearer` is always registered and needs them, and the static
principal's `tid`/`azp` claims are built from the same
`SupportPlaneOptions.TenantId`/`ApprovedDesktopClient` values so that
`ScopeAuthorization.IsApprovedDesktopPrincipal` runs unmodified against
either mechanism. An operator with no Entra tenant at all still names *some*
syntactically valid tenant GUID, audience, and client GUID here; they anchor
the authorization check and the rate-limit/idempotency partitioning, and
never need to correspond to a real, reachable Entra tenant when the static
mechanism is what's actually authenticating requests.

A complete local run with no Entra access at all:

```powershell
$env:ASPNETCORE_ENVIRONMENT = "Development"
$env:Sapphirus__Authority = "https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0"
$env:Sapphirus__Audience = "api://22222222-2222-2222-2222-222222222222"
$env:Sapphirus__ApprovedDesktopClientId = "33333333-3333-3333-3333-333333333333"
$env:Sapphirus__StaticDesktopTokenEnabled = "true"
$env:Sapphirus__StaticDesktopToken = "<a long, random, operator-generated token>"
dotnet run --project services/desktop-support-api
```

The desktop client then presents `Authorization: Bearer <that same token>` on
every `/desktop/v1` request in place of an Entra-issued JWT.

### The token is a production credential

Because this can be the *only* thing authenticating every request in a real
deployment, it is held to a materially higher bar than the outbound
development Foundry key:

- **Minimum length 32**, maximum 512, bounded printable ASCII (mirroring the
  Foundry key's character rule, for the same reason: a token carrying a space,
  control character, or line break could otherwise split the request or make
  the header library raise an exception that quotes the token back).
- **Denylisted placeholder values** (`changeme`, `token`, `test`, `password`,
  `secret`, `default`, `admin`, `placeholder`, and variants) are refused even
  if they happened to be padded to a legal length, and a token consisting of
  one character repeated across its whole length is refused outright.
- **Fixed-time comparison.** `CryptographicOperations.FixedTimeEquals` compares
  the presented token to the configured one; a length mismatch is rejected
  before that comparison (an unavoidable precondition of the API, and it
  leaks nothing about the token's content, only that the lengths differed).
- **Fails closed at composition, not at the first request.** A missing,
  malformed, or denylisted token throws `static_desktop_token_invalid`
  immediately when `Sapphirus:StaticDesktopTokenEnabled` is `true`, during
  host startup (`DesktopAuthenticationComposition.AddDesktopAuthentication`
  constructs `StaticDesktopTokenAuthenticator` eagerly, not behind a lazily
  resolved factory). A host that fails to start is far easier to diagnose
  than one that starts and answers every request `401`.
- **Never logged, never in telemetry, never in a problem document, never in
  a health response, never in a receipt.** The authentication failure message
  is a fixed string (`"The static desktop token was not accepted."`) that
  never quotes the presented or configured value.

### The subject, and why it still works

The rate limiter partition, `IIdempotencyStore.ExecuteAsync(subject, key, …)`,
per-subject consent consumption, `IDeviceRegistry.FindActiveAsync(subject,
registrationId)`, and the `SubjectHash` a receipt is checked against in
`ModelResultGuards` all depend on a stable subject claim. None of that code
was touched. Instead, the static principal is shaped to satisfy it exactly as
a real Entra principal would:

- If `Sapphirus:StaticDesktopSubjectId` is configured, that GUID is used as
  the `oid` claim verbatim.
- Otherwise, the subject is derived as `"static-" +`
  `SHA256("sapphirus:static-desktop-subject:v1\n" + token)[..32]` (hex). This
  is **one-way** — the token cannot be recovered from it — and **stable**
  across restarts, because it depends only on the configured token, which
  does not change between restarts of the same deployment. The `static-`
  prefix is deliberate: the derived subject can never be mistaken for a real
  Entra `oid` (which is always a bare GUID), which is a second, independent
  signal alongside the marker claim below.

### The principal is identifiable as static-token-authenticated

Every principal this mechanism builds carries
`sapphirus_static_token_principal = "true"`, a claim a real Entra token never
has. Combined with the never-GUID-shaped derived subject above, evidence
produced while this mechanism is active — logs, or anything that inspects the
authenticated `ClaimsPrincipal` directly — is never mistakable for a real
Entra principal.

**Named gap.** Unlike the outbound Foundry credential, whose
`CredentialMechanismId` is folded directly into the receipt's
`CredentialBindingHash` (FND-030), there is no equivalent seam on the inbound
side today. A model-access receipt's `TenantHash` and `SubjectHash` are opaque
SHA-256 digests of the tenant and subject (see
`ScopeAuthorization.SubjectPartition`), computed identically regardless of
which authentication mechanism produced the underlying principal. Folding the
inbound mechanism into that hash, or into a new receipt field, was not
attempted here because `ModelResultGuards.ValidateReceiptV2` and the
cross-language receipt digest fixture would all need to move together with
it (see `services/desktop-support-api/Security.cs`), and that is a
receipt-schema change, not an authentication change. Until that exists, the
`sapphirus_static_token_principal` claim on the request-time principal is the
only mechanism-identifying signal; it does not persist into the receipt
itself.

### What a run under this mechanism proves, and does not

**Proved:** that the desktop client can reach every `/desktop/v1` route with
a real, bounded-strength, fixed-time-compared credential, and that
`ScopeAuthorization.IsApprovedDesktopPrincipal` — the same method a real Entra
principal is checked against — genuinely refuses a scope this credential was
not granted (`DesktopScopes.DiagnosticsUpload` is deliberately withheld from
the granted set for exactly this reason).

**Not proved:** anything about Entra JWT validation, issuer/audience
signature checking, token expiry handling, or any behavior specific to
`AddJwtBearer`. That path is exercised by nothing running under this
mechanism, because nothing running under this mechanism presents an Entra
token.

### What a future Entra rollout replaces

If this operator (or a successor) later gains Entra app-registration rights
for this application, turning Entra on requires no code change:

1. Leave `Sapphirus:StaticDesktopTokenEnabled` `false` (its default). This
   alone makes `AddJwtBearer` the sole scheme again, exactly as before this
   feature existed.
2. Supply a real `Sapphirus:Authority` (the tenant's issuer), a real
   `Sapphirus:Audience`, and the real `Sapphirus:ApprovedDesktopClientId`
   from the app registration.
3. Point the desktop client at that tenant for token acquisition.

At that point, `Sapphirus.DesktopSupportApi.Security.
StaticDesktopTokenAuthenticator`, `StaticDesktopAuthenticationHandler`,
`StaticDesktopAuthenticationOptions`, `StaticDesktopAuthenticationDefaults`,
`StaticDesktopSubjectConfiguration`, and
`SupportPlaneOptions.StaticDesktopTokenEnabled`/`StaticDesktopSubjectId` are
exactly what an Entra rollout deletes — every one of them is marked with a
`TODO(entra-rollout)` comment at its point of composition in
`Program.cs`/`DevelopmentDesktopAuthentication.cs` naming this.

## Related documents

- [`foundry-configuration.md`](foundry-configuration.md) — the production
  Foundry configuration surface, endpoint validation, discovery, and catalog
  behaviour. Everything there applies unchanged in this mode except the
  credential.
- [`foundry-prerequisites.md`](foundry-prerequisites.md) — the operator
  prerequisite check, which this mode does not and cannot satisfy.
