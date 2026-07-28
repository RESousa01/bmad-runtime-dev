//! The desktop model-catalog projection (FND-040).
//!
//! The support plane publishes one sanitized effective catalog at the single
//! reviewed route [`SupportStageRoute::ModelCatalog`]. This module owns the one
//! path from those untrusted bytes to a [`VerifiedModelCatalog`], and nothing
//! else in the desktop may construct one.
//!
//! Three properties drive the shape here.
//!
//! * **The route allowlist is the enum.** URL construction stays under
//!   [`SupportApiOrigin`](crate::SupportApiOrigin): the path comes from the
//!   route variant, and query and fragment are cleared. No renderer-supplied
//!   string reaches a URL.
//! * **Cache metadata and catalog content are separate authorities.** The
//!   `ETag` and the expiry that govern *reuse* live in
//!   [`ModelCatalogCacheMetadata`], never inside the decoded body. A `304` is
//!   answered from the cache only when that metadata matches the current
//!   subject and has not expired — a body can therefore never extend its own
//!   validity, and one subject's cache can never answer another subject's
//!   revalidation.
//! * **Display strings reach an untrusted renderer.** Every human-readable
//!   field is checked here for control characters and for the invisible
//!   formatting characters that make one profile name impersonate another.
//!   Lone surrogates never survive JSON parsing, so a body carrying one is
//!   rejected before any field is inspected.

use core::fmt;
use std::collections::HashSet;

use desktop_runtime::generated_contracts as canonical;
use desktop_runtime::UnixMillis;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use zeroize::Zeroizing;

use crate::transport::{
    is_json_content_type, HttpExecutor, HttpResponse, SupportApiTransport, SupportStageRoute,
};
use crate::CloudError;

/// The only catalog schema this desktop build decodes.
const CATALOG_SCHEMA_VERSION: &str = "sapphirus.model-catalog.v1";

/// The only provider a projected profile may name. The catalog cannot carry
/// Azure authority at all, so this is a constant, not a lookup.
const CATALOG_PROVIDER: &str = "microsoft_foundry";

/// Ceiling for one catalog response body.
///
/// The schema bounds a catalog at 64 selectable profiles of 8 variants each
/// plus 64 unavailable entries, which is roughly 120 KiB of maximal JSON. The
/// ceiling is set above that so a legitimate maximal catalog always fits, and
/// far below the crate's generic response ceiling so a catalog route can never
/// be used to push a megabyte into the desktop.
const MAX_CATALOG_RESPONSE_BYTES: usize = 256 * 1024;

/// Schema `maxItems` for both profile arrays. Array bounds are not expressed in
/// the generated Rust types, so they are enforced here.
const MAX_CATALOG_PROFILES: usize = 64;

/// Schema `maxItems` for one profile's variants.
const MAX_PROFILE_VARIANTS: usize = 8;

/// Schema `minItems` for one profile's variants.
const MIN_PROFILE_VARIANTS: usize = 1;

/// Ceiling for a subject binding or an `ETag`. Both are opaque, short, and
/// operator-visible in diagnostics, so both are bounded and charset-checked.
const MAX_OPAQUE_TOKEN_BYTES: usize = 128;

/// Cache metadata for one verified catalog.
///
/// Deliberately a separate type from [`VerifiedModelCatalog`]. Revalidation
/// reads only this: the subject the catalog was issued to, the validator to
/// echo back, and when the catalog stops being usable. Nothing here is derived
/// from a field the catalog body controls except `expires_at`, which is copied
/// once at decode time from an already-validated instant and never re-read from
/// the body afterwards.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelCatalogCacheMetadata {
    subject_hash: String,
    etag: Option<String>,
    expires_at: UnixMillis,
    fetched_at: UnixMillis,
}

impl ModelCatalogCacheMetadata {
    /// The pseudonymous subject this catalog was issued to.
    #[must_use]
    pub fn subject_hash(&self) -> &str {
        &self.subject_hash
    }

    /// The opaque validator to send as `If-None-Match`, when one was issued.
    #[must_use]
    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    /// When this catalog stops being usable.
    #[must_use]
    pub const fn expires_at(&self) -> UnixMillis {
        self.expires_at
    }

