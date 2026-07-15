#![allow(
    clippy::expect_used,
    reason = "invalid static fixtures should fail the test at construction"
)]

use desktop_cloud::{BrokerOutcome, BrokerProtocol, CloudError};
use desktop_runtime::{ContractId, UnixMillis};
use serde_json::{json, Value};

#[cfg(windows)]
use desktop_cloud::WindowsBrokerConfig;
#[cfg(windows)]
use std::{path::PathBuf, time::Duration};

const CLIENT_ID: &str = "11111111-1111-4111-8111-111111111111";
const TENANT_ID: &str = "22222222-2222-4222-8222-222222222222";
const SCOPE: &str = "api://33333333-3333-4333-8333-333333333333/access_as_user";

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("valid fixture identifier")
}

fn protocol() -> BrokerProtocol {
    BrokerProtocol::new(CLIENT_ID, TENANT_ID, [SCOPE]).expect("valid protocol configuration")
}

fn frame(value: &Value) -> Vec<u8> {
    let payload = serde_json::to_vec(value).expect("serialize response fixture");
    let mut framed = Vec::with_capacity(payload.len() + 4);
    framed.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("bounded length")
            .to_be_bytes(),
    );
    framed.extend_from_slice(&payload);
    framed
}

fn success_response(request_id: &str) -> Value {
    json!({
        "protocolVersion": "sapphirus.auth-broker.v1",
        "requestId": request_id,
        "success": true,
        "errorCode": null,
        "retryable": false,
        "accessToken": "top-secret-token",
        "expiresOn": "2026-07-15T14:00:00Z",
        "accountId": "account.home-id",
        "tenantId": TENANT_ID
    })
}

#[test]
fn acquire_request_has_a_fixed_authority_scope_and_bounded_parent_window() {
    let exchange = protocol()
        .acquire_exchange(id("request_1234"), 0x1234, None, false)
        .expect("acquire exchange");
    let payload = exchange.request_payload();
    let request: Value = serde_json::from_slice(payload).expect("request JSON");

    assert_eq!(request["protocolVersion"], "sapphirus.auth-broker.v1");
    assert_eq!(request["operation"], "acquire_token");
    assert_eq!(request["clientId"], CLIENT_ID);
    assert_eq!(
        request["authority"],
        format!("https://login.microsoftonline.com/{TENANT_ID}/v2.0")
    );
    assert_eq!(request["scopes"], json!([SCOPE]));
    assert_eq!(request["parentWindowHandle"], "0x1234");
    assert_eq!(request["allowSystemBrowserFallback"], false);
    assert!(protocol()
        .acquire_exchange(id("request_1234"), 0, None, false)
        .is_err());
}

