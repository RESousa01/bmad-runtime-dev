//! Normalized model stream decoding (FND-043).
//!
//! Three properties drive the shape here.
//!
//! * **Rust never parses raw Foundry SSE.** The support plane normalizes first
//!   and answers the one reviewed route
//!   ([`SupportStageRoute::ModelStream`](crate::SupportStageRoute::ModelStream))
//!   with a server-sent event stream whose every `data:` payload is a generated
//!   [`canonical::ModelStreamEvent`]. This module splits those frames and
//!   decodes those values, and knows nothing about any provider's wire format.
//! * **Everything is bounded before it is decoded.** One event, the whole
//!   aggregate, the event count, and the accumulated output text all have
//!   ceilings, and the ceiling is checked while bytes accumulate rather than
//!   after a frame is complete — otherwise a stream that never emits a
//!   terminator is an unbounded allocation.
//! * **A terminal state is terminal.** Sequence numbers must strictly increase
//!   from one, exactly one terminal event may arrive, and every event after it
//!   — or after a local cancellation — is refused rather than forwarded. This
//!   is what makes "cancel the HTTP stream and reject late events" a property of
//!   the decoder and not of whoever remembers to stop reading.

use core::fmt;

use desktop_runtime::generated_contracts as canonical;
use desktop_runtime::{sha256_bytes, ContractId, Sha256Digest};
use serde::Deserialize;
use thiserror::Error;

use crate::transport::HttpChunkSource;

/// Ceiling for one server-sent event frame, payload included.
///
/// A normalized event is a small closed document; the largest is a completed
/// event carrying a receipt summary. This sits far above that and far below the
/// aggregate ceiling, so one malformed frame cannot exhaust the whole budget.
pub const MAX_STREAM_EVENT_BYTES: usize = 64 * 1024;

/// Ceiling for every byte one run may deliver, framing included.
pub const MAX_STREAM_AGGREGATE_BYTES: usize = 4 * 1024 * 1024;

/// Ceiling for the assembled answer text.
pub const MAX_STREAM_OUTPUT_BYTES: usize = 1024 * 1024;

/// Ceiling for the number of events one run may deliver.
pub const MAX_STREAM_EVENTS: u64 = 100_000;

/// Ceiling for one tool call's accumulated arguments.
pub const MAX_TOOL_ARGUMENTS_BYTES: usize = 64 * 1024;

/// The most tool calls one run may open.
pub const MAX_TOOL_CALLS: usize = 32;

/// Why a run was torn down locally.
///
/// Each of these is a fact the host knows and the support plane does not, which
/// is why cancellation is a local decision that also closes the HTTP stream
/// rather than a request the server has to honor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamCancellation {
    /// The user pressed stop.
    UserStopped,
    /// The cloud session was invalidated underneath the run.
    SessionInvalidated,
    /// The entitlement lease expired mid-run.
    EntitlementExpired,
    /// The device registration was revoked.
    DeviceRevoked,
    /// The task the run belongs to was closed.
    TaskClosed,
}

/// What the trusted app layer is allowed to see.
///
/// Deliberately narrower than the wire event: no receipt body, no proof, no
/// resolved deployment, and no Azure identity of any kind. A refusal or failure
/// carries the closed code and the support plane's own short safe detail, never
/// a provider message.
#[derive(Clone, Debug)]
pub enum SafeStreamEvent {
    /// The provider accepted the request and named the operating point that was
    /// actually resolved.
    Started {
        run_id: ContractId,
        resolved_profile_id: String,
        resolved_variant_id: String,
    },
    /// More answer text.
    OutputTextDelta { output_index: u8, text: String },
    /// A tool call opened.
    ToolCallStarted {
        tool_call_id: String,
        tool_name: String,
    },
    /// A tool call closed, with the hash of its complete arguments.
    ToolCallCompleted {
        tool_call_id: String,
        arguments_hash: Sha256Digest,
        arguments_byte_count: i64,
    },
    /// Reported usage. Carried through unchanged because it is already the
    /// sanitized projection the receipt uses.
    Usage(Box<canonical::ModelAccessReceiptV2ModelUsageV2>),
    /// The run reached a terminal state.
    Terminal(Box<StreamTerminal>),
}

