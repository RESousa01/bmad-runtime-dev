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

The desktop-to-support-API leg is unaffected by this extension. It still needs
an app registration for production; locally it continues to use the existing
development authentication adapters.

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

## Related documents

- [`foundry-configuration.md`](foundry-configuration.md) — the production
  Foundry configuration surface, endpoint validation, discovery, and catalog
  behaviour. Everything there applies unchanged in this mode except the
  credential.
- [`foundry-prerequisites.md`](foundry-prerequisites.md) — the operator
  prerequisite check, which this mode does not and cannot satisfy.
