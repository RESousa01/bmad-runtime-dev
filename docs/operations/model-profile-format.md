# Model profile format and the effective catalog

This is the operator-facing contract between
`tools/foundry-profiles/publish-profiles.ps1` (the publisher) and
`services/desktop-support-api/Model/AppConfigurationModelProfileRegistry.cs`
(the reader), and between the reader and the desktop.

Canonical schemas are authority for field names and value ranges:

| Concern | Schema |
| --- | --- |
| One organization profile | `packages/contracts/schemas/model-profile.schema.json` |
| Desktop projection | `packages/contracts/schemas/model-catalog.schema.json` |
| Deployment inventory | `packages/contracts/schemas/foundry-deployment.schema.json` |
| Request that names a profile | `packages/contracts/schemas/model-access-request-v2.schema.json` |

Nothing in this document is a place to put a real endpoint, tenant,
subscription, credential, or deployment name. Every example below is
synthetic.

## 1. Key layout in App Configuration

| Key | Contents |
| --- | --- |
| `desktop-support:model-profile:<profileId>` | The canonical JSON of one profile |
| `desktop-support:model-profile-registry:version` | The registry-version record |

Both are written under a **label**, `production` by default. Superseded sets are
archived under an immutable `<label>-v<registryVersion>` label.

Two properties of this layout are load-bearing:

* The registry-version key is **outside** the profile prefix. After
  `model-profile` it continues with `-registry`, not with the prefix's
  terminating colon, so `KeyFilter = "desktop-support:model-profile:*"` cannot
  match it. A reader can therefore enumerate profiles with a prefix filter
  without ever mistaking the manifest for a profile.
* The publisher writes the registry-version key **last**. Until the record
  names a profile, that profile key is inert: the record is the manifest, and
  the reader refuses a set whose recomputed hash does not match it.

The reader must pin the label. Without pinning, a live profile could be merged
with an archived one from a different registry version.

## 2. Canonical value and the three digests

Values are RFC 8785 canonical JSON: object members in UTF-16 code-unit order,
no insignificant whitespace, integers only. The canonicalizer of record is
`packages/contracts/scripts/lib/canonical-json.mjs`.

Every digest has the form

```
sha256( "sapphirus:<purpose>:v1" + "\n" + <canonical JSON> )
```

rendered as `sha256:<64 lowercase hex>`.

| Digest | Purpose | Value hashed |
| --- | --- | --- |
| Profile | `model-profile` | The stored canonical profile JSON, byte for byte |
| Registry | `model-profile-registry` | `{"profiles":[{"profileHash":…,"profileId":…},…],"registryVersion":N}`, profiles in ordinal profile-ID order |
| Capability | `model-profile-capability` | The capability surface array, in ordinal profile-ID order |

The **capability surface** of a profile is exactly these members, and no
others: `allowedRegions`, `canonicalOutputSchemaAllowlist`, `deploymentName`,
`enabled`, `expectedModel`, `modelRoleAllowlist`, `profileId`,
`purposeAllowlist`, `qualificationStatus`, `requestModeAllowlist`,
`requiredCapabilities`, `retentionMode`, `supportedModalities`, `visible`. All
array members are sorted. Free text (`displayName`, `description`) and money
(`priceProfile`) are deliberately excluded, so a price change moves the
registry hash but not the capability hash, and every value in the preimage is
drawn from a closed ASCII alphabet the schema pins.

The reader **recomputes** the profile digests from the stored bytes, rebuilds
the registry digest and the capability digest, and refuses the whole set if
either disagrees with the record. That is what makes "the server used exactly
the set the publisher signed" a checked property rather than an assumption.

`services/desktop-support-api.Tests/Model/ModelProfileRegistryTests.cs` pins
digests produced by the JavaScript canonicalizer and asserts the C# reader
reproduces them.

## 3. What the reader refuses

Registry record: wrong schema version, `registryVersion` below 1, a
non-`sha256:` digest, an unknown member, a repeated member, a repeated profile
ID, a value above 65 536 bytes.

Profile value: above 10 000 bytes; not JSON; an unknown member; a repeated
member; a member spelled with different casing; wrong `schemaVersion`; any
field outside the canonical schema's range; a collection outside its declared
bounds; a `retentionMode` other than `transient_no_store`; a zero or negative
token or byte ceiling; `maxInputTokens` above `contextTokens`; a repeated
`variantId`; a variant whose `maxOutputTokens` exceeds the profile ceiling; an
unsafe `deploymentName` (outside `^[A-Za-z0-9][A-Za-z0-9_-]{0,126}[A-Za-z0-9]$`,
not already NFC, or carrying anything outside printable ASCII); a supported
modality whose matching input capability is not required; a qualification
digest that is a single repeated nibble (a placeholder, not evidence); a
`qualified` status with a null `qualifiedAt`.