    /// When this catalog was last received from the support plane.
    #[must_use]
    pub const fn fetched_at(&self) -> UnixMillis {
        self.fetched_at
    }

    /// Whether this catalog may still be reused at `now`.
    #[must_use]
    pub const fn is_usable_at(&self, now: UnixMillis) -> bool {
        now.0 < self.expires_at.0
    }
}

/// A catalog body that passed every projection gate.
///
/// A value of this type has been decoded through the generated contract with
/// unknown fields rejected, has real timestamps, has no duplicate profile or
/// variant identifier, names a default that is actually selectable, and carries
/// only display text that is safe to hand to the renderer.
#[derive(Clone)]
pub struct VerifiedModelCatalog {
    catalog: canonical::ModelCatalog,
    generated_at: UnixMillis,
    expires_at: UnixMillis,
}

impl VerifiedModelCatalog {
    /// The decoded canonical projection.
    #[must_use]
    pub const fn catalog(&self) -> &canonical::ModelCatalog {
        &self.catalog
    }

    /// The monotonic catalog epoch plus input digest.
    #[must_use]
    pub fn catalog_version(&self) -> &str {
        &self.catalog.catalog_version
    }

    /// The content identity of this catalog.
    #[must_use]
    pub fn catalog_hash(&self) -> &str {
        &self.catalog.catalog_hash
    }

    /// Freshness of the deployment snapshot behind this catalog.
    #[must_use]
    pub const fn catalog_state(&self) -> canonical::ModelCatalogCatalogState {
        self.catalog.catalog_state
    }

    /// The profile to select when the user holds no valid persisted selection.
    #[must_use]
    pub fn default_profile_id(&self) -> Option<&str> {
        self.catalog
            .default_profile_id
            .as_ref()
            .map(|profile_id| profile_id.as_str())
    }

    /// The selectable profile identifiers, in catalog order.
    #[must_use]
    pub fn selectable_profile_ids(&self) -> Vec<&str> {
        self.catalog
            .profiles
            .iter()
            .map(|profile| profile.profile_id.as_str())
            .collect()
    }

    /// Whether `profile_id` is selectable right now.
    #[must_use]
    pub fn is_selectable(&self, profile_id: &str) -> bool {
        self.catalog
            .profiles
            .iter()
            .any(|profile| profile.profile_id.as_str() == profile_id)
    }