/// The one terminal outcome of a run.
#[derive(Clone, Debug)]
pub enum StreamTerminal {
    Completed(Box<canonical::ModelStreamEventStreamReceiptSummary>),
    Refused {
        code: canonical::ModelStreamEventStreamRefusalCode,
        detail: Option<String>,
    },
    Failed {
        code: canonical::ModelStreamEventStreamFailureCode,
        retryable: bool,
        detail: Option<String>,
    },
    Cancelled {
        source: canonical::ModelStreamEventStreamCancellationSource,
    },
}

/// The stable, secret-free error surface of stream decoding.
///
/// It is deliberately its own taxonomy rather than a [`crate::CloudError`]. Two
/// reasons. `CallUncertain` has no `CloudError` counterpart and must not be
/// collapsed into a generic transport failure, because the whole point of the
/// value is that it means *do not retry*. And `CloudError` is matched
/// exhaustively by a coordinator outside this lane, so growing it for a
/// stream-local condition would be a cross-lane change for a local fact.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ModelStreamError {
    #[error("the stream event exceeds its byte limit")]
    EventTooLarge,
    #[error("the stream exceeds its aggregate byte limit")]
    StreamTooLarge,
    #[error("the stream exceeds its event count limit")]
    TooManyEvents,
    #[error("the stream frame is not a normalized model stream event")]
    MalformedEvent,
    #[error("the stream event does not belong to this run")]
    WrongRun,
    #[error("the stream sequence is not strictly increasing")]
    SequenceOutOfOrder,
    #[error("an event arrived after the run reached a terminal state")]
    LateEvent,
    #[error("the stream ended without a terminal event")]
    Truncated,
    #[error("the accumulated output exceeds its byte limit")]
    OutputTooLarge,
    #[error("the tool call stream is invalid")]
    ToolCallInvalid,
    #[error("the terminal receipt does not bind this run")]
    ReceiptBindingMismatch,
    #[error("the terminal receipt proof is invalid")]
    ReceiptProofInvalid,
    #[error("the support plane transport failed")]
    TransportFailed,
    /// The provider call may or may not have been accepted. **Never replay it.**
    ///
    /// The support plane spends consent before it opens the provider stream and
    /// nothing restores it, so a second attempt under the same key would be a
    /// second provider call funded by an approval that was already spent. The
    /// server answers a retried claim with exactly this, and the desktop must
    /// reach the same conclusion on its own when a stream dies without a
    /// terminal event after the provider had already started answering.
    #[error("the model call outcome is uncertain and must not be replayed")]
    CallUncertain,
}

/// The facts one run is bound to. Every event is checked against them.
#[derive(Clone, Debug)]
pub struct ModelStreamBinding {
    pub request_id: ContractId,
    pub request_hash: Sha256Digest,
    pub model_profile_id: String,
    pub requested_variant_id: Option<String>,
    pub catalog_version: String,
    pub catalog_hash: Sha256Digest,
    pub model_profile_hash: Sha256Digest,
    pub model_capability_hash: Sha256Digest,
}

/// What a decoded terminal run leaves behind.
#[derive(Clone, Debug)]
pub struct CompletedModelStream {
    run_id: ContractId,
    resolved_profile_id: String,
    resolved_variant_id: String,
    output_text: String,
    output_hash: Sha256Digest,
    event_count: u64,
    terminal: StreamTerminal,
}

impl CompletedModelStream {
    #[must_use]
    pub fn run_id(&self) -> &ContractId {
        &self.run_id
    }

    #[must_use]
    pub fn resolved_profile_id(&self) -> &str {
        &self.resolved_profile_id
    }

    #[must_use]
    pub fn resolved_variant_id(&self) -> &str {
        &self.resolved_variant_id
    }

    /// The assembled answer. Trusted-host only.
    #[must_use]
    pub fn output_text(&self) -> &str {
        &self.output_text
    }

    #[must_use]
    pub const fn output_hash(&self) -> Sha256Digest {
        self.output_hash
    }

    #[must_use]
    pub const fn event_count(&self) -> u64 {
        self.event_count
    }

    #[must_use]
    pub const fn terminal(&self) -> &StreamTerminal {
        &self.terminal
    }
}

impl fmt::Debug for ModelStreamDecoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModelStreamDecoder")
            .field("request_id", &self.binding.request_id)
            .field("run_id", &self.run_id)
            .field("last_sequence", &self.last_sequence)
            .field("event_count", &self.event_count)
            .field("aggregate_bytes", &self.aggregate_bytes)
            .field("output_text", &"[REDACTED]")
            .field("terminated", &self.terminal.is_some())
            .finish_non_exhaustive()
    }
}

