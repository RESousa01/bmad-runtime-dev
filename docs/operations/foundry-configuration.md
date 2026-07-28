# Microsoft Foundry support-plane configuration

Date: 2026-07-28

This is the operator reference for the Foundry half of the support plane's
production configuration: what to set, what the service rejects and why, and
what the deployment catalog does at runtime. It covers FND-020 through FND-023.

Nothing here names a real project, deployment, account, or identity. There is
no Foundry project available to this repository; every behaviour below is
proved against the in-memory fake server and injected fakes, never a live
endpoint.

## Configuration surface

All keys live under the `Sapphirus` section, so the container environment
variable is `Sapphirus__<Key>`.

| Key | Required | Default | Meaning |
|---|---|---|---|
| `FoundryProjectEndpoint` | yes | — | The complete project endpoint, `https://<account>.services.ai.azure.com/api/projects/<project>`. The documented `_project` alias is accepted. |
| `FoundryModelIdentityClientId` | yes | — | Client ID of the user-assigned managed identity that reaches Foundry. |
| `AllowSharedPilotIdentity` | no | `false` | Pilot-only opt-in that permits the Foundry identity to be the same principal as the runtime identity. |
| `FoundryDeploymentRefreshSeconds` | no | `300` | Catalog refresh interval. Bounded to 30–3600 seconds. |
| `FoundryCatalogMaximumStaleSeconds` | no | `1800` | Staleness budget. Must be at least the refresh interval and at most 86400 seconds. |
| `ModelProfileConfigurationPrefix` | no | `desktop-support:model-profiles:` | App Configuration key prefix for the server-owned model profiles. |

There is deliberately **no** key for the Responses endpoint. It is derived as
`<projectEndpoint>/openai/v1/` and cannot be supplied, overridden, or pointed
somewhere else by configuration.

There is deliberately **no** key for a deployment name. Deployments are
discovered at runtime; a deployment name in configuration would reintroduce
exactly the fixed-deployment coupling this migration removes.

## What the project endpoint validator rejects

`ProductionOptions.TryValidateFoundryProjectEndpoint` is a second validator
alongside the existing one, because the existing `TryValidateEndpoint` requires
a root path (`/`) and every project endpoint carries `/api/projects/<project>`.

The configured value must be HTTPS, on the default port, with no user info, no
query, and no fragment. The host must be a plain lowercase ASCII DNS name
ending in `.services.ai.azure.com`. The path must be exactly
`/api/projects/<safe-name>` or `/api/projects/_project`.

The final check is the strict one: after normalization the configured string
must be **byte-identical** to the canonical form
`https://<host>/api/projects/<project>/`, or to that form without its trailing
slash. One comparison therefore rejects all of the following, each of which
parses to something that looks legitimate:

| Rejected input class | Example |
|---|---|
| Account host with no project | `https://account.services.ai.azure.com/` |
| Retired Azure OpenAI host | `https://account.openai.azure.com/api/projects/p` |
| Suffix lookalike | `https://account.services.ai.azure.com.evil.test/api/projects/p` |
| Dot-segment traversal | `https://account.services.ai.azure.com/api/projects/p/../evil` |
| Percent-encoding | `https://account.services.ai.azure.com/api/projects/for%67e` |
| Mixed case in host or fixed path | `https://Account.services.ai.azure.com/api/projects/p` |
| Extra trailing path | `https://account.services.ai.azure.com/api/projects/p/deployments` |
| Explicit port | `https://account.services.ai.azure.com:443/api/projects/p` |
| Query, fragment, user info, plaintext scheme | `…/api/projects/p?api-version=v1` |
| Unicode or punycode homograph host | `https://xn--ccount-2va.services.ai.azure.com/api/projects/p` |

Homograph hosts are rejected in both spellings: any non-ASCII character in the
host fails, and so does any label beginning `xn--`.

**Trailing-slash policy.** Both spellings are accepted on input and normalized
to one form with a trailing slash. That normalized value is what the transport,
the discovery path, and the derived Responses endpoint are all built from.

## Identity rules

The Foundry model identity must be present and must never be the SQL migration
identity. By default it must also differ from the App Configuration / Key Vault
/ SQL runtime identity.

Reusing the runtime identity requires two things at once: the explicit
`AllowSharedPilotIdentity` flag, and the pilot release channel
(`ReleaseChannel: beta`). On the `stable` channel a shared identity is rejected
regardless of the flag. The flag can never make the SQL migration identity
shareable.

Every credential is an explicit `ManagedIdentityCredential` built from a named
client ID. `DefaultAzureCredential` is never used anywhere in the support
plane, and a test asserts the container contains no such registration.

## Deployment discovery

Discovery calls
`GET <projectEndpoint>deployments?api-version=v1&deploymentType=ModelDeployment`
with a bearer token for the `https://ai.azure.com/.default` scope, acquired
once per discovery operation, plus a freshly generated `x-ms-client-request-id`
per request.