    /// The variant identifiers of one selectable profile, in catalog order.
    #[must_use]
    pub fn variant_ids(&self, profile_id: &str) -> Vec<&str> {
        self.catalog
            .profiles
            .iter()
            .find(|profile| profile.profile_id.as_str() == profile_id)
            .map(|profile| {
                profile
                    .variants
                    .iter()
                    .map(|variant| variant.variant_id.as_str())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// When the support plane generated this catalog.
    #[must_use]
    pub const fn generated_at(&self) -> UnixMillis {
        self.generated_at
    }

    /// When this catalog stops being usable.
    #[must_use]
    pub const fn expires_at(&self) -> UnixMillis {
        self.expires_at
    }
}

impl fmt::Debug for VerifiedModelCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedModelCatalog")
            .field("catalog_version", &&*self.catalog.catalog_version)
            .field("catalog_hash", &&*self.catalog.catalog_hash)
            .field("catalog_state", &self.catalog.catalog_state)
            .field("profiles", &self.catalog.profiles.len())
            .field(
                "unavailable_profiles",
                &self.catalog.unavailable_profiles.len(),
            )
            .field("generated_at", &self.generated_at.0)
            .field("expires_at", &self.expires_at.0)
            .field("display_text", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

/// A verified catalog together with the metadata that governs reusing it.
#[derive(Clone, Debug)]
pub struct CachedModelCatalog {
    metadata: ModelCatalogCacheMetadata,
    catalog: VerifiedModelCatalog,
}

impl CachedModelCatalog {
    /// The cache metadata: subject binding, validator, and expiry.
    #[must_use]
    pub const fn metadata(&self) -> &ModelCatalogCacheMetadata {
        &self.metadata
    }

    /// The verified catalog content.
    #[must_use]
    pub const fn catalog(&self) -> &VerifiedModelCatalog {
        &self.catalog
    }
}

/// What one catalog fetch produced.
#[derive(Clone, Debug)]
pub enum ModelCatalogFetch {
    /// The support plane returned a new catalog and it passed every gate.
    Fetched(CachedModelCatalog),
    /// The support plane answered `304` and the cached catalog was still bound
    /// to this subject and unexpired, so it is returned unchanged. A `304` does
    /// not extend validity: expiry comes from the catalog body, and no body was
    /// received.
    Revalidated(CachedModelCatalog),
}

impl ModelCatalogFetch {
    /// The catalog this fetch resolved to, however it was obtained.
    #[must_use]
    pub const fn cached(&self) -> &CachedModelCatalog {
        match self {
            Self::Fetched(cached) | Self::Revalidated(cached) => cached,
        }
    }
}

/// Everything one catalog fetch needs besides the bearer.
#[derive(Clone, Copy, Debug)]
pub struct ModelCatalogRequest<'a> {
    /// The pseudonymous identifier of the currently authenticated subject. The
    /// resulting cache entry is bound to it, and a cached entry bound to a
    /// different subject is never sent as a validator and never reused.
    pub subject_hash: &'a str,
    /// The catalog this desktop already holds, if any.
    pub cached: Option<&'a CachedModelCatalog>,
    /// The request correlation key.
    pub idempotency_key: &'a str,
}

impl<E> SupportApiTransport<E>
where
    E: HttpExecutor,
{
    /// Fetches the effective model catalog, revalidating a cached copy when one
    /// is held.
    ///
    /// A cached catalog is *never* authority on its own: it is offered here
    /// only as a conditional-request validator, and it can be reused only when
    /// the support plane answers `304`, the entry is bound to the same subject,
    /// and it has not expired at `now`.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::TransportFailed`] for malformed local inputs, an
    /// oversized response, an unclassified status, or an unsolicited `304`;
    /// [`CloudError::ReauthenticationRequired`] for `401`;
    /// [`CloudError::EntitlementUnavailable`] for `403`;
    /// [`CloudError::ContractUpgradeRequired`] for `409` and `426`;
    /// [`CloudError::ModelCatalogUnavailable`] for `503` and for a `304` whose
    /// cached catalog can no longer be reused; and
    /// [`CloudError::ModelCatalogIntegrity`] for any body that fails decoding
    /// or validation.
    pub async fn fetch_model_catalog(
        &self,
        bearer: Zeroizing<String>,
        request: &ModelCatalogRequest<'_>,
        now: UnixMillis,
    ) -> Result<ModelCatalogFetch, CloudError> {
        if !is_opaque_token(request.subject_hash) || request.idempotency_key.is_empty() {
            return Err(CloudError::TransportFailed);
        }
        // A validator is offered only from a cache entry that belongs to this
        // subject. Echoing another subject's `ETag` would let the support plane
        // answer `304` for a catalog this subject may not see.
        let bound_cache = request
            .cached
            .filter(|cached| cached.metadata.subject_hash == request.subject_hash);
        let validator = bound_cache.and_then(|cached| cached.metadata.etag.clone());
        let response = self
            .conditional_get(
                SupportStageRoute::ModelCatalog,
                bearer,
                validator.clone(),
                request.idempotency_key,
            )
            .await?;
        match response.status() {
            200 => {
                let catalog = decode_catalog_response(&response)?;
                reject_hash_transplant(bound_cache, &catalog)?;
                Ok(ModelCatalogFetch::Fetched(CachedModelCatalog {
                    metadata: ModelCatalogCacheMetadata {
                        subject_hash: request.subject_hash.to_owned(),
                        etag: response
                            .etag()
                            .filter(|etag| is_opaque_token(etag))
                            .map(str::to_owned),
                        expires_at: catalog.expires_at,
                        fetched_at: now,
                    },
                    catalog,
                }))
            }
            304 => revalidate(validator.as_deref(), bound_cache, &response, now),
            401 => Err(CloudError::ReauthenticationRequired),
            403 => Err(CloudError::EntitlementUnavailable),
            409 | 426 => Err(CloudError::ContractUpgradeRequired),
            503 => Err(CloudError::ModelCatalogUnavailable),
            _ => Err(CloudError::TransportFailed),
        }
    }
}

/// Resolves a `304`.
///
/// Reuse requires all four: a validator was actually sent, the cached entry is
/// bound to the current subject, the entry has not expired, and the response
/// carries no body and no conflicting validator.
fn revalidate(
    sent_validator: Option<&str>,
    bound_cache: Option<&CachedModelCatalog>,
    response: &HttpResponse,
    now: UnixMillis,
) -> Result<ModelCatalogFetch, CloudError> {
    // An unsolicited `304` is a protocol violation, not a cache hit.
    let Some(sent_validator) = sent_validator else {
        return Err(CloudError::TransportFailed);
    };
    // A `304` carries no representation. A body here is smuggling.
    if !response.body().is_empty() {
        return Err(CloudError::ModelCatalogIntegrity);
    }
    // A returned validator must be the one that was revalidated.
    if response
        .etag()
        .is_some_and(|returned| returned != sent_validator)
    {
        return Err(CloudError::ModelCatalogIntegrity);
    }
    let Some(cached) = bound_cache else {
        return Err(CloudError::TransportFailed);
    };
    if !cached.metadata.is_usable_at(now) {
        return Err(CloudError::ModelCatalogUnavailable);
    }
    Ok(ModelCatalogFetch::Revalidated(cached.clone()))
}

/// A catalog version identifies exactly one catalog content hash. The same
/// version arriving with a different hash is a transplant, not an update.
fn reject_hash_transplant(
    bound_cache: Option<&CachedModelCatalog>,
    fresh: &VerifiedModelCatalog,
) -> Result<(), CloudError> {
    let Some(cached) = bound_cache else {
        return Ok(());
    };
    if cached.catalog.catalog_version() == fresh.catalog_version()
        && cached.catalog.catalog_hash() != fresh.catalog_hash()
    {
        return Err(CloudError::ModelCatalogIntegrity);
    }
    Ok(())
}

/// Checks the response envelope, then the body.
fn decode_catalog_response(response: &HttpResponse) -> Result<VerifiedModelCatalog, CloudError> {
    if response.body().len() > MAX_CATALOG_RESPONSE_BYTES {
        return Err(CloudError::TransportFailed);
    }
    if !response.content_type().is_some_and(is_json_content_type) {
        return Err(CloudError::ModelCatalogIntegrity);
    }
    decode_catalog(response.body())
}

/// Decodes and fully validates one catalog body.
///
/// # Errors
///
/// Returns [`CloudError::ModelCatalogIntegrity`] for any body that is not a
/// well-formed, internally consistent, renderer-safe catalog.
fn decode_catalog(body: &[u8]) -> Result<VerifiedModelCatalog, CloudError> {
    // Parsed to a tree first so the nullable-but-required properties can be
    // checked for *presence* before the typed decode collapses "absent" and
    // "null" together. The tree costs one bounded allocation over an already
    // size-capped body.
    //
    // A lone surrogate never survives this line: JSON parsing rejects an escape
    // that does not form a valid pair, so such a body fails before any field is
    // inspected.
    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| CloudError::ModelCatalogIntegrity)?;
    require_nullable_properties(&value)?;
    // Field names are matched case-sensitively and unknown fields are rejected
    // by the generated type, so a catalog cannot smuggle an unreviewed property
    // past this line under any casing.
    let catalog: canonical::ModelCatalog =
        serde_json::from_value(value).map_err(|_| CloudError::ModelCatalogIntegrity)?;
    if catalog.schema_version.as_str() != Some(CATALOG_SCHEMA_VERSION) {
        return Err(CloudError::ModelCatalogIntegrity);
    }
    let generated_at = parse_instant(&catalog.generated_at)?;
    let expires_at = parse_instant(&catalog.expires_at)?;
    if expires_at.0 <= generated_at.0 {
        return Err(CloudError::ModelCatalogIntegrity);
    }
    if catalog.profiles.len() > MAX_CATALOG_PROFILES
        || catalog.unavailable_profiles.len() > MAX_CATALOG_PROFILES
    {
        return Err(CloudError::ModelCatalogIntegrity);
    }
    let mut profile_ids = HashSet::with_capacity(catalog.profiles.len());
    for profile in &catalog.profiles {
        validate_profile(profile)?;
        if !profile_ids.insert(profile.profile_id.as_str()) {
            return Err(CloudError::ModelCatalogIntegrity);
        }
    }
    for unavailable in &catalog.unavailable_profiles {
        if !is_safe_display_text(&unavailable.display_name)
            || !profile_ids.insert(unavailable.profile_id.as_str())
        {
            return Err(CloudError::ModelCatalogIntegrity);
        }
    }
    validate_default_profile(&catalog)?;
    Ok(VerifiedModelCatalog {
        catalog,
        generated_at,
        expires_at,
    })
}

/// Catalog properties the schema requires but types as nullable.
const NULLABLE_REQUIRED_CATALOG: [&str; 1] = ["defaultProfileId"];

/// Profile properties the schema requires but types as nullable.
const NULLABLE_REQUIRED_PROFILE: [&str; 3] = ["family", "statusDetail", "qualifiedAt"];

/// Limit properties the schema requires but types as nullable.
const NULLABLE_REQUIRED_LIMITS: [&str; 4] = [
    "contextTokens",
    "maxInputTokens",
    "maxOutputTokens",
    "maxAttachmentBytes",
];

/// Requires every nullable-but-required property to actually be present.
///
/// `serde` fills an absent `Option` field with `None`, so without this check
/// "the organization published no value" and "the producer dropped the field"
/// would be indistinguishable. That distinction is load-bearing: dropping
/// `qualifiedAt` is exactly how a profile's qualification evidence would go
/// missing with nothing failing, and dropping `defaultProfileId` would turn a
/// producer bug into a silent "the server named no default".
///
/// Every other required property is non-nullable in the generated types, so its
/// absence is already a decode error.
fn require_nullable_properties(value: &serde_json::Value) -> Result<(), CloudError> {
    let catalog = value.as_object().ok_or(CloudError::ModelCatalogIntegrity)?;
    require_properties(catalog, &NULLABLE_REQUIRED_CATALOG)?;
    for profile in catalog
        .get("profiles")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let profile = profile
            .as_object()
            .ok_or(CloudError::ModelCatalogIntegrity)?;
        require_properties(profile, &NULLABLE_REQUIRED_PROFILE)?;
        let limits = profile
            .get("limits")
            .and_then(serde_json::Value::as_object)
            .ok_or(CloudError::ModelCatalogIntegrity)?;
        require_properties(limits, &NULLABLE_REQUIRED_LIMITS)?;
    }
    Ok(())
}

fn require_properties(
    object: &serde_json::Map<String, serde_json::Value>,
    required: &[&str],
) -> Result<(), CloudError> {
    if required.iter().all(|key| object.contains_key(*key)) {
        Ok(())
    } else {
        Err(CloudError::ModelCatalogIntegrity)
    }
}

/// A named default must actually be selectable.
///
/// The `defaultProfileId` *property* is required, so a catalog that omits it
/// entirely never reaches this function — the generated type rejects it as a
/// missing field. What is checked here is a **dangling** default: one that names
/// a profile the catalog did not list as selectable, or one that names a profile
/// the catalog itself said is unavailable. Either would have the host silently
/// invoke, or silently fail to invoke, a model the catalog contradicts itself
/// about.
///
/// An explicit `null` is *not* rejected. The schema admits it, and it means the
/// support plane declined to name a default; the desktop then resolves
/// deterministically (see [`resolve_model_selection`]) from profiles the catalog
/// has already declared invocable. Treating that as an integrity failure would
/// make the documented last resolution step unreachable and would take the
/// product offline over a choice the server was entitled to leave open.
fn validate_default_profile(catalog: &canonical::ModelCatalog) -> Result<(), CloudError> {
    let Some(default_profile_id) = catalog.default_profile_id.as_ref() else {
        return Ok(());
    };
    let selectable = catalog
        .profiles
        .iter()
        .any(|profile| profile.profile_id == *default_profile_id);
    let blocked = catalog
        .unavailable_profiles
        .iter()
        .any(|profile| profile.profile_id == *default_profile_id);
    if selectable && !blocked {
        Ok(())
    } else {
        Err(CloudError::ModelCatalogIntegrity)
    }
}

/// Validates one selectable profile: provider, display text, variant bounds,
/// variant uniqueness, the default variant, and the qualification instant.
fn validate_profile(
    profile: &canonical::ModelCatalogModelProfileProjection,
) -> Result<(), CloudError> {
    if profile.provider.as_str() != Some(CATALOG_PROVIDER) {
        return Err(CloudError::ModelCatalogIntegrity);
    }
    if !is_safe_display_text(&profile.display_name) || !is_safe_display_text(&profile.description) {
        return Err(CloudError::ModelCatalogIntegrity);
    }
    if profile
        .family
        .as_ref()
        .is_some_and(|family| !is_safe_display_text(family))
    {
        return Err(CloudError::ModelCatalogIntegrity);
    }
    if profile
        .status_detail
        .as_ref()
        .is_some_and(|detail| !is_safe_display_text(detail))
    {
        return Err(CloudError::ModelCatalogIntegrity);
    }
    if let Some(qualified_at) = profile.qualified_at.as_ref() {
        parse_instant(qualified_at)?;
    }
    if profile.variants.len() < MIN_PROFILE_VARIANTS
        || profile.variants.len() > MAX_PROFILE_VARIANTS
    {
        return Err(CloudError::ModelCatalogIntegrity);
    }
    let mut variant_ids = HashSet::with_capacity(profile.variants.len());
    for variant in &profile.variants {
        if !is_safe_display_text(&variant.display_name)
            || !variant_ids.insert(variant.variant_id.as_str())
        {
            return Err(CloudError::ModelCatalogIntegrity);
        }
    }
    if !variant_ids.contains(profile.default_variant_id.as_str()) {
        return Err(CloudError::ModelCatalogIntegrity);
    }
    Ok(())
}

/// Parses a canonical UTC instant into desktop clock milliseconds.
///
/// The contract's pattern admits calendar-impossible dates such as
/// `2026-02-31T00:00:00.000Z`, so the pattern match the generated type already
/// performed is not sufficient; the value must also be a real instant.
fn parse_instant(value: &str) -> Result<UnixMillis, CloudError> {
    let millis = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| CloudError::ModelCatalogIntegrity)?
        .unix_timestamp_nanos()
        .checked_div(1_000_000)
        .and_then(|millis| u64::try_from(millis).ok())
        .ok_or(CloudError::ModelCatalogIntegrity)?;
    Ok(UnixMillis(millis))
}

