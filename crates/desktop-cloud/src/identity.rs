use core::fmt;

use async_trait::async_trait;
use desktop_runtime::{ContractId, UnixMillis};
use parking_lot::Mutex;
use zeroize::Zeroizing;

use crate::{AuthStatus, CloudError};

const MIN_TOKEN_BYTES: usize = 8;
const MAX_TOKEN_BYTES: usize = 16 * 1024;

#[async_trait]
pub trait IdentityBroker: Send + Sync {
    /// Acquires one short-lived support-API token.
    ///
    /// # Errors
    ///
    /// Returns a stable [`CloudError`] without exposing raw broker details.
    async fn acquire_token(&self) -> Result<BrokerToken, CloudError>;

    /// Removes the relevant broker account/cache state.
    ///
    /// # Errors
    ///
    /// Returns a stable [`CloudError`] without reversing local invalidation.
    async fn sign_out(&self) -> Result<(), CloudError>;
}

pub struct BrokerToken {
    access_token: Zeroizing<String>,
    tenant_ref: ContractId,
    account_ref: ContractId,
    expires_at: UnixMillis,
}

impl BrokerToken {
    /// Creates a transient broker token with bounded secret bytes.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::IdentityUnavailable`] for empty, oversized, or
    /// whitespace-bearing token material.
    pub fn new(
        access_token: String,
        tenant_ref: ContractId,
        account_ref: ContractId,
        expires_at: UnixMillis,
    ) -> Result<Self, CloudError> {
        if !(MIN_TOKEN_BYTES..=MAX_TOKEN_BYTES).contains(&access_token.len())
            || access_token.bytes().any(|byte| byte.is_ascii_whitespace())
        {
            return Err(CloudError::IdentityUnavailable);
        }
        Ok(Self {
            access_token: Zeroizing::new(access_token),
            tenant_ref,
            account_ref,
            expires_at,
        })
    }
}

impl fmt::Debug for BrokerToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrokerToken")
            .field("access_token", &"[REDACTED]")
            .field("tenant_ref", &self.tenant_ref)
            .field("account_ref", &self.account_ref)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

pub struct CloudAccess {
    access_token: Zeroizing<String>,
    tenant_ref: ContractId,
    account_ref: ContractId,
    expires_at: UnixMillis,
    epoch: u64,
}

impl CloudAccess {
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn with_bearer<T>(&self, operation: impl FnOnce(&str) -> T) -> T {
        operation(self.access_token.as_str())
    }
}

impl fmt::Debug for CloudAccess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CloudAccess")
            .field("access_token", &"[REDACTED]")
            .field("tenant_ref", &self.tenant_ref)
            .field("account_ref", &self.account_ref)
            .field("expires_at", &self.expires_at)
            .field("epoch", &self.epoch)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionSnapshot {
    pub status: AuthStatus,
    pub epoch: u64,
    pub tenant_ref: Option<ContractId>,
    pub account_ref: Option<ContractId>,
}

struct SessionState {
    status: AuthStatus,
    epoch: u64,
    tenant_ref: Option<ContractId>,
    account_ref: Option<ContractId>,
}

pub struct CloudSession<B> {
    broker: B,
    expected_tenant_ref: ContractId,
    state: Mutex<SessionState>,
}

impl<B> CloudSession<B>
where
    B: IdentityBroker,
{
    #[must_use]
    pub fn new(broker: B, expected_tenant_ref: ContractId) -> Self {
        Self {
            broker,
            expected_tenant_ref,
            state: Mutex::new(SessionState {
                status: AuthStatus::SignedOut,
                epoch: 0,
                tenant_ref: None,
                account_ref: None,
            }),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> SessionSnapshot {
        let state = self.state.lock();
        SessionSnapshot {
            status: state.status.clone(),
            epoch: state.epoch,
            tenant_ref: state.tenant_ref.clone(),
            account_ref: state.account_ref.clone(),
        }
    }

    /// Acquires one transient access grant for the configured tenant.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError`] when broker acquisition fails, the token is
    /// expired or tenant-substituted, or the session changes during acquisition.
    pub async fn acquire_access(&self, now: UnixMillis) -> Result<CloudAccess, CloudError> {
        let starting_epoch = {
            let mut state = self.state.lock();
            state.status = AuthStatus::SigningIn;
            state.epoch
        };
        let token = match self.broker.acquire_token().await {
            Ok(token) => token,
            Err(error) => {
                let mut state = self.state.lock();
                if state.epoch == starting_epoch {
                    state.status = AuthStatus::Unavailable;
                }
                return Err(error);
            }
        };
        let mut state = self.state.lock();
        if state.epoch != starting_epoch {
            return Err(CloudError::SessionInvalidated);
        }
        if token.tenant_ref != self.expected_tenant_ref {
            invalidate(&mut state, AuthStatus::ReauthenticationRequired)?;
            return Err(CloudError::TenantMismatch);
        }
        if token.expires_at <= now {
            invalidate(&mut state, AuthStatus::ReauthenticationRequired)?;
            return Err(CloudError::ReauthenticationRequired);
        }
        if state
            .account_ref
            .as_ref()
            .is_some_and(|account_ref| account_ref != &token.account_ref)
        {
            state.epoch = state
                .epoch
                .checked_add(1)
                .ok_or(CloudError::SessionInvalidated)?;
        }
        state.status = AuthStatus::SignedIn;
        state.tenant_ref = Some(token.tenant_ref.clone());
        state.account_ref = Some(token.account_ref.clone());
        Ok(CloudAccess {
            access_token: token.access_token,
            tenant_ref: token.tenant_ref,
            account_ref: token.account_ref,
            expires_at: token.expires_at,
            epoch: state.epoch,
        })
    }

    #[must_use]
    pub fn is_current(&self, access: &CloudAccess) -> bool {
        let state = self.state.lock();
        state.status == AuthStatus::SignedIn
            && state.epoch == access.epoch
            && state.tenant_ref.as_ref() == Some(&access.tenant_ref)
            && state.account_ref.as_ref() == Some(&access.account_ref)
    }

    /// Invalidates local authority before requesting broker cleanup.
    ///
    /// # Errors
    ///
    /// Returns a broker cleanup error after local invalidation has committed.
    pub async fn sign_out(&self) -> Result<(), CloudError> {
        {
            let mut state = self.state.lock();
            invalidate(&mut state, AuthStatus::SignedOut)?;
        }
        self.broker.sign_out().await
    }
}

fn invalidate(state: &mut SessionState, status: AuthStatus) -> Result<(), CloudError> {
    state.epoch = state
        .epoch
        .checked_add(1)
        .ok_or(CloudError::SessionInvalidated)?;
    state.status = status;
    state.tenant_ref = None;
    state.account_ref = None;
    Ok(())
}
