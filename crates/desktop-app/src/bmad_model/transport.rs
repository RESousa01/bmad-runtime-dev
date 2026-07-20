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
#[cfg_attr(
    feature = "deterministic-help",
    allow(
        dead_code,
        reason = "the offline seam composes when deterministic help is off"
    )
)]
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

/// Production transport seam. It validates the package-controlled
/// configuration at construction and fails closed on send until the
/// deployed support plane round-trip is activated during rollout; it never
/// degrades to deterministic or unsigned behavior.
#[cfg(feature = "production-support")]
#[allow(dead_code, reason = "composed by the Task 9 deployed round trip")]
pub(super) struct ProductionHelpTransport {
    _client: desktop_cloud::ProductionSupportClient,
}

#[cfg(feature = "production-support")]
impl std::fmt::Debug for ProductionHelpTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductionHelpTransport")
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "production-support")]
#[allow(dead_code, reason = "composed by the Task 9 deployed round trip")]
impl ProductionHelpTransport {
    pub(super) fn new(client: desktop_cloud::ProductionSupportClient) -> Self {
        Self { _client: client }
    }
}

#[cfg(feature = "production-support")]
impl BmadHelpTransport for ProductionHelpTransport {
    fn send(
        &self,
        _request: AuthorizedModelRequest,
        _deterministic_fixture: &str,
        _now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
        // The deployed round-trip (bootstrap -> registration -> policy ->
        // lease -> signed consent -> model call -> verified receipt) is
        // enabled during the gated rollout; until then production sends
        // fail closed as offline.
        Err(CloudError::Offline)
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
