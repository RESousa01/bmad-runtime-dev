use std::path::{Component, Path, PathBuf};
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

use crate::{
    broker_protocol::PROTOCOL_VERSION, BrokerOutcome, BrokerProtocol, BrokerToken, CloudError,
    IdentityBroker,
};

const HELPER_FILENAME: &str = "Sapphirus.WindowsAuthBroker.exe";
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const MIN_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_TIMEOUT: Duration = Duration::from_mins(5);

#[derive(Clone, Debug)]
pub struct WindowsBrokerConfig {
    package_root: PathBuf,
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
        package_root: impl Into<PathBuf>,
        protocol: BrokerProtocol,
        parent_window_handle: u64,
        allow_system_browser_fallback: bool,
        operation_timeout: Duration,
    ) -> Result<Self, CloudError> {
        let package_root = package_root.into();
        if !valid_package_root(&package_root)
            || parent_window_handle == 0
            || !(MIN_TIMEOUT..=MAX_TIMEOUT).contains(&operation_timeout)
        {
            return Err(CloudError::IdentityUnavailable);
        }
        let helper_path = package_root.join(HELPER_FILENAME);
        Ok(Self {
            package_root,
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

#[derive(Debug, Eq, PartialEq)]
struct BrokerLaunchSpec {
    program: PathBuf,
    current_dir: PathBuf,
    arguments: [String; 2],
    inherit_environment: bool,
    kill_on_drop: bool,
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
        let package_root = std::fs::canonicalize(&self.config.package_root)
            .map_err(|_| CloudError::IdentityUnavailable)?;
        let helper_path = std::fs::canonicalize(&self.config.helper_path)
            .map_err(|_| CloudError::IdentityUnavailable)?;
        if !canonical_helper_is_direct_child(&package_root, &helper_path) {
            return Err(CloudError::IdentityUnavailable);
        }
        let pipe_name = random_pipe_name();
        let pipe_path = format!(r"\\.\pipe\{pipe_name}");
        let mut options = ServerOptions::new();
        options
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .max_instances(1);
        let mut pipe = windows_pipe_security::create_current_user_pipe(&options, &pipe_path)
            .map_err(|_| CloudError::IdentityUnavailable)?;
        let launch = broker_launch_spec(helper_path, package_root, &pipe_name);
        let mut command = Command::new(&launch.program);
        command
            .args(&launch.arguments)
            .current_dir(&launch.current_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if !launch.inherit_environment {
            command.env_clear();
        }
        command.kill_on_drop(launch.kill_on_drop);
        let mut child = command
            .spawn()
            .map_err(|_| CloudError::IdentityUnavailable)?;
        let expected_child_pid = child.id().ok_or(CloudError::IdentityUnavailable)?;
        let request = exchange.request_frame()?;
        let operation = async {
            pipe.connect()
                .await
                .map_err(|_| CloudError::IdentityUnavailable)?;
            let connected_pid = windows_pipe_security::connected_client_pid(&pipe)
                .map_err(|_| CloudError::IdentityUnavailable)?;
            validate_connected_client(expected_child_pid, connected_pid)?;
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
            let status = child
                .wait()
                .await
                .map_err(|_| CloudError::IdentityUnavailable)?;
            validate_broker_completion(
                status.success(),
                windows_pipe_security::has_trailing_bytes(&mut pipe).await?,
            )?;
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

fn valid_package_root(path: &Path) -> bool {
    path.is_absolute()
        && path.file_name().is_some()
        && !path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
}

fn valid_helper_filename(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(HELPER_FILENAME))
}

fn canonical_helper_is_direct_child(package_root: &Path, helper_path: &Path) -> bool {
    helper_path.parent() == Some(package_root) && valid_helper_filename(helper_path)
}

fn broker_launch_spec(
    helper_path: PathBuf,
    package_root: PathBuf,
    pipe_name: &str,
) -> BrokerLaunchSpec {
    BrokerLaunchSpec {
        program: helper_path,
        current_dir: package_root,
        arguments: [pipe_name.to_owned(), PROTOCOL_VERSION.to_owned()],
        inherit_environment: false,
        kill_on_drop: true,
    }
}

fn validate_connected_client(expected_pid: u32, connected_pid: u32) -> Result<(), CloudError> {
    if connected_pid == 0 || connected_pid != expected_pid {
        return Err(CloudError::IdentityUnavailable);
    }
    Ok(())
}

fn validate_broker_completion(
    exit_succeeded: bool,
    trailing_bytes: bool,
) -> Result<(), CloudError> {
    if !exit_succeeded || trailing_bytes {
        return Err(CloudError::IdentityUnavailable);
    }
    Ok(())
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

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "static Windows adapter fixtures should fail at the exact operation"
)]
mod launch_tests {
    use std::path::PathBuf;

    use super::{
        broker_launch_spec, canonical_helper_is_direct_child, validate_broker_completion,
        validate_connected_client, CloudError, HELPER_FILENAME, PROTOCOL_VERSION,
    };

    #[test]
    fn launch_is_fixed_protocol_environment_and_kill_on_timeout() {
        let root = PathBuf::from(r"C:\Program Files\Sapphirus");
        let helper = root.join(HELPER_FILENAME);
        let launch = broker_launch_spec(helper.clone(), root.clone(), "sapphirus-pipe-1234");

        assert_eq!(launch.program, helper);
        assert_eq!(launch.current_dir, root);
        assert_eq!(
            launch.arguments,
            [
                "sapphirus-pipe-1234".to_owned(),
                PROTOCOL_VERSION.to_owned()
            ]
        );
        assert!(!launch.inherit_environment);
        assert!(launch.kill_on_drop);
    }

    #[test]
    fn canonicalized_helper_must_remain_a_direct_packaged_child() {
        let root = PathBuf::from(r"C:\Program Files\Sapphirus");
        assert!(canonical_helper_is_direct_child(
            &root,
            &root.join(HELPER_FILENAME)
        ));
        assert!(!canonical_helper_is_direct_child(
            &root,
            &PathBuf::from(r"C:\outside").join(HELPER_FILENAME)
        ));
        assert!(!canonical_helper_is_direct_child(
            &root,
            &root.join("nested").join(HELPER_FILENAME)
        ));
    }

    #[test]
    fn foreign_or_zero_client_pid_is_rejected() {
        assert_eq!(validate_connected_client(42, 42), Ok(()));
        assert_eq!(
            validate_connected_client(42, 7),
            Err(CloudError::IdentityUnavailable)
        );
        assert_eq!(
            validate_connected_client(42, 0),
            Err(CloudError::IdentityUnavailable)
        );
    }

    #[test]
    fn nonzero_exit_and_trailing_bytes_are_rejected() {
        assert_eq!(validate_broker_completion(true, false), Ok(()));
        assert_eq!(
            validate_broker_completion(false, false),
            Err(CloudError::IdentityUnavailable)
        );
        assert_eq!(
            validate_broker_completion(true, true),
            Err(CloudError::IdentityUnavailable)
        );
    }
}

#[allow(unsafe_code)]
mod windows_pipe_security {
    use core::ffi::c_void;
    use std::io;
    use std::os::windows::io::AsRawHandle;

    use tokio::io::AsyncReadExt;
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, LocalFree, ERROR_BROKEN_PIPE, HANDLE, HLOCAL};
    use windows::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    };
    use windows::Win32::Security::{
        GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
        TOKEN_USER,
    };
    use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                let _ = unsafe { CloseHandle(self.0) };
            }
        }
    }

    struct CurrentUserSecurityAttributes {
        descriptor: PSECURITY_DESCRIPTOR,
        attributes: SECURITY_ATTRIBUTES,
    }

    impl CurrentUserSecurityAttributes {
        fn new() -> io::Result<Self> {
            let sddl = current_user_only_sddl()?;
            let wide_sddl: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
            let mut descriptor = PSECURITY_DESCRIPTOR::default();
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    PCWSTR(wide_sddl.as_ptr()),
                    SDDL_REVISION_1,
                    &raw mut descriptor,
                    None,
                )
            }
            .map_err(io::Error::other)?;
            if descriptor.is_invalid() {
                return Err(io::Error::other("invalid pipe security descriptor"));
            }
            let attributes = SECURITY_ATTRIBUTES {
                nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>())
                    .map_err(|_| io::Error::other("security attributes size overflow"))?,
                lpSecurityDescriptor: descriptor.0,
                bInheritHandle: false.into(),
            };
            Ok(Self {
                descriptor,
                attributes,
            })
        }

        fn as_raw(&mut self) -> *mut c_void {
            (&raw mut self.attributes).cast()
        }
    }

    fn current_user_only_sddl() -> io::Result<String> {
        Ok(format!("D:P(A;;GA;;;{})", current_process_user_sid()?))
    }

    fn current_process_user_sid() -> io::Result<String> {
        let mut token = HANDLE::default();
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) }
            .map_err(io::Error::other)?;
        let token = OwnedHandle(token);

        let mut required_bytes = 0_u32;
        let _ =
            unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &raw mut required_bytes) };
        if required_bytes == 0 {
            return Err(io::Error::other("current user token has no SID data"));
        }
        let required_bytes_usize = usize::try_from(required_bytes)
            .map_err(|_| io::Error::other("token information size overflow"))?;
        let word_count = required_bytes_usize.div_ceil(size_of::<usize>());
        let mut aligned_buffer = vec![0_usize; word_count];
        unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                Some(aligned_buffer.as_mut_ptr().cast()),
                required_bytes,
                &raw mut required_bytes,
            )
        }
        .map_err(io::Error::other)?;
        let token_user = unsafe { &*aligned_buffer.as_ptr().cast::<TOKEN_USER>() };
        if token_user.User.Sid.is_invalid() {
            return Err(io::Error::other("current user SID is invalid"));
        }

        let mut sid_text = PWSTR::null();
        unsafe { ConvertSidToStringSidW(token_user.User.Sid, &raw mut sid_text) }
            .map_err(io::Error::other)?;
        if sid_text.is_null() {
            return Err(io::Error::other("current user SID text is invalid"));
        }
        let sid = unsafe { sid_text.to_string() }.map_err(io::Error::other);
        let _ = unsafe { LocalFree(Some(HLOCAL(sid_text.0.cast()))) };
        sid
    }

    impl Drop for CurrentUserSecurityAttributes {
        fn drop(&mut self) {
            let _ = unsafe { LocalFree(Some(HLOCAL(self.descriptor.0))) };
        }
    }

    pub(super) fn create_current_user_pipe(
        options: &ServerOptions,
        path: &str,
    ) -> io::Result<NamedPipeServer> {
        let mut security = CurrentUserSecurityAttributes::new()?;
        unsafe { options.create_with_security_attributes_raw(path, security.as_raw()) }
    }

    pub(super) fn connected_client_pid(pipe: &NamedPipeServer) -> io::Result<u32> {
        let mut pid = 0_u32;
        unsafe {
            GetNamedPipeClientProcessId(HANDLE(pipe.as_raw_handle()), &raw mut pid)
                .map_err(io::Error::other)?;
        }
        if pid == 0 {
            return Err(io::Error::other("named pipe client has no process id"));
        }
        Ok(pid)
    }

    pub(super) async fn has_trailing_bytes(
        pipe: &mut NamedPipeServer,
    ) -> Result<bool, crate::CloudError> {
        let mut trailing = [0_u8; 1];
        match pipe.read(&mut trailing).await {
            Ok(0) => Ok(false),
            Ok(_) => Ok(true),
            Err(error) if error.raw_os_error() == Some(ERROR_BROKEN_PIPE.0.cast_signed()) => {
                Ok(false)
            }
            Err(_) => Err(crate::CloudError::IdentityUnavailable),
        }
    }

    #[cfg(test)]
    #[allow(
        clippy::expect_used,
        reason = "static Windows adapter fixtures should fail at the exact operation"
    )]
    mod tests {
        use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

        #[tokio::test]
        async fn current_user_pipe_accepts_current_process_and_reports_its_pid() {
            let path = format!(
                r"\\.\pipe\sapphirus-security-test-{}-{}",
                std::process::id(),
                super::super::random_hex()
            );
            let mut options = ServerOptions::new();
            options
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .max_instances(1);
            let server = super::create_current_user_pipe(&options, &path).expect("owner-only pipe");
            let _client = ClientOptions::new().open(&path).expect("same-user client");
            server.connect().await.expect("connect");

            assert_eq!(
                super::connected_client_pid(&server).expect("client pid"),
                std::process::id()
            );
        }

        #[test]
        fn pipe_dacl_names_the_actual_process_user_sid() {
            let sid = super::current_process_user_sid().expect("current user SID");
            let sddl = super::current_user_only_sddl().expect("pipe SDDL");

            assert!(sid.starts_with("S-1-"));
            assert_eq!(sddl, format!("D:P(A;;GA;;;{sid})"));
            assert!(!sddl.contains(";;;OW"));
        }
    }
}