/// Whether a string is safe to hand to the renderer as display text.
///
/// The renderer is untrusted, and so is the support plane's projection of an
/// organization's own profile registry. Two classes are rejected:
///
/// * **Control characters** (C0, `DEL`, and C1), which break every line-oriented
///   log, diagnostic, and terminal surface the string can reach.
/// * **Invisible formatting characters** — zero-width spaces and joiners, bidi
///   overrides and isolates, the byte-order mark, and the interlinear
///   annotation controls. These are what turn one profile name into a visual
///   twin of another, which is the whole risk of rendering an organization's
///   text next to a model the user is about to trust.
///
/// Lone surrogates cannot reach this function: JSON parsing rejects an escape
/// that does not form a valid pair, so such a body fails before decoding.
fn is_safe_display_text(value: &str) -> bool {
    !value.is_empty()
        && !value.chars().any(|character| {
            character.is_control()
                || matches!(character,
                    '\u{00ad}'
                    | '\u{061c}'
                    | '\u{180e}'
                    | '\u{200b}'..='\u{200f}'
                    | '\u{202a}'..='\u{202e}'
                    | '\u{2060}'..='\u{2064}'
                    | '\u{2066}'..='\u{206f}'
                    | '\u{feff}'
                    | '\u{fff9}'..='\u{fffb}')
        })
}

