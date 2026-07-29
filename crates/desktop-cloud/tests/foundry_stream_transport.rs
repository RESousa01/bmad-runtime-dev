//! FND-043: the Managed Responses streaming transport.
//!
//! Everything asserted here is a property of the decoder rather than of a
//! caller who remembers to stop reading: bounds are enforced while bytes
//! accumulate, sequences must strictly increase, exactly one terminal event may
//! arrive, and a stream that dies after the provider started answering is
//! reported as uncertain rather than as something to retry.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "a fixture that does not hold is a broken test, and it must stop the run"
)]

use std::collections::VecDeque;

use async_trait::async_trait;
use desktop_cloud::{
    read_model_stream, verify_terminal_receipt, CloudError, HttpChunkSource, HttpExecutor,
    HttpResponse, ModelStreamBinding, ModelStreamDecoder, ModelStreamError, OutboundHttpRequest,
    SafeStreamEvent, StreamCancellation, StreamReceiptProofVerifier, StreamTerminal,
    StreamingHttpResponse, SupportApiOrigin, SupportApiTransport, SupportStageRoute,
};
use desktop_runtime::sha256_bytes;
use serde_json::{json, Value};
use zeroize::Zeroizing;

const REQUEST_ID: &str = "request_01J00000000000000000000000";
const RUN_ID: &str = "run_01J00000000000000000000000";
const HASH: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn binding() -> ModelStreamBinding {
    ModelStreamBinding {
        request_id: desktop_runtime::ContractId::new(REQUEST_ID).expect("request id"),
        request_hash: sha256_bytes(b"request"),
        model_profile_id: "balanced-reasoning".to_owned(),
        requested_variant_id: Some("balanced".to_owned()),
        catalog_version: "v7.001122".to_owned(),
        catalog_hash: sha256_bytes(b"catalog"),
        model_profile_hash: sha256_bytes(b"profile"),
        model_capability_hash: sha256_bytes(b"capability"),
    }
}

/// One server-sent event frame, exactly as `ModelStreamEndpoint.RenderFrame`
/// writes it.
fn frame(event: &Value) -> Vec<u8> {
    let name = event["type"].as_str().expect("event type");
    format!("event: {name}\ndata: {event}\n\n").into_bytes()
}

fn started(sequence: u64) -> Value {
    json!({
        "type": "response_started",
        "requestId": REQUEST_ID,
        "runId": RUN_ID,
        "sequence": sequence,
        "startedAt": "2026-07-28T10:00:00.000Z",
        "resolvedProfileId": "balanced-reasoning",
        "resolvedVariantId": "balanced"
    })
}

fn delta(sequence: u64, text: &str) -> Value {
    json!({
        "type": "output_text_delta",
        "requestId": REQUEST_ID,
        "runId": RUN_ID,
        "sequence": sequence,
        "outputIndex": 0,
        "textDelta": text
    })
}

fn completed(sequence: u64, output_bytes: i64) -> Value {
    json!({
        "type": "completed",
        "requestId": REQUEST_ID,
        "runId": RUN_ID,
        "sequence": sequence,
        "receiptSummary": {
            "receiptId": "receipt_01J00000000000000000000000",
            "receiptHash": HASH,
            "resolvedProfileId": "balanced-reasoning",
            "resolvedVariantId": "balanced",
            "terminalStatus": "succeeded",
            "outputBytes": output_bytes,
            "completedAt": "2026-07-28T10:00:02.000Z"
        }
    })
}

struct ScriptedChunks {
    chunks: VecDeque<Vec<u8>>,
}

#[async_trait]
impl HttpChunkSource for ScriptedChunks {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, CloudError> {
        Ok(self.chunks.pop_front())
    }
}

fn source(frames: Vec<Vec<u8>>) -> Box<dyn HttpChunkSource> {
    Box::new(ScriptedChunks {
        chunks: frames.into_iter().collect(),
    })
}

struct AcceptingProof;

impl StreamReceiptProofVerifier for AcceptingProof {
    fn verify_receipt_proof(
        &self,
        _receipt_id: &str,
        _receipt_hash: &str,
    ) -> Result<(), ModelStreamError> {
        Ok(())
    }
}

struct RefusingProof;

impl StreamReceiptProofVerifier for RefusingProof {
    fn verify_receipt_proof(
        &self,
        _receipt_id: &str,
        _receipt_hash: &str,
    ) -> Result<(), ModelStreamError> {
        Err(ModelStreamError::ReceiptProofInvalid)
    }
}

#[test]
fn the_stream_route_is_the_one_separately_reviewed_path() {
    assert_eq!(
        SupportStageRoute::ModelStream.path(),
        "/desktop/v1/model-access/streams",
    );
    assert_eq!(
        SupportStageRoute::ModelStream.method(),
        desktop_cloud::HttpMethod::Post,
    );
}