/// Verifies the signed proof over a terminal receipt hash.
///
/// A trait so the production support client's key ring stays the only thing that
/// knows about issuers, audiences, and pinned keys; this module only knows that
/// a hash must be proved before a run counts as complete.
pub trait StreamReceiptProofVerifier: Send + Sync {
    /// Verifies the proof for `receipt_id` over `receipt_hash`.
    ///
    /// # Errors
    ///
    /// Returns [`ModelStreamError::ReceiptProofInvalid`] when the proof, its
    /// issuer, its audience, or its key fails policy.
    fn verify_receipt_proof(
        &self,
        receipt_id: &str,
        receipt_hash: &str,
    ) -> Result<(), ModelStreamError>;
}

#[derive(Deserialize)]
struct EventTypeProbe {
    #[serde(rename = "type")]
    type_: String,
}

/// One run's incremental decoder.
///
/// It is fed raw bytes as they arrive and yields only safe events. It owns every
/// bound, the sequence check, the terminal-state check, and the accumulation the
/// receipt verification below needs.
pub struct ModelStreamDecoder {
    binding: ModelStreamBinding,
    buffer: Vec<u8>,
    run_id: Option<ContractId>,
    resolved_profile_id: Option<String>,
    resolved_variant_id: Option<String>,
    last_sequence: u64,
    event_count: u64,
    aggregate_bytes: usize,
    output_text: String,
    open_tool_calls: Vec<String>,
    terminal: Option<StreamTerminal>,
    cancelled: Option<StreamCancellation>,
    /// True once the provider has demonstrably started answering. A stream that
    /// dies after this point is uncertain rather than merely failed.
    provider_started: bool,
}

impl ModelStreamDecoder {
    #[must_use]
    pub fn new(binding: ModelStreamBinding) -> Self {
        Self {
            binding,
            buffer: Vec::new(),
            run_id: None,
            resolved_profile_id: None,
            resolved_variant_id: None,
            last_sequence: 0,
            event_count: 0,
            aggregate_bytes: 0,
            output_text: String::new(),
            open_tool_calls: Vec::new(),
            terminal: None,
            cancelled: None,
            provider_started: false,
        }
    }

