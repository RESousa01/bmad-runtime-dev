use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use desktop_runtime::{ContractId, UnixMillis};
use rand::RngCore;
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::ServerOptions;
use tokio::process::Command;
use tokio::time::timeout;
use zeroize::Zeroizing;

use crate::{BrokerOutcome, BrokerProtocol, BrokerToken, CloudError, IdentityBroker};

const HELPER_FILENAME: &str = "Sapphirus.WindowsAuthBroker.exe";
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const MIN_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_TIMEOUT: Duration = Duration::from_mins(5);

#[derive(Clone, Debug)]
pub struct WindowsBrokerConfig {
    helper_path: PathBuf,
    protocol: BrokerProtocol,
    parent_window_handle: u64,
    allow_system_browser_fallback: bool,
    operation_timeout: Duration,
}

impl WindowsBrokerConfig {
    /// Seals fixed helper and identity settings for one desktop composition.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::IdentityUnavailable`] when the helper path is not
    /// absolute/package-shaped, the window handle is zero, or the timeout is
    /// outside the broker protocol's bounded operation window.
    pub fn new(
        helper_path: impl Into<PathBuf>,
        protocol: BrokerProtocol,
        parent_window_handle: u64,
        allow_system_browser_fallback: bool,
        operation_timeout: Duration,
    ) -> Result<Self, CloudError> {
        let helper_path = helper_path.into();
        if !valid_helper_path(&helper_path)
            || parent_window_handle == 0
            || !(MIN_TIMEOUT..=MAX_TIMEOUT).contains(&operation_timeout)
        {
            return Err(CloudError::IdentityUnavailable);
        }
        Ok(Self {
            helper_path,
            protocol,
            parent_window_handle,
            allow_system_browser_fallback,
            operation_timeout,
        })
    }
}

#[derive(Debug)]
pub struct WindowsIdentityBroker {
    config: WindowsBrokerConfig,
}

impl WindowsIdentityBroker {
    #[must_use]
    pub const fn new(config: WindowsBrokerConfig) -> Self {
        Self { config }
    }

    async fn run_exchange(
        &self,
        exchange: &crate::BrokerExchange,
    ) -> Result<Zeroizing<Vec<u8>>, CloudError> {
        let pipe_name = random_pipe_name();
        let pipe_path = format!(r"\\.\pipe\{pipe_name}");
        let mut pipe = ServerOptions::new()
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .create(&pipe_path)
            .map_err(|_| CloudError::IdentityUnavailable)?;
        let mut child = Command::new(&self.config.helper_path)
            .arg(&pipe_name)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| CloudError::IdentityUnavailable)?;
        let request = exchange.request_frame()?;
        let operation = async {
            pipe.connect()
                .await
                .map_err(|_| CloudError::IdentityUnavailable)?;
            pipe.write_all(&request)
                .await
                .map_err(|_| CloudError::IdentityUnavailable)?;
            pipe.flush()
                .await
                .map_err(|_| CloudError::IdentityUnavailable)?;

            let mut length_bytes = [0_u8; size_of::<u32>()];
            pipe.read_exact(&mut length_bytes)
                .await
                .map_err(|_| CloudError::IdentityUnavailable)?;
            let length = usize::try_from(u32::from_be_bytes(length_bytes))
                .map_err(|_| CloudError::IdentityUnavailable)?;
            if length == 0 || length > MAX_MESSAGE_BYTES {
                return Err(CloudError::IdentityUnavailable);
            }
            let mut response = Zeroizing::new(vec![0_u8; length + size_of::<u32>()]);
            response[..size_of::<u32>()].copy_from_slice(&length_bytes);
            pipe.read_exact(&mut response[size_of::<u32>()..])
                .await
                .map_err(|_| CloudError::IdentityUnavailable)?;
            child
                .wait()
                .await
                .map_err(|_| CloudError::IdentityUnavailable)?;
            Ok(response)
        };
        timeout(self.config.operation_timeout, operation)
            .await
            .map_err(|_| CloudError::IdentityUnavailable)?
    }
}

#[async_trait]
impl IdentityBroker for WindowsIdentityBroker {
    async fn acquire_token(&self) -> Result<BrokerToken, CloudError> {
        let exchange = self.config.protocol.acquire_exchange(
            random_request_id()?,
            self.config.parent_window_handle,
            None,
            self.config.allow_system_browser_fallback,
        )?;
        let response = self.run_exchange(&exchange).await?;
        match exchange.accept_response(&response, now_millis()?)? {
            BrokerOutcome::Token(token) => Ok(token),
            BrokerOutcome::SignedOut => Err(CloudError::IdentityUnavailable),
        }
    }

    async fn sign_out(&self) -> Result<(), CloudError> {
        let exchange = self
            .config
            .protocol
            .sign_out_exchange(random_request_id()?, None)?;
        let response = self.run_exchange(&exchange).await?;
        match exchange.accept_response(&response, now_millis()?)? {
            BrokerOutcome::SignedOut => Ok(()),
            BrokerOutcome::Token(_) => Err(CloudError::IdentityUnavailable),
        }
    }
}

fn valid_helper_path(path: &Path) -> bool {
    path.is_absolute()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(HELPER_FILENAME))
}

fn random_pipe_name() -> String {
    format!("sapphirus-auth-{}", random_hex())
}

fn random_request_id() -> Result<ContractId, CloudError> {
    ContractId::new(format!("request_{}", random_hex()))
        .map_err(|_| CloudError::IdentityUnavailable)
}

fn random_hex() -> String {
    let mut bytes = [0_u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn now_millis() -> Result<UnixMillis, CloudError> {
    let millis = OffsetDateTime::now_utc()
        .unix_timestamp_nanos()
        .checked_div(1_000_000)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(CloudError::IdentityUnavailable)?;
    Ok(UnixMillis(millis))
}