#[tokio::test]
async fn a_complete_stream_decodes_incrementally_and_binds_its_receipt() {
    let frames = vec![
        frame(&started(1)),
        frame(&delta(2, "Hello, ")),
        frame(&delta(3, "world.")),
        frame(&completed(4, 13)),
    ];
    let mut seen = Vec::new();
    let completed_run = read_model_stream(source(frames), binding(), |event| {
        seen.push(match event {
            SafeStreamEvent::Started { .. } => "started",
            SafeStreamEvent::OutputTextDelta { .. } => "delta",
            SafeStreamEvent::Terminal(_) => "terminal",
            _ => "other",
        });
        None
    })
    .await
    .expect("a complete stream");

    assert_eq!(seen, ["started", "delta", "delta", "terminal"]);
    assert_eq!(completed_run.output_text(), "Hello, world.");
    assert_eq!(completed_run.event_count(), 4);
    assert!(matches!(
        completed_run.terminal(),
        StreamTerminal::Completed(_)
    ));
    verify_terminal_receipt(&completed_run, &binding(), &AcceptingProof)
        .expect("a bound receipt with a valid proof");
}

/// A receipt that does not bind this run, or whose proof fails policy, must not
/// mark the run complete.
#[tokio::test]
async fn a_receipt_that_does_not_bind_the_run_is_refused() {
    let run = read_model_stream(
        source(vec![
            frame(&started(1)),
            frame(&delta(2, "hi")),
            // The reported size disagrees with the bytes that actually arrived.
            frame(&completed(3, 99)),
        ]),
        binding(),
        |_| None,
    )
    .await
    .expect("a terminated stream");
    assert_eq!(
        verify_terminal_receipt(&run, &binding(), &AcceptingProof),
        Err(ModelStreamError::ReceiptBindingMismatch),
    );

    let honest = read_model_stream(
        source(vec![
            frame(&started(1)),
            frame(&delta(2, "hi")),
            frame(&completed(3, 2)),
        ]),
        binding(),
        |_| None,
    )
    .await
    .expect("a terminated stream");
    assert_eq!(
        verify_terminal_receipt(&honest, &binding(), &RefusingProof),
        Err(ModelStreamError::ReceiptProofInvalid),
    );
}

#[test]
fn events_must_belong_to_this_run_and_arrive_in_order() {
    let mut decoder = ModelStreamDecoder::new(binding());
    decoder.accept(&frame(&started(1))).expect("first event");

    let mut foreign = delta(2, "x");
    foreign["runId"] = json!("run_01J00000000000000000000009");
    assert_eq!(
        ModelStreamDecoder::new(binding())
            .accept(&frame(&started(1)))
            .and_then(|_| {
                let mut other = ModelStreamDecoder::new(binding());
                other.accept(&frame(&started(1)))?;
                other.accept(&frame(&foreign))
            })
            .err(),
        Some(ModelStreamError::WrongRun),
    );

    // A repeated sequence is not a duplicate to tolerate; it is a stream that
    // cannot be ordered.
    let mut replayed = ModelStreamDecoder::new(binding());
    replayed.accept(&frame(&started(1))).expect("first");
    replayed.accept(&frame(&delta(2, "a"))).expect("second");
    assert_eq!(
        replayed.accept(&frame(&delta(2, "b"))).err(),
        Some(ModelStreamError::SequenceOutOfOrder),
    );

    let mut wrong_request = ModelStreamDecoder::new(binding());
    let mut alien = started(1);
    alien["requestId"] = json!("request_01J00000000000000000000009");
    assert_eq!(
        wrong_request.accept(&frame(&alien)).err(),
        Some(ModelStreamError::WrongRun),
    );
}

/// A terminal state is terminal. This is the property that makes "reject late
/// events" true of the decoder rather than of whoever remembers to stop.
#[test]
fn nothing_is_admitted_after_a_terminal_event_or_a_cancellation() {
    let mut decoder = ModelStreamDecoder::new(binding());
    decoder.accept(&frame(&started(1))).expect("first");
    decoder.accept(&frame(&completed(2, 0))).expect("terminal");
    assert!(decoder.is_finished());
    assert_eq!(
        decoder.accept(&frame(&delta(3, "late"))).err(),
        Some(ModelStreamError::LateEvent),
    );

    for source in [
        StreamCancellation::UserStopped,
        StreamCancellation::SessionInvalidated,
        StreamCancellation::EntitlementExpired,
        StreamCancellation::DeviceRevoked,
        StreamCancellation::TaskClosed,
    ] {
        let mut cancelled = ModelStreamDecoder::new(binding());
        cancelled.accept(&frame(&started(1))).expect("first");
        cancelled.cancel(source);
        assert_eq!(cancelled.cancellation(), Some(source));
        assert_eq!(
            cancelled.accept(&frame(&delta(2, "late"))).err(),
            Some(ModelStreamError::LateEvent),
        );
    }
}