/// One selection the desktop persisted, as plain data.
///
/// This is the whole durable footprint of a model choice. It carries stable
/// organization-chosen identifiers and the identity of the catalog they were
/// chosen from — never a deployment name, an endpoint, or anything else that
/// could reconstitute Azure authority from local state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedModelSelection {
    /// The stable profile identifier the user chose.
    pub profile_id: String,
    /// The variant of that profile the user chose.
    pub variant_id: String,
    /// The version of the catalog the choice was made from.
    pub catalog_version: String,
    /// The content hash of the catalog the choice was made from.
    pub catalog_hash: String,
    /// When the choice was made.
    pub selected_at: UnixMillis,
}

/// The model the desktop will actually use, bound to the catalog that offered
/// it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedModelSelection {
    profile_id: String,
    variant_id: String,
    catalog_version: String,
    catalog_hash: String,
}

impl ResolvedModelSelection {
    /// The stable profile identifier.
    #[must_use]
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    /// The variant of that profile.
    #[must_use]
    pub fn variant_id(&self) -> &str {
        &self.variant_id
    }

    /// The version of the catalog this selection resolved against.
    #[must_use]
    pub fn catalog_version(&self) -> &str {
        &self.catalog_version
    }

    /// The content hash of the catalog this selection resolved against.
    #[must_use]
    pub fn catalog_hash(&self) -> &str {
        &self.catalog_hash
    }