    /// Whether this run has reached a terminal state or been cancelled, and so
    /// the HTTP stream should be closed.
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.terminal.is_some() || self.cancelled.is_some()
    }

    /// Tears the run down locally.
    ///
    /// Everything that arrives afterwards is refused as a late event, so a
    /// caller that cancels but keeps reading — because the HTTP close has not
    /// landed yet — cannot forward a delta the user already stopped.
    pub fn cancel(&mut self, source: StreamCancellation) {
        if self.cancelled.is_none() && self.terminal.is_none() {
            self.cancelled = Some(source);
        }
    }

    #[must_use]
    pub const fn cancellation(&self) -> Option<StreamCancellation> {
        self.cancelled
    }

    /// Accepts the next arriving bytes and returns every safe event they
    /// completed, in order.
    ///
    /// # Errors
    ///
    /// Returns [`ModelStreamError`] for a bound violation, a malformed or
    /// misattributed event, an out-of-order sequence, or an event that arrives
    /// after the run finished.
    pub fn accept(&mut self, chunk: &[u8]) -> Result<Vec<SafeStreamEvent>, ModelStreamError> {
        if self.is_finished() {
            return Err(ModelStreamError::LateEvent);
        }
        self.aggregate_bytes = self
            .aggregate_bytes
            .checked_add(chunk.len())
            .ok_or(ModelStreamError::StreamTooLarge)?;
        if self.aggregate_bytes > MAX_STREAM_AGGREGATE_BYTES {
            return Err(ModelStreamError::StreamTooLarge);
        }
        self.buffer.extend_from_slice(chunk);
        // Checked while bytes accumulate, not after a frame completes: a stream
        // that never emits a blank-line terminator is otherwise an unbounded
        // allocation with every individual chunk inside its own limit.
        if self.buffer.len() > MAX_STREAM_EVENT_BYTES {
            return Err(ModelStreamError::EventTooLarge);
        }

        let mut decoded = Vec::new();
        while let Some(frame_end) = find_frame_end(&self.buffer) {
            let frame: Vec<u8> = self.buffer.drain(..frame_end.total).collect();
            let payload = frame
                .get(..frame_end.body)
                .ok_or(ModelStreamError::MalformedEvent)?;
            if let Some(event) = self.decode_frame(payload)? {
                let terminal = matches!(event, SafeStreamEvent::Terminal(_));
                decoded.push(event);
                if terminal {
                    break;
                }
            }
        }
        Ok(decoded)
    }

    /// Concludes the run once the body is complete.
    ///
    /// # Errors
    ///
    /// Returns [`ModelStreamError::CallUncertain`] when the provider had started
    /// answering and the stream ended without a terminal event — the one case
    /// that must never be retried — and [`ModelStreamError::Truncated`] when it
    /// ended before the provider started, which is safe to treat as a call that
    /// never happened.
    pub fn finish(self) -> Result<CompletedModelStream, ModelStreamError> {
        let Some(terminal) = self.terminal else {
            return Err(if self.provider_started {
                ModelStreamError::CallUncertain
            } else {
                ModelStreamError::Truncated
            });
        };
        let run_id = self.run_id.ok_or(ModelStreamError::MalformedEvent)?;
        let output_hash = sha256_bytes(self.output_text.as_bytes());
        Ok(CompletedModelStream {
            run_id,
            resolved_profile_id: self.resolved_profile_id.unwrap_or_default(),
            resolved_variant_id: self.resolved_variant_id.unwrap_or_default(),
            output_text: self.output_text,
            output_hash,
            event_count: self.event_count,
            terminal,
        })
    }

    fn decode_frame(
        &mut self,
        payload: &[u8],
    ) -> Result<Option<SafeStreamEvent>, ModelStreamError> {
        let Some(data) = event_data(payload) else {
            // A comment, a retry hint, or a keep-alive. Ignored rather than
            // refused: it carries no event and consumed a counted byte budget.
            return Ok(None);
        };
        if data.len() > MAX_STREAM_EVENT_BYTES {
            return Err(ModelStreamError::EventTooLarge);
        }
        self.event_count = self
            .event_count
            .checked_add(1)
            .ok_or(ModelStreamError::TooManyEvents)?;
        if self.event_count > MAX_STREAM_EVENTS {
            return Err(ModelStreamError::TooManyEvents);
        }
        // The generated union is untagged, so the discriminating `type` member
        // is not enforced by deserialization. It is read here and compared
        // against the variant that actually matched, which is what stops a
        // payload from being admitted as an event it does not claim to be.
        let probe: EventTypeProbe =
            serde_json::from_slice(data).map_err(|_| ModelStreamError::MalformedEvent)?;
        let event: canonical::ModelStreamEvent =
            serde_json::from_slice(data).map_err(|_| ModelStreamError::MalformedEvent)?;
        if variant_type(&event) != probe.type_ {
            return Err(ModelStreamError::MalformedEvent);
        }
        self.admit(event).map(Some)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one linear admission row per normalized stream event variant"
    )]
    fn admit(
        &mut self,
        event: canonical::ModelStreamEvent,
    ) -> Result<SafeStreamEvent, ModelStreamError> {
        if self.is_finished() {
            return Err(ModelStreamError::LateEvent);
        }
        let (request_id, run_id, sequence) = identity(&event);
        if request_id != self.binding.request_id.as_str() {
            return Err(ModelStreamError::WrongRun);
        }
        match &self.run_id {
            Some(known) if known.as_str() != run_id => return Err(ModelStreamError::WrongRun),
            Some(_) => {}
            None => {
                self.run_id =
                    Some(ContractId::new(run_id).map_err(|_| ModelStreamError::MalformedEvent)?);
            }
        }
        if sequence <= self.last_sequence {
            return Err(ModelStreamError::SequenceOutOfOrder);
        }
        self.last_sequence = sequence;

        match event {
            canonical::ModelStreamEvent::ResponseStartedEvent(started) => {
                if self.provider_started {
                    return Err(ModelStreamError::MalformedEvent);
                }
                // The support plane may resolve the profile default when the
                // request named no variant, but it may never resolve a
                // *different profile* than the one that was consented to.
                if started.resolved_profile_id.as_str() != self.binding.model_profile_id {
                    return Err(ModelStreamError::WrongRun);
                }
                if let Some(requested) = &self.binding.requested_variant_id {
                    if started.resolved_variant_id.as_str() != requested {
                        return Err(ModelStreamError::WrongRun);
                    }
                }
                self.provider_started = true;
                self.resolved_profile_id = Some(started.resolved_profile_id.to_string());
                self.resolved_variant_id = Some(started.resolved_variant_id.to_string());
                Ok(SafeStreamEvent::Started {
                    run_id: self.run_id.clone().ok_or(ModelStreamError::MalformedEvent)?,
                    resolved_profile_id: started.resolved_profile_id.to_string(),
                    resolved_variant_id: started.resolved_variant_id.to_string(),
                })
            }
            canonical::ModelStreamEvent::OutputTextDeltaEvent(delta) => {
                if !self.provider_started {
                    return Err(ModelStreamError::SequenceOutOfOrder);
                }
                let text = delta.text_delta.to_string();
                let next = self
                    .output_text
                    .len()
                    .checked_add(text.len())
                    .ok_or(ModelStreamError::OutputTooLarge)?;
                if next > MAX_STREAM_OUTPUT_BYTES {
                    return Err(ModelStreamError::OutputTooLarge);
                }
                self.output_text.push_str(&text);
                Ok(SafeStreamEvent::OutputTextDelta {
                    output_index: delta.output_index,
                    text,
                })
            }
            canonical::ModelStreamEvent::ToolCallStartedEvent(call) => {
                if !self.provider_started || self.open_tool_calls.len() >= MAX_TOOL_CALLS {
                    return Err(ModelStreamError::ToolCallInvalid);
                }
                let id = call.tool_call_id.to_string();
                if self.open_tool_calls.contains(&id) {
                    return Err(ModelStreamError::ToolCallInvalid);
                }
                self.open_tool_calls.push(id.clone());
                Ok(SafeStreamEvent::ToolCallStarted {
                    tool_call_id: id,
                    tool_name: call.tool_name.to_string(),
                })
            }
            canonical::ModelStreamEvent::ToolCallArgumentsDeltaEvent(delta) => {
                let id = delta.tool_call_id.to_string();
                if !self.open_tool_calls.contains(&id)
                    || delta.arguments_delta.len() > MAX_TOOL_ARGUMENTS_BYTES
                {
                    return Err(ModelStreamError::ToolCallInvalid);
                }
                // Arguments are accumulated by the support plane, which is what
                // hashes them on completion. Nothing is forwarded here: a
                // partial argument document is not something a trusted consumer
                // can act on, and forwarding it would invite one to try.
                Ok(SafeStreamEvent::ToolCallStarted {
                    tool_call_id: id,
                    tool_name: String::new(),
                })
            }
            canonical::ModelStreamEvent::ToolCallCompletedEvent(call) => {
                let id = call.tool_call_id.to_string();
                let Some(index) = self.open_tool_calls.iter().position(|open| open == &id) else {
                    return Err(ModelStreamError::ToolCallInvalid);
                };
                if call.arguments_byte_count < 0
                    || call.arguments_byte_count
                        > i64::try_from(MAX_TOOL_ARGUMENTS_BYTES).unwrap_or(i64::MAX)
                {
                    return Err(ModelStreamError::ToolCallInvalid);
                }
                self.open_tool_calls.remove(index);
                Ok(SafeStreamEvent::ToolCallCompleted {
                    tool_call_id: id,
                    arguments_hash: Sha256Digest::parse(&call.arguments_hash.to_string())
                        .map_err(|_| ModelStreamError::MalformedEvent)?,
                    arguments_byte_count: call.arguments_byte_count,
                })
            }
            canonical::ModelStreamEvent::UsageEvent(usage) => {
                Ok(SafeStreamEvent::Usage(Box::new(usage.usage)))
            }
            canonical::ModelStreamEvent::CompletedEvent(completed) => {
                if !self.provider_started || !self.open_tool_calls.is_empty() {
                    return Err(ModelStreamError::MalformedEvent);
                }
                let terminal = StreamTerminal::Completed(Box::new(completed.receipt_summary));
                self.terminal = Some(terminal.clone());
                Ok(SafeStreamEvent::Terminal(Box::new(terminal)))
            }
            canonical::ModelStreamEvent::RefusedEvent(refused) => {
                let terminal = StreamTerminal::Refused {
                    code: refused.refusal_code,
                    detail: refused.refusal_detail.map(|detail| detail.to_string()),
                };
                self.terminal = Some(terminal.clone());
                Ok(SafeStreamEvent::Terminal(Box::new(terminal)))
            }
            canonical::ModelStreamEvent::FailedEvent(failed) => {
                let terminal = StreamTerminal::Failed {
                    code: failed.failure_code,
                    retryable: failed.retryable,
                    detail: failed.failure_detail.map(|detail| detail.to_string()),
                };
                self.terminal = Some(terminal.clone());
                Ok(SafeStreamEvent::Terminal(Box::new(terminal)))
            }
            canonical::ModelStreamEvent::CancelledEvent(cancelled) => {
                let terminal = StreamTerminal::Cancelled {
                    source: cancelled.cancellation_source,
                };
                self.terminal = Some(terminal.clone());
                Ok(SafeStreamEvent::Terminal(Box::new(terminal)))
            }
        }
    }
}

