//! Allocation-stable std realization of bounded record transcript retention.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{Failure, FailureCode, OperationAction, OperationInput, PortId, ValueRef};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::RECORD_TRANSCRIPT_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct RecordTranscriptOperation {
    transcript: conduit_net::BoundedRecordTranscript,
    maximum_events: usize,
    events: usize,
    closed: [bool; 3],
    framed_type: Vec<u8>,
    terminal_type: Vec<u8>,
}

impl RecordTranscriptOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Closed { port: PortId(port) } if port < 3 => {
                self.closed[usize::from(port)] = true;
                if self.closed.iter().all(|closed| *closed) {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 270),
        }
    }

    pub(super) fn resume_value(
        &mut self,
        port: PortId,
        value: ValueRef,
        canonical: &[u8],
    ) -> OperationAction {
        let index = usize::from(port.0);
        if index >= 3
            || self.closed[index]
            || self.events >= self.maximum_events
            || value.byte_len > MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
        {
            return fail(FailureCode::StorageExhausted, 271);
        }
        let result = match index {
            0 | 1 => super::typed_record_operation::typed_leaf(canonical, &self.framed_type)
                .map_err(|_| ())
                .and_then(|frame| {
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
            _ => super::typed_record_operation::typed_leaf(canonical, &self.terminal_type)
                .map_err(|_| ())
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
            return fail(FailureCode::InvalidInput, 272);
        }
        self.events += 1;
        OperationAction::Emit { port, value }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn cancel(&mut self) {}
}

fn limits(placement: &PlannedGear) -> Result<(usize, usize, usize, usize), String> {
    let [items, events, frame_bytes, retained_bytes] = placement.configuration.as_slice() else {
        return Err("record transcript requires four exact bounds".into());
    };
    let values = [items, events, frame_bytes, retained_bytes].map(|entry| match entry.value {
        ConfigurationValue::U64(value) => usize::try_from(value).ok(),
        _ => None,
    });
    if items.key != "maximum-items"
        || events.key != "maximum-events"
        || frame_bytes.key != "maximum-frame-bytes"
        || retained_bytes.key != "maximum-retained-bytes"
    {
        return Err("record transcript bounds are mislabeled".into());
    }
    let [Some(items), Some(events), Some(frame_bytes), Some(retained_bytes)] = values else {
        return Err("record transcript bounds are invalid".into());
    };
    if events == 0 || events > conduit_net::MAXIMUM_RECORD_TRANSCRIPT_EVENTS {
        return Err("record transcript event bound is outside reviewed limits".into());
    }
    conduit_net::BoundedRecordTranscript::new(items, frame_bytes, retained_bytes, 0)
        .map_err(|error| format!("record transcript limits: {error:?}"))?;
    Ok((items, events, frame_bytes, retained_bytes))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::record_transcript_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.limits != offer.limits
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err("planned transcript differs from installed realization".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    let (_, events, frame_bytes, _) = limits(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: u16::try_from(events.saturating_mul(6).saturating_add(16))
            .map_err(|_| "record transcript sign budget overflow")?,
        maximum_value_bytes: u32::try_from(frame_bytes.saturating_add(512))
            .map_err(|_| "record transcript value bound overflow")?,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let (items, events, frame_bytes, retained_bytes) = limits(placement)?;
    Ok(InstalledOperation::RecordTranscript(
        RecordTranscriptOperation {
            transcript: conduit_net::BoundedRecordTranscript::new(
                items,
                frame_bytes,
                retained_bytes,
                0,
            )
            .map_err(|error| format!("prepare bounded transcript: {error:?}"))?,
            maximum_events: events,
            events: 0,
            closed: [false; 3],
            framed_type: conduit_net::framed_typed_record_type()
                .canonical_bytes()
                .map_err(|error| format!("encode framed record type: {error:?}"))?,
            terminal_type: conduit_net::terminal_event_type()
                .canonical_bytes()
                .map_err(|error| format!("encode terminal event type: {error:?}"))?,
        },
    ))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
