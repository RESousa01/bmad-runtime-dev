//! Bounded stage-exchange coverage (readiness Task 9 foundation): the
//! reviewed route set, method binding, and fail-closed response handling.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Mutex;

use async_trait::async_trait;
use desktop_cloud::{
    CloudError, HttpExecutor, HttpMethod, HttpResponse, OutboundHttpRequest, SupportApiOrigin,
    SupportApiTransport, SupportStageRoute,
};
use zeroize::Zeroizing;

struct ScriptedExecutor {
    seen: Mutex<Vec<(HttpMethod, String, usize)>>,
    response: Box<dyn Fn() -> HttpResponse + Send + Sync>,
}

#[async_trait]
impl HttpExecutor for ScriptedExecutor {
    async fn execute(&self, request: OutboundHttpRequest) -> Result<HttpResponse, CloudError> {
        self.seen.lock().expect("seen lock").push((
            request.method(),
            request.url().as_str().to_owned(),
            request.body().len(),
        ));
        Ok((self.response)())
    }
}

fn transport(
    response: impl Fn() -> HttpResponse + Send + Sync + 'static,
) -> SupportApiTransport<ScriptedExecutor> {
    let origin = SupportApiOrigin::new("https://support.example.test").expect("origin");
    SupportApiTransport::new(
        origin,
        ScriptedExecutor {
            seen: Mutex::new(Vec::new()),
            response: Box::new(response),
        },
    )
}

fn json_ok(body: &str) -> HttpResponse {
    HttpResponse::new(
        200,
        Some("application/json".to_owned()),
        Some(body.len() as u64),
        body.as_bytes().to_vec(),
    )
}

#[tokio::test]
async fn stage_routes_bind_their_reviewed_paths_and_methods() {
    for (route, expected_path, expected_method) in [
        (
            SupportStageRoute::Bootstrap,
            "/desktop/v1/bootstrap",
            HttpMethod::Get,
        ),
        (
            SupportStageRoute::DeviceRegistrations,
            "/desktop/v1/devices/registrations",
            HttpMethod::Post,
        ),
        (
            SupportStageRoute::PolicyCurrent,
            "/desktop/v1/policy/current",
            HttpMethod::Get,
        ),
        (
            SupportStageRoute::EntitlementLeases,
            "/desktop/v1/entitlements/leases",
            HttpMethod::Post,
        ),
    ] {
        let transport = transport(|| json_ok("{\"ok\":true}"));
        let body = if matches!(expected_method, HttpMethod::Post) {
            b"{\"request\":1}".to_vec()
        } else {
            Vec::new()
        };
        let bytes = transport
            .exchange_stage(route, Zeroizing::new("token".to_owned()), body, "idem_1")
            .await
            .expect("stage exchange");
        assert_eq!(bytes, b"{\"ok\":true}");
        let seen = transport.executor_for_test().seen.lock().expect("seen");
        let (method, url, _) = seen.first().expect("one request");
        assert_eq!(*method, expected_method, "{expected_path}");
        assert!(url.ends_with(expected_path), "{url}");
        // The stage URL never inherits the model endpoint path, a query,
        // or a fragment.
        assert!(!url.contains("model-access"));
        assert!(!url.contains('?'));
        assert!(!url.contains('#'));
    }
}

#[tokio::test]
async fn stage_exchange_fails_closed_on_untrusted_responses() {
    // Non-2xx status.
    let failing = transport(|| {
        HttpResponse::new(
            500,
            Some("application/json".to_owned()),
            Some(2),
            b"{}".to_vec(),
        )
    });
    assert!(matches!(
        failing
            .exchange_stage(
                SupportStageRoute::Bootstrap,
                Zeroizing::new("token".to_owned()),
                Vec::new(),
                "idem_1",
            )
            .await,
        Err(CloudError::TransportFailed)
    ));

    // Redirects are not followed into trust.
    let redirect = transport(|| HttpResponse::new(302, None, Some(0), Vec::new()));
    assert!(redirect
        .exchange_stage(
            SupportStageRoute::PolicyCurrent,
            Zeroizing::new("token".to_owned()),
            Vec::new(),
            "idem_2",
        )
        .await
        .is_err());

    // Non-JSON bodies are opaque.
    let html = transport(|| {
        HttpResponse::new(
            200,
            Some("text/html".to_owned()),
            Some(6),
            b"<html>".to_vec(),
        )
    });
    assert!(html
        .exchange_stage(
            SupportStageRoute::Bootstrap,
            Zeroizing::new("token".to_owned()),
            Vec::new(),
            "idem_3",
        )
        .await
        .is_err());

    // GET stages reject smuggled bodies before any I/O.
    let get_with_body = transport(|| json_ok("{}"));
    assert!(get_with_body
        .exchange_stage(
            SupportStageRoute::PolicyCurrent,
            Zeroizing::new("token".to_owned()),
            b"{\"x\":1}".to_vec(),
            "idem_4",
        )
        .await
        .is_err());
    assert!(get_with_body
        .executor_for_test()
        .seen
        .lock()
        .expect("seen")
        .is_empty());

    // Oversized stage requests fail before any I/O.
    let oversized = transport(|| json_ok("{}"));
    assert!(oversized
        .exchange_stage(
            SupportStageRoute::EntitlementLeases,
            Zeroizing::new("token".to_owned()),
            vec![b'x'; 64 * 1024 + 1],
            "idem_5",
        )
        .await
        .is_err());
    assert!(oversized
        .executor_for_test()
        .seen
        .lock()
        .expect("seen")
        .is_empty());
}