It runs on its own named `HttpClient` (`foundry-deployment-discovery`, this
repository's first `IHttpClientFactory` registration) whose transport:

- disables automatic redirects, because a redirect is the cheapest way to move
  a bearer token to another origin;
- enables only gzip and deflate decompression, with the 1 MiB cap applied to
  the decompressed stream;
- uses a 10-second connect and request timeout;
- sends a fixed redacted user agent (`sapphirus-support-plane/1`) that
  discloses no OS build, machine name, or SDK version;
- sends `x-ms-return-client-request-id: true` so a support investigation can
  correlate without any log carrying a prompt, deployment name, or endpoint;
- disables cookies and proxies.

### Pagination is the SSRF gate

A `nextLink` is followed only when every one of these holds:

- HTTPS, default port, no user info, no fragment;
- the same host as the project endpoint;
- the path is exactly `<projectPath>deployments`;
- the query contains exactly one `api-version=v1`, an optional
  `deploymentType=ModelDeployment`, at most one bounded `continuationToken`,
  and no other key;
- the link has not already been visited in this operation (cycle detection);
- fewer than 32 pages have been fetched.

Anything else is an integrity failure, not a retry. The response bodies of all
pages of one operation are bounded to 1 MiB in aggregate.

### Normalization

Raw provider fields never escape the DTO layer. Discovery projects into the
canonical `sapphirus.foundry-deployment.v1` contract
(`packages/contracts/schemas/foundry-deployment.schema.json`): deployment name,
type, publisher, model name and version, bounded SKU fields, and an allowlisted
capability set, in canonical order.

Provider capability keys map onto canonical names — `json_schema` becomes
`structuredOutput`, `tool_calling` becomes `toolCalling`, and so on. A
capability the provider did not advertise normalizes to `false`. Normalization
narrows a profile; it never broadens one. Unknown provider fields and unknown
capability keys are ignored rather than projected, and they do not change the
snapshot hash.

Duplicate deployment names collapse only when their canonical identity hashes
match exactly. A name that is redefined with conflicting identity fails the
whole snapshot rather than being silently resolved.

### Failure classification

| Provider condition | Classification |
|---|---|
| 401, 403, token unavailable | authorization |
| 400, 404 | project or configuration |
| 408, 429, 5xx, timeout, unreachable | transient |
| Malformed, oversized, duplicate-property, or unrepresentable body | invalid body |
| `nextLink` origin, path, query, cycle, or page bound | pagination integrity |

The retrieval timestamp, page count, byte count, and provider request IDs are
kept in server-only evidence. That evidence carries no deployment name, project
endpoint, token, or provider response body.

## Catalog cache and health

The cache stores one immutable snapshot with its generated-at, expires-at,
maximum-stale-at, snapshot hash, and last refresh outcome. Defaults are a
5-minute refresh and a 30-minute maximum staleness budget.

- A fresh snapshot is returned immediately, without touching the network.
- When it has expired, exactly one caller refreshes. Everyone else waits on a
  single gate and re-checks freshness under it, so a burst of readers produces
  one provider call rather than a stampede.
- On a **transient** failure the last known valid snapshot keeps serving, but
  only until its maximum-stale instant. No failure ever moves that instant.
- On an **authorization, project-configuration, or pagination-integrity**
  failure service stops immediately. A stale snapshot is not served even when
  the staleness budget has time left.
- A refreshed snapshot that does not normalize never replaces the one that did.
  The catalog reports `invalid` and keeps the last valid snapshot stored, so a
  later successful refresh recovers cleanly.

A background service warms the catalog at startup and refreshes on a jittered
interval (the interval plus up to 20 %, never less) so a fleet of replicas does
not synchronize onto the provider. It stops on the host shutdown token and
never lets a refresh failure escape.

### Health states

The catalog has exactly four public states: `fresh`, `stale_usable`,
`unavailable`, `invalid`.

| State | Health status |
|---|---|
| `fresh` | healthy |
| `stale_usable` | degraded |
| `unavailable` | unhealthy |
| `invalid` | unhealthy |

The health check is registered under the dependency class `foundry_catalog`
and reads cached state only — a probe never starts a refresh. The response
renderer emits `{status, dependencies:[{dependency, status}]}` and nothing
else. No deployment name, snapshot hash, project endpoint, or provider message
appears in a health response.

## Operator checklist

1. Run the prerequisite check in
   [`foundry-prerequisites.md`](foundry-prerequisites.md) and confirm the
   report says `"ready": true`.
2. Set `Sapphirus__FoundryProjectEndpoint` and
   `Sapphirus__FoundryModelIdentityClientId` on the Container App.
3. Leave `Sapphirus__AllowSharedPilotIdentity` unset unless a pilot explicitly
   requires a shared principal, and confirm the release channel is `beta`.
4. Confirm the identity has the Foundry User role at the intended scope.
5. After rollout, confirm `/healthz/ready` reports the `foundry_catalog`
   dependency as `healthy`.

## Migration status

The legacy `ModelEndpoint`, `ModelDeployment`, and configured
provider/model/capability/deployment hash values are still required by
production configuration. They are consumed by `ModelAccessProfile` and the
existing Azure OpenAI executor, which a later task replaces. Removing them is
not a configuration change alone: the deployment templates, the local compose
contract, and the CI configuration contract all inject them today.