    /// Whether moving from `self` to `next` invalidates everything derived from
    /// the model.
    ///
    /// Context estimates, the consent the user gave over a specific manifest,
    /// and any prepared request are all computed against one profile's limits
    /// under one catalog. A change to the catalog, the profile, or the variant
    /// makes all three stale, so they are discarded rather than carried into a
    /// model they were never sized for.
    #[must_use]
    pub fn invalidates_prepared_work(&self, next: &Self) -> bool {
        self != next
    }
}

/// Why a persisted selection can no longer be honored.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelSelectionBlocked {
    /// The profile is absent from the catalog entirely.
    ProfileRemoved,
    /// The profile is still visible but is not currently invocable, with the
    /// prerequisite and the recovery action the catalog stated.
    ProfileUnavailable {
        /// Why the profile cannot be invoked.
        availability: canonical::ModelCatalogBlockedAvailability,
        /// What the catalog says would restore it.
        recovery_action: canonical::ModelCatalogModelRecoveryAction,
    },
    /// The profile is selectable but the chosen variant is gone.
    VariantRemoved,
}

/// Where a resolved selection came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelSelectionSource {
    /// The current selection was still available.
    CurrentSelection,
    /// The most recent still-valid prior selection.
    RecentSelection,
    /// The catalog's own default.
    CatalogDefault,
    /// The deterministic first selectable profile.
    FirstSelectableProfile,
}