/// Verifies that a completed run's terminal receipt binds this exact request.
///
/// # What is checked
///
/// The resolved profile and variant must be the ones the run was bound to, the
/// terminal status must be a success, the reported output size must match the
/// bytes that were actually accumulated, and the receipt hash must carry a valid
/// signed proof. A run that fails any of these is not marked complete, which is
/// what stops a receipt for some other call from closing this one.
///
/// # Errors
///
/// Returns [`ModelStreamError::ReceiptBindingMismatch`] for a receipt that does
/// not bind this run and [`ModelStreamError::ReceiptProofInvalid`] when the
/// proof fails policy.
pub fn verify_terminal_receipt<V>(
    completed: &CompletedModelStream,
    binding: &ModelStreamBinding,
    verifier: &V,
) -> Result<(), ModelStreamError>
where
    V: StreamReceiptProofVerifier + ?Sized,
{
    let StreamTerminal::Completed(summary) = completed.terminal() else {
        return Err(ModelStreamError::ReceiptBindingMismatch);
    };
    if summary.resolved_profile_id.as_str() != binding.model_profile_id
        || summary.resolved_profile_id.as_str() != completed.resolved_profile_id()
        || summary.resolved_variant_id.as_str() != completed.resolved_variant_id()
    {
        return Err(ModelStreamError::ReceiptBindingMismatch);
    }
    if let Some(requested) = &binding.requested_variant_id {
        if summary.resolved_variant_id.as_str() != requested {
            return Err(ModelStreamError::ReceiptBindingMismatch);
        }
    }
    if !matches!(
        summary.terminal_status,
        canonical::ModelAccessReceiptV2ModelTerminalStatus::Succeeded
    ) {
        return Err(ModelStreamError::ReceiptBindingMismatch);
    }
    let reported = u64::try_from(summary.output_bytes.0)
        .map_err(|_| ModelStreamError::ReceiptBindingMismatch)?;
    let accumulated = u64::try_from(completed.output_text().len())
        .map_err(|_| ModelStreamError::ReceiptBindingMismatch)?;
    if reported != accumulated {
        return Err(ModelStreamError::ReceiptBindingMismatch);
    }
    verifier.verify_receipt_proof(
        summary.receipt_id.as_str(),
        &summary.receipt_hash.to_string(),
    )
}

