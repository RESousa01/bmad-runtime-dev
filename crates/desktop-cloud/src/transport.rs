use core::fmt;
use std::time::Duration;

use async_trait::async_trait;
use url::Url;
use zeroize::Zeroizing;

use crate::{
    AuthorizedModelRequest, CloudAccess, CloudError, CloudSession, DispatchedModelRequest,
    IdentityBroker, RawModelOutput, VerifiedEntitlement,
};
use desktop_runtime::UnixMillis;

const MODEL_ACCESS_PATH: &str = "desktop/v1/model-access/calls";
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MIN_HTTP_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_HTTP_TIMEOUT: Duration = Duration::from_mins(2);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupportApiOrigin {
    endpoint: Url,
}

impl SupportApiOrigin {
    /// Parses one immutable support-plane HTTPS origin.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::InvalidSupportOrigin`] for non-HTTPS URLs or an
    /// origin containing credentials, a path, query, or fragment.
    pub fn new(value: impl AsRef<str>) -> Result<Self, CloudError> {
        let origin = Url::parse(value.as_ref()).map_err(|_| CloudError::InvalidSupportOrigin)?;
        if origin.scheme() != "https"
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.path() != "/"
            || origin.query().is_some()
            || origin.fragment().is_some()
        {
            return Err(CloudError::InvalidSupportOrigin);
        }
        let endpoint = origin
            .join(MODEL_ACCESS_PATH)
            .map_err(|_| CloudError::InvalidSupportOrigin)?;
        Ok(Self { endpoint })
    }

    #[must_use]
    pub fn endpoint(&self) -> &Url {
        &self.endpoint
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
}

pub struct OutboundHttpRequest {
    method: HttpMethod,
    url: Url,
    body: Vec<u8>,
    bearer: Zeroizing<String>,
    idempotency_key: String,
    /// The conditional-request validator, when the caller holds a cached
    /// representation. Opaque to this crate: it is echoed back exactly as the
    /// support plane issued it and is never parsed or derived from a body.
    if_none_match: Option<String>,
}

impl OutboundHttpRequest {
    #[must_use]
    pub const fn method(&self) -> HttpMethod {
        self.method
    }

    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    #[must_use]
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    #[must_use]
    pub fn if_none_match(&self) -> Option<&str> {
        self.if_none_match.as_deref()
    }
}

impl fmt::Debug for OutboundHttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OutboundHttpRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("body", &"[REDACTED]")
            .field("body_bytes", &self.body.len())
            .field("bearer", &"[REDACTED]")
            .field("idempotency_key", &self.idempotency_key)
            .field("if_none_match", &self.if_none_match.is_some())
            .finish()
    }
}

#[derive(Clone)]
pub struct HttpResponse {
    status: u16,
    content_type: Option<String>,
    content_length: Option<u64>,
    body: Vec<u8>,
    /// The representation validator the support plane issued, when it issued
    /// one. Held beside the body rather than inside it: cache metadata and
    /// catalog content are separate authorities, and a body may never supply
    /// its own validator.
    etag: Option<String>,
}

impl HttpResponse {
    #[must_use]
    pub fn new(
        status: u16,
        content_type: Option<String>,
        content_length: Option<u64>,
        body: Vec<u8>,
    ) -> Self {
        Self {
            status,
            content_type,
            content_length,
            body,
            etag: None,
        }
    }

    /// Attaches the transport-observed `ETag` header value.
    #[must_use]
    pub fn with_etag(mut self, etag: Option<String>) -> Self {
        self.etag = etag;
        self
    }

    pub(crate) const fn status(&self) -> u16 {
        self.status
    }

    pub(crate) fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    pub(crate) fn body(&self) -> &[u8] {
        &self.body
    }

    pub(crate) fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }
}

impl fmt::Debug for HttpResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("content_type", &self.content_type)
            .field("content_length", &self.content_length)
            .field("body", &"[REDACTED]")
            .field("body_bytes", &self.body.len())
            .field("etag", &self.etag.is_some())
            .finish()
    }
}

