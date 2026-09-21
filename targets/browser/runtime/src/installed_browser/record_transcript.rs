//! Browser production-kernel realization of bounded record transcript retention.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, ImplementationId, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, HostedValueStore, PortId,
};

const IMPLEMENTATION: &str = "browser/bounded-record-transcript@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_net::record_transcript_semantic_contract(),
        Back {
            capability_id: CapabilityId::from(IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from("browser/bounded-record-transcript@1"),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-net/bounded-record-transcript@1"),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

struct TranscriptOperation {
    transcript: conduit_net::BoundedRecordTranscript,
    maximum_events: usize,
    events: usize,
    closed: [bool; 3],
    framed_type: Vec<u8>,
    terminal_type: Vec<u8>,
}

impl<const PORTS: usize> StepBack<PORTS> for TranscriptOperation {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        for port in [PortId(0), PortId(1), PortId(2)] {
            let Some(value) = io.input(port) else {
                continue;
            };
            let index = usize::from(port.0);
            if self.closed[index]
                || self.events >= self.maximum_events
                || value.byte_len > MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
            {
                return fail(FailureCode::StorageExhausted, 2);
            }
            if !io.output_ready(port) {
                return StepOutcome::Await;
            }
            let Some(canonical) = input_bytes.input(port) else {
                return fail(FailureCode::InvalidInput, 3);
            };
            let result = match index {
                0 | 1 => exact_leaf(canonical, &self.framed_type).and_then(|frame| {
                    self.transcript
                        .record(
                            if index == 0 {
                                conduit_net::RecordTranscriptDirection::Sent
                            } else {
                                conduit_net::RecordTranscriptDirection::Received
                            },
                            frame,
                        )
                        .map(|_| ())
                        .map_err(|_| ())
                }),
                _ => exact_leaf(canonical, &self.terminal_type)
                    .and_then(|wire| {
                        conduit_net::decode_record_transcript_terminal(wire).map_err(|_| ())
                    })
                    .and_then(|terminal| {
                        self.transcript
                            .terminal(terminal)
                            .map(|_| ())
                            .map_err(|_| ())
                    }),
            };
            if result.is_err() {
                return fail(FailureCode::InvalidInput, 3);
            }
            io.consume(port).expect("present transcript event");
            io.send(port, value).expect("ready transcript output");
            self.events += 1;
            return StepOutcome::Progress;
        }
        for port in [PortId(0), PortId(1), PortId(2)] {
            let index = usize::from(port.0);
            if io.input_closed(port) && !self.closed[index] {
                io.consume_closed(port)
                    .expect("observed transcript closure");
                self.closed[index] = true;
                return if self.closed.iter().all(|closed| *closed) {
                    StepOutcome::Complete
                } else {
                    StepOutcome::Progress
                };
            }
        }
        StepOutcome::Await
    }
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    let (items, events, frame_bytes, retained_bytes) = limits(placement)?;
    Ok(BrowserOperation::installed_step(TranscriptOperation {
        transcript: conduit_net::BoundedRecordTranscript::new(
            items,
            frame_bytes,
            retained_bytes,
            0,
        )
        .map_err(|error| format!("prepare browser transcript: {error:?}"))?,
        maximum_events: events,
        events: 0,
        closed: [false; 3],
        framed_type: conduit_net::framed_typed_record_type()
            .canonical_bytes()
            .map_err(|error| format!("encode browser framed type: {error:?}"))?,
        terminal_type: conduit_net::terminal_event_type()
            .canonical_bytes()
            .map_err(|error| format!("encode browser terminal type: {error:?}"))?,
    }))
}

fn limits(placement: &PlannedGear) -> Result<(usize, usize, usize, usize), String> {
    let [items, events, frame_bytes, retained_bytes] = placement.configuration.as_slice() else {
        return Err("browser transcript requires four exact bounds".into());
    };
    if items.key != "maximum-items"
        || events.key != "maximum-events"
        || frame_bytes.key != "maximum-frame-bytes"
        || retained_bytes.key != "maximum-retained-bytes"
    {
        return Err("browser transcript bounds are mislabeled".into());
    }
    let convert = |entry: &conduit_core::ConfigurationEntry| match &entry.value {
        ConfigurationValue::U64(value) => usize::try_from(*value).ok(),
        _ => None,
    };
    let (Some(items), Some(events), Some(frame_bytes), Some(retained_bytes)) = (
        convert(items),
        convert(events),
        convert(frame_bytes),
        convert(retained_bytes),
    ) else {
        return Err("browser transcript bounds are invalid".into());
    };
    if events == 0 || events > conduit_net::MAXIMUM_RECORD_TRANSCRIPT_EVENTS {
        return Err("browser transcript event bound is outside reviewed limits".into());
    }
    conduit_net::BoundedRecordTranscript::new(items, frame_bytes, retained_bytes, 0)
        .map_err(|error| format!("browser transcript limits: {error:?}"))?;
    Ok((items, events, frame_bytes, retained_bytes))
}

fn exact_leaf<'a>(canonical: &'a [u8], value_type: &[u8]) -> Result<&'a [u8], ()> {
    let node = canonical.strip_prefix(value_type).ok_or(())?;
    if node.first() != Some(&0) || node.len() < 5 {
        return Err(());
    }
    let length = usize::try_from(u32::from_le_bytes(node[1..5].try_into().map_err(|_| ())?))
        .map_err(|_| ())?;
    (node.len() == 5 + length).then_some(&node[5..]).ok_or(())
}

fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