/// Reads one whole stream from a chunk source and concludes it.
///
/// `forward` is called with every safe event as it is decoded and answers
/// whether the run should keep going; answering with a cancellation closes the
/// stream and refuses everything that arrives afterwards. That is how stop,
/// session invalidation, entitlement expiry, device revocation, and task close
/// all reach the transport: the source is dropped, which cancels the HTTP
/// request.
///
/// # Errors
///
/// Returns [`ModelStreamError`] for any bound, ordering, attribution, or
/// terminal-state violation, and [`ModelStreamError::CallUncertain`] when the
/// body ends after the provider started answering but before a terminal event.
pub async fn read_model_stream<F>(
    mut source: Box<dyn HttpChunkSource>,
    binding: ModelStreamBinding,
    mut forward: F,
) -> Result<CompletedModelStream, ModelStreamError>
where
    F: FnMut(&SafeStreamEvent) -> Option<StreamCancellation>,
{
    let mut decoder = ModelStreamDecoder::new(binding);
    loop {
        let chunk = source
            .next_chunk()
            .await
            .map_err(|_| ModelStreamError::TransportFailed)?;
        let Some(chunk) = chunk else { break };
        for event in decoder.accept(&chunk)? {
            if let Some(cancellation) = forward(&event) {
                decoder.cancel(cancellation);
                // The source is dropped with the loop, which is what tears down
                // the HTTP request. Nothing later can be admitted.
                drop(source);
                return Err(ModelStreamError::CallUncertain);
            }
        }
        if decoder.is_finished() {
            break;
        }
    }
    drop(source);
    decoder.finish()
}