/// One bounded, incrementally readable response body.
///
/// It exists because a model stream cannot be buffered before it is decoded:
/// the whole point is that the first token reaches the user before the last one
/// is produced. The source yields whatever bytes have arrived; framing, bounds,
/// and ordering are the decoder's job (see [`crate::model_stream`]).
#[async_trait]
pub trait HttpChunkSource: Send {
    /// Reads the next chunk, or `None` once the body is complete.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::TransportFailed`] when the connection breaks.
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, CloudError>;
}

/// A response whose body is still arriving.
pub struct StreamingHttpResponse {
    status: u16,
    content_type: Option<String>,
    body: Box<dyn HttpChunkSource>,
}

impl StreamingHttpResponse {
    #[must_use]
    pub fn new(status: u16, content_type: Option<String>, body: Box<dyn HttpChunkSource>) -> Self {
        Self {
            status,
            content_type,
            body,
        }
    }

    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }

    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    /// Consumes the response into its chunk source.
    #[must_use]
    pub fn into_body(self) -> Box<dyn HttpChunkSource> {
        self.body
    }
}

impl fmt::Debug for StreamingHttpResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StreamingHttpResponse")
            .field("status", &self.status)
            .field("content_type", &self.content_type)
            .field("body", &"[STREAMING]")
            .finish()
    }
}

#[async_trait]
pub trait HttpExecutor: Send + Sync {
    /// Executes one already-authorized request against its immutable URL.
    ///
    /// # Errors
    ///
    /// Returns only stable, sanitized cloud errors.
    async fn execute(&self, request: OutboundHttpRequest) -> Result<HttpResponse, CloudError>;

    /// Opens one already-authorized request as an incrementally readable body.
    ///
    /// Provided rather than required so every existing executor — including the
    /// scripted ones the offline gates run against — keeps compiling. The
    /// default refuses, which is the fail-closed direction: an executor that has
    /// not opted in cannot be used to open a model stream.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::TransportFailed`] when the executor does not
    /// support streaming or the connection cannot be established.
    async fn execute_stream(
        &self,
        request: OutboundHttpRequest,
    ) -> Result<StreamingHttpResponse, CloudError> {
        let _ = request;
        Err(CloudError::TransportFailed)
    }
}

#[derive(Debug)]
pub struct SupportApiTransport<E> {
    origin: SupportApiOrigin,
    executor: E,
}

impl<E> SupportApiTransport<E>
where
    E: HttpExecutor,
{
    #[must_use]
    pub const fn new(origin: SupportApiOrigin, executor: E) -> Self {
        Self { origin, executor }
    }

    /// Test-support accessor for scripted executors.
    #[doc(hidden)]
    #[must_use]
    pub const fn executor_for_test(&self) -> &E {
        &self.executor
    }

    /// Sends one sealed request with one currently valid cloud access grant.
    ///
    /// # Errors
    ///
    /// Returns a stable [`CloudError`] for stale sessions, request drift,
    /// transport failure, or malformed and oversized untrusted responses.
    pub async fn send<B>(
        &self,
        session: &CloudSession<B>,
        access: &CloudAccess,
        entitlement: &VerifiedEntitlement,
        request: AuthorizedModelRequest,
        now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError>
    where
        B: IdentityBroker,
    {
        if !session.is_current(access) {
            return Err(CloudError::SessionInvalidated);
        }
        if !session.is_current_at(access, now) {
            return Err(CloudError::ReauthenticationRequired);
        }
        request.verify()?;
        entitlement.authorize_model_request(request.policy_hash(), now)?;
        let body = serde_json::to_vec(&request).map_err(|_| CloudError::TransportFailed)?;
        if body.len() > MAX_REQUEST_BYTES {
            return Err(CloudError::TransportFailed);
        }
        let bearer = access.bearer_copy();
        if !session.is_current(access) {
            return Err(CloudError::SessionInvalidated);
        }
        let response = self
            .executor
            .execute(OutboundHttpRequest {
                method: HttpMethod::Post,
                url: self.origin.endpoint.clone(),
                body,
                bearer,
                idempotency_key: request.request_id().to_string(),
                if_none_match: None,
            })
            .await?;
        if !session.is_current(access) {
            return Err(CloudError::SessionInvalidated);
        }
        let output = validate_response(&response)?;
        Ok((DispatchedModelRequest::new(request), output))
    }
}

/// The reviewed desktop support-plane stage routes, mirrored one-to-one
/// from the C# `/desktop/v1` surface. No other path is reachable through
/// the stage exchange.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportStageRoute {
    Bootstrap,
    DeviceRegistrations,
    PolicyCurrent,
    EntitlementLeases,
    /// The effective model catalog (FND-040). The enum *is* the allowlist: a
    /// renderer can name this variant but can never supply the path it maps
    /// to, so no renderer-controlled string ever reaches URL construction.
    ModelCatalog,
    /// The one fixed, reviewed streaming route (FND-043, `STREAM-SURFACE`).
    ///
    /// It is a constant here and in `ModelStreamEndpoint.FullRoute` on the
    /// server. The renderer never supplies a URL, and there is no path
    /// parameter, query key, or header through which a caller could reach a
    /// different provider surface.
    ModelStream,
}

