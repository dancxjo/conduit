//! Pull-based installed operation for bounded hosted speech synthesis.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::PIPER_SPEECH_IMPLEMENTATION,
    budget,
    prepare,
};

#[cfg(test)]
pub(super) static DETERMINISTIC_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::DETERMINISTIC_SPEECH_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct SpeechSynthesisOperation {
    continuation: ValueRef,
    pending: Option<RequestId>,
    next_request: u32,
    emitted_blocks: u16,
    maximum_blocks: u16,
    started: bool,
    finished: bool,
}

impl SpeechSynthesisOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.started && self.pending.is_none() => {
                let Ok(input) = BoundedValueRef::new(value, conduit_tongues::MAXIMUM_TEXT_BYTES)
                else {
                    return fail(FailureCode::InvalidInput, 1);
                };
                if value.byte_len == 0 {
                    return fail(FailureCode::InvalidInput, 2);
                }
                self.started = true;
                self.request(input)
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None)
                        if self.emitted_blocks < self.maximum_blocks
                            && output.admitted_bytes
                                == conduit_std_offers::PIPER_PCM_BLOCK_BYTES
                            && output.value.byte_len
                                <= conduit_std_offers::PIPER_PCM_BLOCK_BYTES =>
                    {
                        self.emitted_blocks += 1;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Completed, None, None) if self.started => {
                        self.finished = true;
                        OperationAction::Complete
                    }
                    (HostOperationDisposition::Denied, _, _) => {
                        fail(FailureCode::HostOperationDenied, 3)
                    }
                    (HostOperationDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 4),
                    (HostOperationDisposition::Failed, _, _) => {
                        fail(FailureCode::HostOperationFailed, 5)
                    }
                    (HostOperationDisposition::Completed, Some(_), None) => {
                        fail(FailureCode::WorkBudgetExhausted, 6)
                    }
                    _ => fail(FailureCode::InvalidLifecycle, 7),
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 8),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.finished {
            return OperationAction::Complete;
        }
        if self.started && self.pending.is_none() {
            let input = BoundedValueRef::new(self.continuation, 1)
                .expect("prepared speech continuation marker is one byte");
            return self.request(input);
        }
        OperationAction::Await
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.finished = true;
    }

    fn request(&mut self, input: BoundedValueRef) -> OperationAction {
        let request = RequestId(self.next_request);
        self.next_request = self.next_request.saturating_add(1);
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input,
        }
    }
}

fn maximum_output_bytes(placement: &PlannedGear) -> Result<u32, String> {
    let value = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-output-bytes", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or_else(|| "speech synthesis output bound is missing".to_string())?;
    let value = u32::try_from(value)
        .map_err(|_| "speech synthesis output bound does not fit the kernel".to_string())?;
    if value == 0 || value > conduit_tongues::MAXIMUM_PCM_BYTES {
        return Err("speech synthesis output bound is outside the portable contract".into());
    }
    Ok(value)
}