Set level: a repeated profile ID; two **visible** profiles whose display names
normalize (trim, collapse whitespace, lowercase) to the same string. A hidden
profile may reuse a visible display name, because it never appears in a picker.

Whole registry: a profile named by the record but absent from the store; a key
whose profile ID does not match the `profileId` inside the value; a registry
hash or capability hash mismatch; a **registry-version rollback**.

Note the one deliberate split. A qualification status of `unqualified` or
`expired` with structurally valid evidence is *accepted into the registry* and
*blocked at catalog time* as `unqualified`. Only structurally missing evidence
is a registry-level refusal. Without this split the catalog's `unqualified`
blocked state would be unreachable and a de-qualified profile would vanish
without explanation.

## 4. Caching

The registry uses the same discipline as
`Policy/AppConfigurationPolicyProvider.cs`: one `SemaphoreSlim`, a 5 minute
refresh interval, and a 1 hour bounded last-known-valid window that **only a
transient load failure** may draw on. Every validation failure is immediate and
hard. Past the window the load failure propagates; there is nothing valid left
to serve.

## 5. The effective catalog

A profile becomes selectable only when all of the following hold. They are
evaluated in this order, and the first failure is what the desktop is told:

1. `enabled` is true — otherwise `policy_blocked` / `contact_administrator`.
2. The current region is in both the profile's `allowedRegions` and the tenant
   policy's — otherwise `policy_blocked`.
3. If the tenant policy publishes a non-empty approved-deployment list, the
   profile's deployment is in it — otherwise `policy_blocked`. **An empty list
   is not a block**: it means the policy publishes no deployment allowlist and
   the signed registry is the only authority. The registry is itself signed
   organization configuration, so this is a narrowing that may be absent, not a
   fail-open.
4. Qualification status is `qualified` — otherwise `unqualified` /
   `awaiting_qualification`.
5. A deployment with that **exact** name exists in the inventory — otherwise
   `deployment_missing` / `select_another_profile`, or `catalog_stale` /
   `refresh_catalog` when the snapshot is stale, because a stale snapshot is
   not authority for absence.
6. That deployment's publisher, model name, and model version all match
   `expectedModel` — otherwise `deployment_missing`. A name that now serves a
   different model is not a narrowing; it is a different model.
7. The subject is entitled to the profile — otherwise `not_entitled` /
   `request_entitlement`.

Capabilities are projected as the boolean AND of what the organization
qualified with what the provider currently advertises. A deployment may confirm
or narrow; it can never broaden. Any narrowing marks the profile `degraded`
with a short status detail that carries no provider body, endpoint, or
deployment name.

Limits are projected as: `contextTokens` and `maxInputTokens` from the profile;
`maxOutputTokens` from the **default variant**, not the profile ceiling, because
the ceiling is the most any variant may reach rather than what an unqualified
request gets; `maxAttachmentBytes` only if attachments survived narrowing, and
`null` otherwise so the desktop renders it as unknown rather than as a
zero-byte allowance.

Hidden profiles (`visible: false`) are omitted from **both** lists but stay
resolvable server side, which is how the adapted legacy profile keeps working
without ever appearing in a picker.

### Catalog version and hash

`catalogVersion` matches `^v[1-9][0-9]{0,9}\.[0-9a-f]{16}$`. The epoch is the
signed `registryVersion`; the digest is the first 16 hex characters of
`catalogHash`.

`catalogHash` has purpose `model-catalog` over:

```
{ catalogState, deploymentSnapshotHash, entitlementClass, policyHash,
  policyVersion, qualificationEvidenceHash, region, registryHash,
  registryVersion, schemaVersion }
```

`qualificationEvidenceHash` has purpose `model-catalog-qualification` over the
ordered list of `{deploymentHash, evidenceHash, profileId, status}` for every
joined entry.

Expiry is bounded: 5 minutes for a fresh catalog, 1 minute for a
`stale_usable` one.

## 6. `GET /desktop/v1/model-catalog`

Requires the approved desktop client and `Desktop.Access`. It deliberately does
**not** require `Desktop.Model.Invoke`: a user must be able to see which models
exist, and why one disappeared, without having consented to invoke anything.