impl SupportStageRoute {
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Bootstrap => "/desktop/v1/bootstrap",
            Self::DeviceRegistrations => "/desktop/v1/devices/registrations",
            Self::PolicyCurrent => "/desktop/v1/policy/current",
            Self::EntitlementLeases => "/desktop/v1/entitlements/leases",
            Self::ModelCatalog => "/desktop/v1/model-catalog",
            Self::ModelStream => "/desktop/v1/model-access/streams",
        }
    }

    #[must_use]
    pub const fn method(self) -> HttpMethod {
        match self {
            Self::Bootstrap | Self::PolicyCurrent | Self::ModelCatalog => HttpMethod::Get,
            Self::DeviceRegistrations | Self::EntitlementLeases | Self::ModelStream => {
                HttpMethod::Post
            }
        }
    }
}

const MAX_STAGE_REQUEST_BYTES: usize = 64 * 1024;

impl<E> SupportApiTransport<E>
where
    E: HttpExecutor,
{
    /// Exchanges one bounded stage request against a reviewed route. The
    /// returned bytes are untrusted until a signed-document verifier
    /// (`accept_policy`, `accept_lease`, `accept_receipt_proof`, or the
    /// registration verifier) accepts them.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::TransportFailed`] for oversized requests,
    /// non-success statuses, or oversized/opaque responses.
    pub async fn exchange_stage(
        &self,
        route: SupportStageRoute,
        bearer: Zeroizing<String>,
        body: Vec<u8>,
        idempotency_key: &str,
    ) -> Result<Vec<u8>, CloudError> {
        if body.len() > MAX_STAGE_REQUEST_BYTES {
            return Err(CloudError::TransportFailed);
        }
        if matches!(route.method(), HttpMethod::Get) && !body.is_empty() {
            return Err(CloudError::TransportFailed);
        }
        let response = self
            .executor
            .execute(OutboundHttpRequest {
                method: route.method(),
                url: self.stage_url(route),
                body,
                bearer,
                idempotency_key: idempotency_key.to_owned(),
                if_none_match: None,
            })
            .await?;
        if !(200..300).contains(&response.status) {
            return Err(CloudError::TransportFailed);
        }
        let is_json = response
            .content_type
            .as_deref()
            .is_some_and(|content_type| content_type.starts_with("application/json"));
        if !is_json || response.body.len() > MAX_RESPONSE_BYTES {
            return Err(CloudError::TransportFailed);
        }
        Ok(response.body)
    }

    /// Builds one reviewed stage URL under the fixed support origin. The path
    /// comes only from [`SupportStageRoute`], and any query or fragment the
    /// configured origin might carry is cleared.
    fn stage_url(&self, route: SupportStageRoute) -> Url {
        let mut url = self.origin.endpoint.clone();
        url.set_path(route.path());
        url.set_query(None);
        url.set_fragment(None);
        url
    }

    /// Issues one bodyless conditional `GET` against a reviewed route and
    /// returns the raw, still-untrusted response.
    ///
    /// The caller owns classification and decoding; this method only guarantees
    /// that the method is `GET`, that no body is sent, and that the URL was
    /// built from the route allowlist under the fixed origin.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::TransportFailed`] when the route is not a `GET`
    /// route or the executor fails.
    pub(crate) async fn conditional_get(
        &self,
        route: SupportStageRoute,
        bearer: Zeroizing<String>,
        if_none_match: Option<String>,
        idempotency_key: &str,
    ) -> Result<HttpResponse, CloudError> {
        if !matches!(route.method(), HttpMethod::Get) {
            return Err(CloudError::TransportFailed);
        }
        self.executor
            .execute(OutboundHttpRequest {
                method: HttpMethod::Get,
                url: self.stage_url(route),
                body: Vec::new(),
                bearer,
                idempotency_key: idempotency_key.to_owned(),
                if_none_match,
            })
            .await
    }

    /// Opens the one reviewed model stream route.
    ///
    /// The URL is built from [`SupportStageRoute::ModelStream`] under the fixed
    /// origin, exactly as every other stage route is; no caller supplies a path.
    /// The response is refused unless it is a `200` carrying
    /// `text/event-stream`, so a buffered JSON body — including a problem
    /// document the server chose to return before the stream opened — can never
    /// be fed to the stream decoder.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::TransportFailed`] for an oversized request body, a
    /// non-`200` status, a media type other than `text/event-stream`, or an
    /// executor that does not support streaming.
    pub async fn open_model_stream(
        &self,
        bearer: Zeroizing<String>,
        body: Vec<u8>,
        idempotency_key: &str,
    ) -> Result<StreamingHttpResponse, CloudError> {
        if body.len() > MAX_REQUEST_BYTES || body.is_empty() || idempotency_key.is_empty() {
            return Err(CloudError::TransportFailed);
        }
        let response = self
            .executor
            .execute_stream(OutboundHttpRequest {
                method: HttpMethod::Post,
                url: self.stage_url(SupportStageRoute::ModelStream),
                body,
                bearer,
                idempotency_key: idempotency_key.to_owned(),
                if_none_match: None,
            })
            .await?;
        if response.status() != 200
            || !response
                .content_type()
                .is_some_and(is_event_stream_content_type)
        {
            return Err(CloudError::TransportFailed);
        }
        Ok(response)
    }
}

