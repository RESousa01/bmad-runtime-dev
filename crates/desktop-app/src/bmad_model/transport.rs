use desktop_cloud::{
    AuthorizedModelRequest, CloudError, DispatchedModelRequest, OfflineModelTransport,
    RawModelOutput,
};
use desktop_runtime::UnixMillis;

pub(super) trait BmadHelpTransport: Send + Sync {
    fn send(
        &self,
        request: AuthorizedModelRequest,
        deterministic_fixture: &str,
        now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError>;
}

#[derive(Debug, Default)]
pub(super) struct OfflineHelpTransport;

impl BmadHelpTransport for OfflineHelpTransport {
    fn send(
        &self,
        request: AuthorizedModelRequest,
        _deterministic_fixture: &str,
        _now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
        OfflineModelTransport.send(request)
    }
}

#[cfg(feature = "deterministic-help")]
#[derive(Debug, Default)]
pub(super) struct DeterministicHelpTransport;

#[cfg(feature = "deterministic-help")]
impl BmadHelpTransport for DeterministicHelpTransport {
    fn send(
        &self,
        request: AuthorizedModelRequest,
        deterministic_fixture: &str,
        now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
        desktop_cloud::DeterministicModelTransport.send_fixture(
            request,
            deterministic_fixture.to_owned(),
            now,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{BmadHelpTransport, OfflineHelpTransport};

    #[test]
    fn offline_transport_is_the_default_fail_closed_seam() {
        fn assert_transport<T: BmadHelpTransport>() {}
        assert_transport::<OfflineHelpTransport>();
    }
}