| Element | Value |
| --- | --- |
| Optional query | `lastSelectedProfileId` — advisory, honoured only if still selectable. Any other query key is `request_invalid` |
| Optional header | `X-Sapphirus-Contract-Epoch` |
| ETag | `"<catalogHash>"`, strong; a `W/` prefix is not honoured |
| Cache | `private, max-age=30, must-revalidate`, `Vary: Authorization` |
| Also set | `Last-Modified` (generated), `Expires` (expiry), on 200 and 304 |

Problems:

| Condition | Code | Status |
| --- | --- | --- |
| Inventory never reachable, dependency down, untrusted registry, policy rollback | `model_catalog_unavailable` | 503 |
| Inventory aged past its staleness budget | `model_catalog_stale` | 503 |
| Client contract epoch below the minimum | `model_contract_upgrade_required` | 426 |
| Nothing selectable and nothing visible to explain why | `model_catalog_no_entitled_profiles` | 403 |

Nothing selectable but something visible is a **200 with an empty `profiles`
array, a null `defaultProfileId`, and populated `unavailableProfiles`** — that
is the case the catalog exists for.

Default selection: explicit organization default, then the supplied last-valid
user selection, then ascending `priority` with ties broken on ordinal profile
ID. Every step draws only from the currently selectable set, so a blocked
profile can never become the default and there is no cross-profile fallback.

## 7. Consent binding hashes

A version two request carries `modelProfileHash` and `modelCapabilityHash`. Both
are derived from values the desktop received in the catalog projection, so both
sides compute the same digest without sharing a canonicalizer for free text.

`modelProfileHash`, purpose `model-profile-binding`:

```
{ availability, capabilities, catalogVersion, defaultVariantId, limits,
  profileId, variants }
```

where `capabilities` is the seven projected booleans, `limits` is the four
projected values (null permitted), and `variants` is the ordered list of
`{maxOutputTokens, reasoningEffort, variantId}`.

`modelCapabilityHash`, purpose `model-capability-binding`:

```
{ capabilities, catalogVersion, profileId }
```

## 8. Route resolution

`ResolvedModelRoute` is server-only and is the only place a deployment name
exists at request time. Resolution checks, in order: catalog epoch; profile
presence; entitlement; the two binding hashes; the variant; purpose, model
role, request mode, and canonical output schema against the profile
allowlists; modalities, attachments, and tool definitions against the
**narrowed** capability set; context and output budgets; region and retention
from policy and profile rather than from the request; and every consent
binding.

Failure codes: `catalog_stale`, `profile_unavailable`, `profile_not_entitled`,
`variant_invalid`, `capability_mismatch`, `deployment_missing`,
`context_limit_exceeded`, `profile_binding_mismatch`.

**Catalog selection alone is not authority.** `VerifyBeforeDispatch` rechecks,
immediately before dispatch, that the deployment still exists and that its
publisher, model name, model version, and identity hash are unchanged. A
deployment replaced between selection and dispatch fails closed.

A retry re-dispatches the identical `ResolvedModelRoute` with the identical
request bytes. Resolution is deterministic and is not re-run between attempts,
so attempt two cannot reach a different profile, deployment, variant, or limit
than attempt one. There is no cross-profile fallback anywhere.

## 9. Development

`DevelopmentFixtureModelProfileRegistry` serves a labelled untrusted fixture
set so the whole architecture runs without an App Configuration store. Its
constructor refuses any environment but Development and Test, and
`EffectiveModelCatalogService` refuses an untrusted snapshot a second time
unless it was explicitly composed to permit one — which only the Development
composition does. Its deployment name,
`synthetic-development-deployment`, is deliberately implausible, so pointing it
at a real project yields an honest `deployment_missing` rather than an
accidental match.

## 10. Known gaps

* **The default profile key prefix does not match the publisher.**
  `ProductionOptions.ModelProfileConfigurationPrefix` defaults to
  `desktop-support:model-profiles:` (plural) while the publisher writes
  `desktop-support:model-profile:` (singular). Until the default is corrected,
  a production deployment must set the prefix explicitly or the registry reads
  zero profiles and the catalog fails closed.
* **The catalog projection does not carry the two binding hashes.** The
  canonical `model-catalog` schema has no member for `modelProfileHash` or
  `modelCapabilityHash`, so the desktop must derive them with section 7's
  recipe rather than read them. Adding two optional members to the projection
  would remove a whole class of implementation drift.
* **There is no explicit organization-default field.** The canonical profile
  schema has no "is default" member and no configuration key binds one, so the
  organization default is currently always absent and selection falls through
  to the user hint and then to deterministic order.
* **Entitlement is not yet per-subject in production.**
  `RegistryModelEntitlementSource` entitles every profile the signed registry
  carries, under one class name that is bound into the catalog digest. The
  seam and the filtering are real; the directory-backed source is not built.