/// What resolving a selection against a catalog produced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelSelectionOutcome {
    /// A model was resolved without asking the user anything.
    Selected {
        /// The model to use.
        selection: ResolvedModelSelection,
        /// Which rule produced it.
        source: ModelSelectionSource,
    },
    /// The user's own selection is gone or blocked. The caller must preserve
    /// the draft, surface `blocked`, and require an explicit choice. Nothing
    /// here selects a replacement: silently switching the model a user is
    /// mid-draft with is the failure this variant exists to prevent.
    RequiresExplicitChoice {
        /// The selection that can no longer be honored.
        previous: PersistedModelSelection,
        /// Why, in the catalog's own vocabulary.
        blocked: ModelSelectionBlocked,
    },
    /// The catalog offers nothing selectable at all.
    NoneSelectable,
}

/// Resolves which model to use against a freshly verified catalog.
///
/// The order is: the current selection if it is still available, then the most
/// recent still-valid prior selection, then the catalog's default, then the
/// deterministic first selectable profile.
///
/// The first step is deliberately not a fallback. If the *current* selection
/// has disappeared or become unavailable, resolution stops and returns
/// [`ModelSelectionOutcome::RequiresExplicitChoice`] — it does not fall through
/// to the default, because a user who is part-way through a draft must never
/// discover afterwards that a different model answered.
#[must_use]
pub fn resolve_model_selection(
    catalog: &VerifiedModelCatalog,
    current: Option<&PersistedModelSelection>,
    recent: &[PersistedModelSelection],
) -> ModelSelectionOutcome {
    if let Some(current) = current {
        return match honor_selection(catalog, current) {
            Ok(selection) => ModelSelectionOutcome::Selected {
                selection,
                source: ModelSelectionSource::CurrentSelection,
            },
            Err(blocked) => ModelSelectionOutcome::RequiresExplicitChoice {
                previous: current.clone(),
                blocked,
            },
        };
    }
    // Newest first, with identifier tiebreakers so two selections recorded in
    // the same millisecond always resolve the same way.
    let mut candidates: Vec<&PersistedModelSelection> = recent.iter().collect();
    candidates.sort_by(|left, right| {
        right
            .selected_at
            .0
            .cmp(&left.selected_at.0)
            .then_with(|| left.profile_id.cmp(&right.profile_id))
            .then_with(|| left.variant_id.cmp(&right.variant_id))
    });
    for candidate in candidates {
        if let Ok(selection) = honor_selection(catalog, candidate) {
            return ModelSelectionOutcome::Selected {
                selection,
                source: ModelSelectionSource::RecentSelection,
            };
        }
    }
    if let Some(selection) = catalog
        .default_profile_id()
        .and_then(|profile_id| default_variant_selection(catalog, profile_id))
    {
        return ModelSelectionOutcome::Selected {
            selection,
            source: ModelSelectionSource::CatalogDefault,
        };
    }
    // Lexicographically smallest identifier rather than array position: profile
    // identifiers are unique within a catalog, so this is stable even if the
    // support plane ever reorders the array between two otherwise identical
    // catalogs.
    let first = catalog
        .catalog
        .profiles
        .iter()
        .map(|profile| profile.profile_id.as_str())
        .min();
    match first.and_then(|profile_id| default_variant_selection(catalog, profile_id)) {
        Some(selection) => ModelSelectionOutcome::Selected {
            selection,
            source: ModelSelectionSource::FirstSelectableProfile,
        },
        None => ModelSelectionOutcome::NoneSelectable,
    }
}