/// The media type the reviewed stream route must answer with.
///
/// Deliberately exact rather than a prefix test: accepting anything that merely
/// starts with `text/event-stream` would also accept
/// `text/event-stream-experimental`, which is not this contract.
pub(crate) fn is_event_stream_content_type(value: &str) -> bool {
    value
        .split_once(';')
        .map_or(value, |(media_type, _)| media_type)
        .trim()
        .eq_ignore_ascii_case("text/event-stream")
}

/// Reads a `reqwest` response body one arriving chunk at a time.
///
/// `Response::chunk` is used rather than `bytes_stream` deliberately: it is the
/// same primitive the buffered executor above already reads with, so streaming
/// adds no new way for bytes to enter the process.
struct ReqwestChunkSource {
    response: reqwest::Response,
}

#[async_trait]
impl HttpChunkSource for ReqwestChunkSource {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, CloudError> {
        let chunk = self
            .response
            .chunk()
            .await
            .map_err(|_| CloudError::TransportFailed)?;
        Ok(chunk.map(|bytes| bytes.to_vec()))
    }
}

#[derive(Debug)]
pub struct ReqwestHttpExecutor {
    client: reqwest::Client,
}

impl ReqwestHttpExecutor {
    /// Creates a no-redirect, no-proxy, rustls HTTPS executor.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::TransportFailed`] for invalid timeout bounds or
    /// client construction failure.
    pub fn new(connect_timeout: Duration, operation_timeout: Duration) -> Result<Self, CloudError> {
        if !(MIN_HTTP_TIMEOUT..=MAX_HTTP_TIMEOUT).contains(&connect_timeout)
            || !(MIN_HTTP_TIMEOUT..=MAX_HTTP_TIMEOUT).contains(&operation_timeout)
            || connect_timeout > operation_timeout
        {
            return Err(CloudError::TransportFailed);
        }
        let client = reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(connect_timeout)
            .timeout(operation_timeout)
            .user_agent("sapphirus-desktop/0.1")
            .build()
            .map_err(|_| CloudError::TransportFailed)?;
        Ok(Self { client })
    }
}