struct FrameEnd {
    /// Bytes belonging to the frame body, terminator excluded.
    body: usize,
    /// Bytes to consume from the buffer, terminator included.
    total: usize,
}

/// Finds the end of one server-sent event frame.
///
/// A frame ends at a blank line. Both `\n\n` and `\r\n\r\n` are accepted because
/// an intermediary may rewrite line endings; nothing else is.
fn find_frame_end(buffer: &[u8]) -> Option<FrameEnd> {
    for index in 0..buffer.len() {
        if buffer[index..].starts_with(b"\n\n") {
            return Some(FrameEnd {
                body: index,
                total: index + 2,
            });
        }
        if buffer[index..].starts_with(b"\r\n\r\n") {
            return Some(FrameEnd {
                body: index,
                total: index + 4,
            });
        }
    }
    None
}

/// Extracts the `data:` payload of one frame.
///
/// A frame with no `data:` line carries no event. Multiple `data:` lines are
/// refused rather than joined: the support plane writes exactly one, so more
/// than one means the frame was rewritten between the two ends.
fn event_data(frame: &[u8]) -> Option<&[u8]> {
    let mut found: Option<&[u8]> = None;
    for line in frame.split(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let Some(rest) = line.strip_prefix(b"data:") else {
            continue;
        };
        if found.is_some() {
            return None;
        }
        found = Some(rest.strip_prefix(b" ").unwrap_or(rest));
    }
    found
}

const fn variant_type(event: &canonical::ModelStreamEvent) -> &'static str {
    match event {
        canonical::ModelStreamEvent::ResponseStartedEvent(_) => "response_started",
        canonical::ModelStreamEvent::OutputTextDeltaEvent(_) => "output_text_delta",
        canonical::ModelStreamEvent::ToolCallStartedEvent(_) => "tool_call_started",
        canonical::ModelStreamEvent::ToolCallArgumentsDeltaEvent(_) => "tool_call_arguments_delta",
        canonical::ModelStreamEvent::ToolCallCompletedEvent(_) => "tool_call_completed",
        canonical::ModelStreamEvent::UsageEvent(_) => "usage",
        canonical::ModelStreamEvent::CompletedEvent(_) => "completed",
        canonical::ModelStreamEvent::RefusedEvent(_) => "refused",
        canonical::ModelStreamEvent::FailedEvent(_) => "failed",
        canonical::ModelStreamEvent::CancelledEvent(_) => "cancelled",
    }
}

fn identity(event: &canonical::ModelStreamEvent) -> (&str, &str, u64) {
    match event {
        canonical::ModelStreamEvent::ResponseStartedEvent(value) => (
            &value.request_id,
            &value.run_id,
            value.sequence.0.get(),
        ),
        canonical::ModelStreamEvent::OutputTextDeltaEvent(value) => (
            &value.request_id,
            &value.run_id,
            value.sequence.0.get(),
        ),
        canonical::ModelStreamEvent::ToolCallStartedEvent(value) => (
            &value.request_id,
            &value.run_id,
            value.sequence.0.get(),
        ),
        canonical::ModelStreamEvent::ToolCallArgumentsDeltaEvent(value) => (
            &value.request_id,
            &value.run_id,
            value.sequence.0.get(),
        ),
        canonical::ModelStreamEvent::ToolCallCompletedEvent(value) => (
            &value.request_id,
            &value.run_id,
            value.sequence.0.get(),
        ),
        canonical::ModelStreamEvent::UsageEvent(value) => (
            &value.request_id,
            &value.run_id,
            value.sequence.0.get(),
        ),
        canonical::ModelStreamEvent::CompletedEvent(value) => (
            &value.request_id,
            &value.run_id,
            value.sequence.0.get(),
        ),
        canonical::ModelStreamEvent::RefusedEvent(value) => (
            &value.request_id,
            &value.run_id,
            value.sequence.0.get(),
        ),
        canonical::ModelStreamEvent::FailedEvent(value) => (
            &value.request_id,
            &value.run_id,
            value.sequence.0.get(),
        ),
        canonical::ModelStreamEvent::CancelledEvent(value) => (
            &value.request_id,
            &value.run_id,
            value.sequence.0.get(),
        ),
    }
}
