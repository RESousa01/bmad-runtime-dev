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

/// Production transport seam. When the desktop shell composes the full
/// deployed round trip (broker, installation identity, trust rings, HTTPS
/// executor), sends execute the proven bootstrap -> registration ->
/// policy -> lease -> model -> receipt sequence on a dedicated runtime.
/// Without that composition it fails closed as offline; it never degrades
/// to deterministic or unsigned behavior.
#[cfg(feature = "production-support")]
#[allow(
    dead_code,
    reason = "the shell composes the round trip at desktop enablement"
)]
pub(super) struct ProductionHelpTransport {
    runtime: tokio::runtime::Runtime,
    #[cfg(windows)]
    round_trip: Option<
        desktop_cloud::ProductionRoundTrip<
            desktop_cloud::WindowsIdentityBroker,
            desktop_cloud::ReqwestHttpExecutor,
        >,
    >,
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
#[allow(
    dead_code,
    reason = "the shell composes the round trip at desktop enablement"
)]
impl ProductionHelpTransport {
    /// Fail-closed construction for builds whose shell has not composed
    /// the deployed round trip.
    ///
    /// # Errors
    ///
    /// Returns [`desktop_cloud::CloudError::TransportFailed`] when the
    /// dedicated runtime cannot start.
    pub(super) fn fail_closed() -> Result<Self, desktop_cloud::CloudError> {
        Ok(Self {
            runtime: production_runtime()?,
            #[cfg(windows)]
            round_trip: None,
        })
    }

    /// Owns one fully composed deployed round trip.
    ///
    /// # Errors
    ///
    /// Returns [`desktop_cloud::CloudError::TransportFailed`] when the
    /// dedicated runtime cannot start.
    #[cfg(windows)]
    pub(super) fn with_round_trip(
        round_trip: desktop_cloud::ProductionRoundTrip<
            desktop_cloud::WindowsIdentityBroker,
            desktop_cloud::ReqwestHttpExecutor,
        >,
    ) -> Result<Self, desktop_cloud::CloudError> {
        Ok(Self {
            runtime: production_runtime()?,
            round_trip: Some(round_trip),
        })
    }
}

#[cfg(feature = "production-support")]
fn production_runtime() -> Result<tokio::runtime::Runtime, desktop_cloud::CloudError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| desktop_cloud::CloudError::TransportFailed)
}

#[cfg(feature = "production-support")]
impl BmadHelpTransport for ProductionHelpTransport {
    fn send(
        &self,
        request: AuthorizedModelRequest,
        _deterministic_fixture: &str,
        now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
        #[cfg(windows)]
        if let Some(round_trip) = &self.round_trip {
            return self.runtime.block_on(round_trip.send(request, now));
        }
        // Without the shell-composed round trip, production sends fail
        // closed as offline — never deterministic, never unsigned.
        let _ = (request, now);
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