/// Whether one persisted selection can still be honored, and if not, why.
fn honor_selection(
    catalog: &VerifiedModelCatalog,
    persisted: &PersistedModelSelection,
) -> Result<ResolvedModelSelection, ModelSelectionBlocked> {
    let Some(profile) = catalog
        .catalog
        .profiles
        .iter()
        .find(|profile| profile.profile_id.as_str() == persisted.profile_id)
    else {
        return Err(catalog
            .catalog
            .unavailable_profiles
            .iter()
            .find(|profile| profile.profile_id.as_str() == persisted.profile_id)
            .map_or(ModelSelectionBlocked::ProfileRemoved, |profile| {
                ModelSelectionBlocked::ProfileUnavailable {
                    availability: profile.availability,
                    recovery_action: profile.recovery_action,
                }
            }));
    };
    if !profile
        .variants
        .iter()
        .any(|variant| variant.variant_id.as_str() == persisted.variant_id)
    {
        return Err(ModelSelectionBlocked::VariantRemoved);
    }
    Ok(ResolvedModelSelection {
        profile_id: persisted.profile_id.clone(),
        variant_id: persisted.variant_id.clone(),
        catalog_version: catalog.catalog_version().to_owned(),
        catalog_hash: catalog.catalog_hash().to_owned(),
    })
}

/// The selection for one selectable profile at its own default variant.
fn default_variant_selection(
    catalog: &VerifiedModelCatalog,
    profile_id: &str,
) -> Option<ResolvedModelSelection> {
    let profile = catalog
        .catalog
        .profiles
        .iter()
        .find(|profile| profile.profile_id.as_str() == profile_id)?;
    Some(ResolvedModelSelection {
        profile_id: profile.profile_id.to_string(),
        variant_id: profile.default_variant_id.to_string(),
        catalog_version: catalog.catalog_version().to_owned(),
        catalog_hash: catalog.catalog_hash().to_owned(),
    })
}

/// Whether an opaque binding — a subject hash or an `ETag` — is short, printable
/// ASCII with no header-injection or log-injection characters.
///
/// Double quotes are admitted because a well-formed `ETag` is quoted
/// (`"abc"`, `W/"abc"`); backslashes are not, because an escaped quote inside a
/// validator is the one shape that could change how the value re-serializes.
fn is_opaque_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_OPAQUE_TOKEN_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'\\')
}
