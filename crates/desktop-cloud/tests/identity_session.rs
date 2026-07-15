#![allow(
    clippy::expect_used,
    reason = "invalid static fixtures should fail the test at construction"
)]

use async_trait::async_trait;
use desktop_cloud::{
    AuthStatus, BrokerToken, CloudError, CloudSession, IdentityBroker, SessionSnapshot,
};
use desktop_runtime::{ContractId, UnixMillis};
use parking_lot::Mutex;

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("valid fixture identifier")
}

struct StaticBroker {
    access_token: String,
    tenant_ref: ContractId,
    account_ref: ContractId,
    sign_out_fails: bool,
}

impl StaticBroker {
    fn successful(access_token: &str, tenant_ref: &str, account_ref: &str) -> Self {
        Self {
            access_token: access_token.to_owned(),
            tenant_ref: id(tenant_ref),
            account_ref: id(account_ref),
            sign_out_fails: false,
        }
    }

    fn sign_out_failure() -> Self {
        Self {
            sign_out_fails: true,
            ..Self::successful("secret-token", "tenant_ref", "account_ref")
        }
    }
}

#[async_trait]
impl IdentityBroker for StaticBroker {
    async fn acquire_token(&self) -> Result<BrokerToken, CloudError> {
        BrokerToken::new(
            self.access_token.clone(),
            self.tenant_ref.clone(),
            self.account_ref.clone(),
            UnixMillis(60_000),
        )
    }

    async fn sign_out(&self) -> Result<(), CloudError> {
        if self.sign_out_fails {
            Err(CloudError::IdentityUnavailable)
        } else {
            Ok(())
        }
    }
}

struct AccountSequenceBroker {
    accounts: Mutex<Vec<ContractId>>,
}

#[async_trait]
impl IdentityBroker for AccountSequenceBroker {
    async fn acquire_token(&self) -> Result<BrokerToken, CloudError> {
        let account_ref = self
            .accounts
            .lock()
            .pop()
            .ok_or(CloudError::IdentityUnavailable)?;
        BrokerToken::new(
            "secret-token".to_owned(),
            id("tenant_ref"),
            account_ref,
            UnixMillis(60_000),
        )
    }

    async fn sign_out(&self) -> Result<(), CloudError> {
        Ok(())
    }
}

#[tokio::test]
async fn access_token_debug_is_redacted_and_sign_out_invalidates_the_epoch() {
    let session = CloudSession::new(
        StaticBroker::successful("secret-token", "tenant_ref", "account_ref"),
        id("tenant_ref"),
    );
    let access = session
        .acquire_access(UnixMillis(1_000))
        .await
        .expect("access");

    assert!(!format!("{access:?}").contains("secret-token"));
    assert_eq!(access.with_bearer(str::len), "secret-token".len());
    assert!(session.is_current(&access));
    session.sign_out().await.expect("sign out");
    assert!(!session.is_current(&access));
    assert_eq!(
        session.snapshot(),
        SessionSnapshot {
            status: AuthStatus::SignedOut,
            epoch: 1,
            tenant_ref: None,
            account_ref: None,
        }
    );
}

#[tokio::test]
async fn local_sign_out_remains_terminal_when_broker_cleanup_fails() {
    let session = CloudSession::new(StaticBroker::sign_out_failure(), id("tenant_ref"));
    let before = session.snapshot().epoch;

    assert_eq!(
        session.sign_out().await,
        Err(CloudError::IdentityUnavailable)
    );
    assert_eq!(session.snapshot().epoch, before + 1);
    assert_eq!(session.snapshot().status, AuthStatus::SignedOut);
}

#[tokio::test]
async fn tenant_substitution_never_returns_an_access_grant() {
    let session = CloudSession::new(
        StaticBroker::successful("secret-token", "other_tenant", "account_ref"),
        id("tenant_ref"),
    );

    assert!(matches!(
        session.acquire_access(UnixMillis(1_000)).await,
        Err(CloudError::TenantMismatch)
    ));
    assert_eq!(
        session.snapshot().status,
        AuthStatus::ReauthenticationRequired
    );
}

#[tokio::test]
async fn expired_broker_token_is_rejected() {
    let session = CloudSession::new(
        StaticBroker::successful("secret-token", "tenant_ref", "account_ref"),
        id("tenant_ref"),
    );

    assert!(matches!(
        session.acquire_access(UnixMillis(60_000)).await,
        Err(CloudError::ReauthenticationRequired)
    ));
}

#[tokio::test]
async fn account_change_invalidates_the_previous_access_epoch() {
    let session = CloudSession::new(
        AccountSequenceBroker {
            accounts: Mutex::new(vec![id("account_b"), id("account_a")]),
        },
        id("tenant_ref"),
    );
    let first = session
        .acquire_access(UnixMillis(1_000))
        .await
        .expect("first access");
    let second = session
        .acquire_access(UnixMillis(1_000))
        .await
        .expect("second access");

    assert!(!session.is_current(&first));
    assert!(session.is_current(&second));
    assert_eq!(session.snapshot().epoch, first.epoch() + 1);
}