#[async_trait]
impl HttpExecutor for ReqwestHttpExecutor {
    async fn execute(&self, request: OutboundHttpRequest) -> Result<HttpResponse, CloudError> {
        let OutboundHttpRequest {
            method,
            url,
            body,
            bearer,
            idempotency_key,
            if_none_match,
        } = request;
        let builder = match method {
            HttpMethod::Get => self.client.get(url),
            HttpMethod::Post => self
                .client
                .post(url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body),
        };
        let mut builder = builder
            .bearer_auth(bearer.as_str())
            .header(reqwest::header::ACCEPT, "application/json")
            .header("Idempotency-Key", idempotency_key);
        if let Some(validator) = if_none_match {
            // An `ETag` is opaque, but it still has to be a legal header value.
            // A validator that cannot be represented is dropped rather than
            // failing the request: the worst case is an unconditional fetch.
            if let Ok(value) = reqwest::header::HeaderValue::from_str(&validator) {
                builder = builder.header(reqwest::header::IF_NONE_MATCH, value);
            }
        }
        let mut response = builder
            .send()
            .await
            .map_err(|_| CloudError::TransportFailed)?;
        let status = response.status().as_u16();
        let content_length = response.content_length();
        if content_length.is_some_and(|length| {
            usize::try_from(length).map_or(true, |length| length > MAX_RESPONSE_BYTES)
        }) {
            return Err(CloudError::TransportFailed);
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let mut response_body = Vec::with_capacity(
            content_length
                .and_then(|length| usize::try_from(length).ok())
                .unwrap_or_default(),
        );
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| CloudError::TransportFailed)?
        {
            let next_length = response_body
                .len()
                .checked_add(chunk.len())
                .ok_or(CloudError::TransportFailed)?;
            if next_length > MAX_RESPONSE_BYTES {
                return Err(CloudError::TransportFailed);
            }
            response_body.extend_from_slice(&chunk);
        }
        Ok(HttpResponse::new(status, content_type, content_length, response_body).with_etag(etag))
    }

    async fn execute_stream(
        &self,
        request: OutboundHttpRequest,
    ) -> Result<StreamingHttpResponse, CloudError> {
        let OutboundHttpRequest {
            method,
            url,
            body,
            bearer,
            idempotency_key,
            if_none_match,
        } = request;
        // A stream is opened by exactly one method. Anything else would mean the
        // caller reached this through a route that is not the reviewed one.
        if method != HttpMethod::Post || if_none_match.is_some() {
            return Err(CloudError::TransportFailed);
        }
        let response = self
            .client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .header("Idempotency-Key", idempotency_key)
            .bearer_auth(bearer.as_str())
            .body(body)
            .send()
            .await
            .map_err(|_| CloudError::TransportFailed)?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        Ok(StreamingHttpResponse::new(
            status,
            content_type,
            Box::new(ReqwestChunkSource { response }),
        ))
    }
}

impl SupportApiTransport<ReqwestHttpExecutor> {
    /// Creates the production fixed-origin HTTPS composition.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::TransportFailed`] when the bounded executor cannot
    /// be constructed.
    pub fn production(
        origin: SupportApiOrigin,
        connect_timeout: Duration,
        operation_timeout: Duration,
    ) -> Result<Self, CloudError> {
        Ok(Self::new(
            origin,
            ReqwestHttpExecutor::new(connect_timeout, operation_timeout)?,
        ))
    }
}

fn validate_response(response: &HttpResponse) -> Result<RawModelOutput, CloudError> {
    let actual_length =
        u64::try_from(response.body.len()).map_err(|_| CloudError::TransportFailed)?;
    if response.status != 200
        || response
            .content_length
            .is_some_and(|declared_length| declared_length != actual_length)
        || response.body.len() > MAX_RESPONSE_BYTES
        || !response
            .content_type
            .as_deref()
            .is_some_and(is_json_content_type)
    {
        return Err(CloudError::TransportFailed);
    }
    serde_json::from_slice(&response.body).map_err(|_| CloudError::InvalidModelOutput)
}

/// The one media-type test in this crate.
///
/// The direct-provider path used to run its own `to_ascii_lowercase()` +
/// `starts_with("application/json")`, which accepted `application/json-seq` and
/// allocated a `String` per response. Two sites deciding the same question by
/// different rules is the drift this consolidation removes.
pub(crate) fn is_json_content_type(value: &str) -> bool {
    value
        .split_once(';')
        .map_or(value, |(media_type, _)| media_type)
        .trim()
        .eq_ignore_ascii_case("application/json")
}