#[test]
fn successful_response_is_correlated_tenant_bound_and_secret_safe() {
    let exchange = protocol()
        .acquire_exchange(id("request_1234"), 0x1234, None, false)
        .expect("exchange");
    let outcome = exchange
        .accept_response(&frame(&success_response("request_1234")), UnixMillis(1_000))
        .expect("accepted token");

    assert!(matches!(outcome, BrokerOutcome::Token(_)));
    let debug = format!("{outcome:?}");
    assert!(!debug.contains("top-secret-token"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn malformed_duplicate_oversized_and_substituted_responses_fail_closed() {
    let exchange = || {
        protocol()
            .acquire_exchange(id("request_1234"), 0x1234, None, false)
            .expect("exchange")
    };

    let mut wrong_request = success_response("request_9999");
    assert!(matches!(
        exchange().accept_response(&frame(&wrong_request), UnixMillis(1_000)),
        Err(CloudError::IdentityUnavailable)
    ));
    wrong_request["requestId"] = json!("request_1234");
    wrong_request["tenantId"] = json!("44444444-4444-4444-8444-444444444444");
    assert!(matches!(
        exchange().accept_response(&frame(&wrong_request), UnixMillis(1_000)),
        Err(CloudError::TenantMismatch)
    ));

    let duplicate = "{\"protocolVersion\":\"sapphirus.auth-broker.v1\",\"requestId\":\"request_1234\",\"requestId\":\"request_9999\",\"success\":false,\"errorCode\":\"authentication_failed\",\"retryable\":false,\"accessToken\":null,\"expiresOn\":null,\"accountId\":null,\"tenantId\":null}"
        .to_owned();
    let mut duplicate_frame = Vec::new();
    duplicate_frame.extend_from_slice(
        &u32::try_from(duplicate.len())
            .expect("bounded")
            .to_be_bytes(),
    );
    duplicate_frame.extend_from_slice(duplicate.as_bytes());
    assert!(matches!(
        exchange().accept_response(&duplicate_frame, UnixMillis(1_000)),
        Err(CloudError::IdentityUnavailable)
    ));

    let mut oversized = vec![0_u8; 4];
    oversized.copy_from_slice(&(65_537_u32).to_be_bytes());
    assert!(matches!(
        exchange().accept_response(&oversized, UnixMillis(1_000)),
        Err(CloudError::IdentityUnavailable)
    ));
}

#[test]
fn expired_tokens_and_stable_broker_failures_are_mapped_without_raw_details() {
    let exchange = || {
        protocol()
            .acquire_exchange(id("request_1234"), 0x1234, None, false)
            .expect("exchange")
    };
    let expired_at = UnixMillis(1_800_000_000_000);
    assert!(matches!(
        exchange().accept_response(&frame(&success_response("request_1234")), expired_at),
        Err(CloudError::ReauthenticationRequired)
    ));

    let failure = json!({
        "protocolVersion": "sapphirus.auth-broker.v1",
        "requestId": "request_1234",
        "success": false,
        "errorCode": "reauthentication_required",
        "retryable": false,
        "accessToken": null,
        "expiresOn": null,
        "accountId": null,
        "tenantId": null
    });
    assert!(matches!(
        exchange().accept_response(&frame(&failure), UnixMillis(1_000)),
        Err(CloudError::ReauthenticationRequired)
    ));
}

#[test]
fn sign_out_requires_a_success_response_without_token_material() {
    let exchange = protocol()
        .sign_out_exchange(id("request_1234"), Some("account.home-id"))
        .expect("sign-out exchange");
    let request: Value = serde_json::from_slice(exchange.request_payload()).expect("request JSON");
    assert_eq!(request["operation"], "sign_out");
    assert_eq!(request["accountId"], "account.home-id");
    assert_eq!(request["parentWindowHandle"], Value::Null);

    let response = json!({
        "protocolVersion": "sapphirus.auth-broker.v1",
        "requestId": "request_1234",
        "success": true,
        "errorCode": null,
        "retryable": false,
        "accessToken": null,
        "expiresOn": null,
        "accountId": null,
        "tenantId": null
    });
    assert!(matches!(
        exchange.accept_response(&frame(&response), UnixMillis(1_000)),
        Ok(BrokerOutcome::SignedOut)
    ));
}

#[cfg(windows)]
#[test]
fn windows_adapter_accepts_only_a_fixed_packaged_helper_and_bounded_operation() {
    let helper = PathBuf::from(r"C:\Program Files\Sapphirus\Sapphirus.WindowsAuthBroker.exe");
    assert!(
        WindowsBrokerConfig::new(helper, protocol(), 0x1234, false, Duration::from_secs(30),)
            .is_ok()
    );
    assert!(WindowsBrokerConfig::new(
        PathBuf::from("Sapphirus.WindowsAuthBroker.exe"),
        protocol(),
        0x1234,
        false,
        Duration::from_secs(30),
    )
    .is_err());
    assert!(WindowsBrokerConfig::new(
        PathBuf::from(r"C:\Program Files\Sapphirus\other.exe"),
        protocol(),
        0x1234,
        false,
        Duration::from_secs(30),
    )
    .is_err());
}