fn maximum_blocks(placement: &PlannedGear) -> Result<u16, String> {
    let bytes = maximum_output_bytes(placement)?;
    let payload_per_block = u32::from(conduit_std_offers::PIPER_FRAMES_PER_BLOCK) * 2;
    let blocks = u16::try_from(bytes.div_ceil(payload_per_block))
        .map_err(|_| "speech synthesis block bound does not fit the kernel".to_string())?;
    if blocks > conduit_std_offers::PIPER_MAXIMUM_BLOCKS {
        return Err("speech synthesis block bound exceeds its execution profile".into());
    }
    Ok(blocks)
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = match placement.implementation_id.as_str() {
        conduit_std_offers::PIPER_SPEECH_IMPLEMENTATION => conduit_std_offers::piper_speech_offer(),
        conduit_std_offers::DETERMINISTIC_SPEECH_IMPLEMENTATION => {
            conduit_std_offers::deterministic_speech_offer()
        }
        _ => return Err("planned speech implementation is not installed".into()),
    };
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || !placement.authority.is_empty()
        || placement.configuration.len() != 1
    {
        return Err("planned Piper speech identity does not match its installation".into());
    }
    let requires_process =
        placement.implementation_id.as_str() == conduit_std_offers::PIPER_SPEECH_IMPLEMENTATION;
    if requires_process
        != (placement.resources.len() == 1
            && placement.resources[0].class_id.as_str()
                == conduit_std_offers::PIPER_PROCESS_RESOURCE_CLASS
            && placement.resources[0].units == 1
            && placement.resources[0].protected.is_none()
            && placement.resources[0].compute.is_none())
    {
        return Err("planned Piper process reservation is not exact".into());
    }
    maximum_output_bytes(placement)?;
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    let maximum_blocks = maximum_blocks(placement)?;
    Ok(OperationBudget {
        // The source Text remains live until its first Host completion while
        // that completion stores one output block. The continuation marker is
        // retained for all later pulls.
        value_items: 3,
        value_bytes: 1
            + conduit_tongues::MAXIMUM_TEXT_BYTES
            + conduit_std_offers::PIPER_PCM_BLOCK_BYTES,
        host_requests: usize::from(maximum_blocks) + 1,
        sign_items: 64,
        maximum_value_bytes: conduit_std_offers::PIPER_PCM_BLOCK_BYTES
            .max(conduit_tongues::MAXIMUM_TEXT_BYTES),
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let continuation = values
        .store(&[0])
        .map_err(|error| format!("store speech continuation marker: {error:?}"))?;
    Ok(InstalledOperation::SpeechSynthesis(
        SpeechSynthesisOperation {
            continuation,
            pending: None,
            next_request: 0,
            emitted_blocks: 0,
            maximum_blocks: maximum_blocks(placement)?,
            started: false,
            finished: false,
        },
    ))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

pub(super) fn execute_piper<'a>(
    adapter: Option<&'a mut crate::hosted_speech::PiperSpeechAdapter>,
    input: &[u8],
    cancelled: bool,
) -> Result<Option<&'a [u8]>, crate::hosted_speech::PiperFailure> {
    let adapter = adapter.ok_or(crate::hosted_speech::PiperFailure::MissingProvider)?;
    if adapter.is_active() {
        if input != [0] {
            return Err(crate::hosted_speech::PiperFailure::InvalidText);
        }
    } else {
        let text = core::str::from_utf8(input)
            .map_err(|_| crate::hosted_speech::PiperFailure::InvalidText)?;
        adapter.begin(text)?;
    }
    match adapter.next(|| cancelled)? {
        crate::hosted_speech::PiperSynthesisStep::Block(block) => Ok(Some(block)),
        crate::hosted_speech::PiperSynthesisStep::Complete(_) => Ok(None),
    }
}

#[cfg(test)]
pub(super) struct FakeSpeechHost {
    blocks: [Vec<u8>; 3],
    next: usize,
    active: bool,
}

#[cfg(test)]
impl FakeSpeechHost {
    pub(super) fn execute(&mut self, input: &[u8]) -> Result<Option<&[u8]>, String> {
        if !self.active {
            let text = core::str::from_utf8(input)
                .map_err(|_| "fake speech input is not UTF-8".to_string())?;
            if text.is_empty() {
                return Err("fake speech input is empty".into());
            }
            self.active = true;
        } else if input != [0] {
            return Err("fake speech continuation marker is malformed".into());
        }
        let output = self.blocks.get(self.next).map(Vec::as_slice);
        self.next += usize::from(output.is_some());
        Ok(output)
    }
}

#[cfg(test)]
pub(super) fn prepare_fake_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Result<Vec<Option<FakeSpeechHost>>, String> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            if placement.implementation_id.as_str()
                != conduit_std_offers::DETERMINISTIC_SPEECH_IMPLEMENTATION
            {
                return Ok(None);
            }
            validate(placement)?;
            let blocks: [Vec<u8>; 3] = core::array::from_fn(|index| {
                let frame_count = 4;
                let header = conduit_audio::PcmFrameHeader::new(
                    conduit_audio::PcmSampleRepresentation::Signed16LittleEndian,
                    22_050,
                    conduit_audio::PcmChannelLayout::Mono,
                    frame_count,
                    0x5350_4545_4348,
                    index as u64 * u64::from(frame_count),
                    false,
                )
                .expect("fake speech profile is canonical");
                let sample = i16::try_from(index + 1).expect("three blocks fit i16");
                let mut payload = Vec::with_capacity(usize::from(frame_count) * 2);
                for _ in 0..frame_count {
                    payload.extend_from_slice(&sample.to_le_bytes());
                }
                header
                    .encode_frame(&payload)
                    .expect("fake speech block is canonical")
            });
            Ok(Some(FakeSpeechHost {
                blocks,
                next: 0,
                active: false,
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(slot: u16, byte_len: u32) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len,
        }
    }

    #[test]
    fn operation_pulls_one_block_only_after_each_emit_commits() {
        let mut operation = SpeechSynthesisOperation {
            continuation: value(9, 1),
            pending: None,
            next_request: 0,
            emitted_blocks: 0,
            maximum_blocks: 2,
            started: false,
            finished: false,
        };
        assert!(matches!(operation.start(), OperationAction::Await));
        assert!(matches!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: value(1, 7),
            }),
            OperationAction::RequestHostOperation {
                request: RequestId(0),
                ..
            }
        ));
        let output = BoundedValueRef::new(
            value(2, conduit_std_offers::PIPER_PCM_BLOCK_BYTES),
            conduit_std_offers::PIPER_PCM_BLOCK_BYTES,
        )
        .unwrap();
        assert!(matches!(
            operation.resume(OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome: conduit_kernel::HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(output),
                    failure: None,
                },
            }),
            OperationAction::Emit {
                port: PortId(0),
                ..
            }
        ));
        assert!(matches!(
            operation.advance(),
            OperationAction::RequestHostOperation {
                request: RequestId(1),
                ..
            }
        ));
    }

    #[test]
    fn operation_refuses_a_provider_block_beyond_the_admitted_count() {
        let mut operation = SpeechSynthesisOperation {
            continuation: value(9, 1),
            pending: Some(RequestId(2)),
            next_request: 3,
            emitted_blocks: 2,
            maximum_blocks: 2,
            started: true,
            finished: false,
        };
        let output =
            BoundedValueRef::new(value(3, 1), conduit_std_offers::PIPER_PCM_BLOCK_BYTES).unwrap();
        assert!(matches!(
            operation.resume(OperationInput::HostOperationCompleted {
                request: RequestId(2),
                outcome: conduit_kernel::HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(output),
                    failure: None,
                },
            }),
            OperationAction::Fail(Failure {
                code: FailureCode::WorkBudgetExhausted,
                ..
            })
        ));
    }

    #[test]
    fn fake_provider_emits_three_canonical_contiguous_profile_blocks() {
        let blocks: [Vec<u8>; 3] = core::array::from_fn(|index| {
            let header = conduit_audio::PcmFrameHeader::new(
                conduit_audio::PcmSampleRepresentation::Signed16LittleEndian,
                22_050,
                conduit_audio::PcmChannelLayout::Mono,
                4,
                0x5350_4545_4348,
                index as u64 * 4,
                false,
            )
            .unwrap();
            header.encode_frame(&[0; 8]).unwrap()
        });
        let mut host = FakeSpeechHost {
            blocks,
            next: 0,
            active: false,
        };
        for (index, input) in [b"Rosehip".as_slice(), &[0], &[0]].into_iter().enumerate() {
            let block = host.execute(input).unwrap().unwrap();
            let (header, payload) = conduit_audio::PcmFrameHeader::decode_frame(block).unwrap();
            assert_eq!(header.sample_rate_hz, 22_050);
            assert_eq!(header.layout, conduit_audio::PcmChannelLayout::Mono);
            assert_eq!(header.start_frame, index as u64 * 4);
            assert_eq!(payload.len(), 8);
        }
        assert_eq!(host.execute(&[0]).unwrap(), None);
    }
}