/// The reconnection policy, stated as a test: a stream that ends after the
/// provider started answering is uncertain and must never be replayed. A stream
/// that ends before it started never reached the provider and is merely
/// truncated.
#[tokio::test]
async fn an_interrupted_stream_is_uncertain_rather_than_retryable() {
    let interrupted = read_model_stream(
        source(vec![frame(&started(1)), frame(&delta(2, "partial"))]),
        binding(),
        |_| None,
    )
    .await;
    assert_eq!(interrupted.err(), Some(ModelStreamError::CallUncertain));

    let never_started = read_model_stream(source(Vec::new()), binding(), |_| None).await;
    assert_eq!(never_started.err(), Some(ModelStreamError::Truncated));

    // Stopping mid-answer is the same conclusion: consent was already spent, so
    // there is nothing safe to retry.
    let stopped = read_model_stream(
        source(vec![
            frame(&started(1)),
            frame(&delta(2, "partial")),
            frame(&completed(3, 7)),
        ]),
        binding(),
        |event| {
            matches!(event, SafeStreamEvent::OutputTextDelta { .. })
                .then_some(StreamCancellation::UserStopped)
        },
    )
    .await;
    assert_eq!(stopped.err(), Some(ModelStreamError::CallUncertain));
}

#[test]
fn bounds_are_enforced_while_bytes_accumulate() {
    // A frame that never terminates cannot grow past the single-event ceiling,
    // even though every individual chunk is small.
    let mut decoder = ModelStreamDecoder::new(binding());
    let chunk = vec![b'x'; 8 * 1024];
    let mut refused = None;
    for _ in 0..64 {
        if let Err(error) = decoder.accept(&chunk) {
            refused = Some(error);
            break;
        }
    }
    assert_eq!(refused, Some(ModelStreamError::EventTooLarge));

    // A single oversized answer is refused rather than accumulated.
    let mut output = ModelStreamDecoder::new(binding());
    output.accept(&frame(&started(1))).expect("first");
    let huge = "y".repeat(desktop_cloud::MAX_STREAM_OUTPUT_BYTES + 1);
    assert_eq!(
        output.accept(&frame(&delta(2, &huge))).err(),
        // The frame carrying it exceeds the per-event ceiling first, which is
        // the tighter of the two and the one that bounds allocation.
        Some(ModelStreamError::EventTooLarge),
    );
}

/// The generated union is untagged, so the discriminating `type` member is not
/// enforced by deserialization. A payload that claims one type and matches
/// another must not be admitted.
#[test]
fn an_event_that_lies_about_its_type_is_refused() {
    let mut mislabelled = delta(1, "text");
    mislabelled["type"] = json!("usage");
    let mut decoder = ModelStreamDecoder::new(binding());
    assert_eq!(
        decoder.accept(&frame(&mislabelled)).err(),
        Some(ModelStreamError::MalformedEvent),
    );
}

/// The support plane may resolve the profile default when no variant was
/// requested, but it may never answer with a different profile than the one the
/// user consented to.
#[test]
fn a_substituted_profile_or_variant_is_refused() {
    let mut other_profile = started(1);
    other_profile["resolvedProfileId"] = json!("fast-drafting");
    assert_eq!(
        ModelStreamDecoder::new(binding())
            .accept(&frame(&other_profile))
            .err(),
        Some(ModelStreamError::WrongRun),
    );

    let mut other_variant = started(1);
    other_variant["resolvedVariantId"] = json!("thorough");
    assert_eq!(
        ModelStreamDecoder::new(binding())
            .accept(&frame(&other_variant))
            .err(),
        Some(ModelStreamError::WrongRun),
    );

    // With no variant requested, resolving the profile default is allowed.
    let mut open = binding();
    open.requested_variant_id = None;
    assert!(ModelStreamDecoder::new(open)
        .accept(&frame(&other_variant))
        .is_ok());
}

struct NonStreamingExecutor;

#[async_trait]
impl HttpExecutor for NonStreamingExecutor {
    async fn execute(&self, _request: OutboundHttpRequest) -> Result<HttpResponse, CloudError> {
        Err(CloudError::TransportFailed)
    }
}

struct WrongMediaTypeExecutor;

#[async_trait]
impl HttpExecutor for WrongMediaTypeExecutor {
    async fn execute(&self, _request: OutboundHttpRequest) -> Result<HttpResponse, CloudError> {
        Err(CloudError::TransportFailed)
    }

    async fn execute_stream(
        &self,
        _request: OutboundHttpRequest,
    ) -> Result<StreamingHttpResponse, CloudError> {
        Ok(StreamingHttpResponse::new(
            200,
            Some("application/json".to_owned()),
            source(Vec::new()),
        ))
    }
}

/// An executor that has not opted into streaming refuses, and a response that
/// is not an event stream is refused before a byte reaches the decoder — so a
/// buffered problem document can never be fed to it.
#[tokio::test]
async fn only_an_event_stream_response_may_reach_the_decoder() {
    let origin = SupportApiOrigin::new("https://support.example.test").expect("origin");
    let closed = SupportApiTransport::new(origin.clone(), NonStreamingExecutor);
    assert_eq!(
        closed
            .open_model_stream(Zeroizing::new("token".to_owned()), b"{}".to_vec(), "key")
            .await
            .err(),
        Some(CloudError::TransportFailed),
    );

    let wrong = SupportApiTransport::new(origin, WrongMediaTypeExecutor);
    assert_eq!(
        wrong
            .open_model_stream(Zeroizing::new("token".to_owned()), b"{}".to_vec(), "key")
            .await
            .err(),
        Some(CloudError::TransportFailed),
    );
}
