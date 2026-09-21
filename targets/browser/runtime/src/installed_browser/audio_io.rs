//! Browser-owned bounded PCM capture and safe playback installations.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{
    kind_id, resource_requirement, AuthorityContractId, AuthorityRequirement, CapabilityOffer,
    HostCallContractId, HostCallRequirement, PlannedGear,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef, ValueStorage,
};

pub(crate) const CAPTURE_IMPLEMENTATION: &str = "browser/pcm-push-to-talk@1";
pub(crate) const PLAY_IMPLEMENTATION: &str = "browser/webaudio-safe-pcm@1";
const PROFILE: &str = "browser/pcm-human-audio@1";
const ARTIFACT: &str = "conduit-browser-runtime/pcm-human-audio@1";
pub(crate) const CAPTURE_OPERATION: &str = "conduit.host/browser-pcm-push-to-talk@1";
pub(crate) const PLAY_OPERATION: &str = "conduit.host/browser-safe-pcm-playback@1";
pub(crate) const CAPTURE_RESOURCE: &str = "conduit.resource/browser-microphone-turn@1";
pub(crate) const PLAY_RESOURCE: &str = "conduit.resource/browser-audio-output@1";
pub(crate) const CAPTURE_POOL: &str = "browser/microphone-turn";
pub(crate) const PLAY_POOL: &str = "browser/audio-output";
pub(crate) const CAPTURE_AUTHORITY: &str = "conduit.authority/request-browser-microphone@1";
pub(crate) const PLAY_AUTHORITY: &str = "conduit.authority/use-browser-audio-output@1";
pub(crate) const MAXIMUM_SAFE_GAIN_MILLIONTHS: u32 = 50_000;
pub(crate) const MAXIMUM_CAPTURE_REQUESTS: u32 = 8_192;
const _: () = assert!(MAXIMUM_SAFE_GAIN_MILLIONTHS <= 50_000);
const _: () = assert!(MAXIMUM_CAPTURE_REQUESTS > 0);

pub(crate) static CAPTURE: BrowserInstallation = BrowserInstallation {
    implementation_id: CAPTURE_IMPLEMENTATION,
    offer: capture_offer,
    prepare: prepare_capture,
    perform: None,
};

pub(crate) static PLAYBACK: BrowserInstallation = BrowserInstallation {
    implementation_id: PLAY_IMPLEMENTATION,
    offer: playback_offer,
    prepare: prepare_playback,
    perform: None,
};

pub(crate) fn capture_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::audio_capture_push_to_talk_contract();
    conduit_semantic_catalog::realization_offer(
        contract,
        conduit_semantic_catalog::AUDIO_CAPTURE_PUSH_TO_TALK_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: CAPTURE_IMPLEMENTATION,
            execution_profile: PROFILE,
            implementation: CAPTURE_IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![HostCallRequirement {
            contract_id: HostCallContractId::from(CAPTURE_OPERATION),
            target_kind: Some(kind_id(
                conduit_semantic_catalog::AUDIO_CAPTURE_PUSH_TO_TALK_KIND,
            )),
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: conduit_audio::MAXIMUM_PCM_FRAME_BYTES
                + conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN as u32,
        }],
        vec![resource_requirement(CAPTURE_RESOURCE, 1)],
        vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(CAPTURE_AUTHORITY),
            host_call_contract_id: HostCallContractId::from(CAPTURE_OPERATION),
            subject_kind: kind_id(conduit_semantic_catalog::AUDIO_CAPTURE_PUSH_TO_TALK_KIND),
        }],
    )
}

pub(crate) fn playback_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::audio_play_contract();
    conduit_semantic_catalog::realization_offer(
        contract,
        conduit_semantic_catalog::AUDIO_PLAY_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: PLAY_IMPLEMENTATION,
            execution_profile: PROFILE,
            implementation: PLAY_IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![HostCallRequirement {
            contract_id: HostCallContractId::from(PLAY_OPERATION),
            target_kind: Some(kind_id(conduit_audio::AUDIO_PCM_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_audio::MAXIMUM_PCM_FRAME_BYTES
                + conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        vec![resource_requirement(PLAY_RESOURCE, 1)],
        vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(PLAY_AUTHORITY),
            host_call_contract_id: HostCallContractId::from(PLAY_OPERATION),
            subject_kind: kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
        }],
    )
}

fn prepare_capture(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &capture_offer())?;
    let request = values
        .store(&[0])
        .map_err(|error| format!("store browser microphone request: {error:?}"))?;
    Ok(BrowserBack::installed_step(CaptureBack {
        request,
        next: 0,
        pending: false,
        completed: false,
    }))
}

fn prepare_playback(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &playback_offer())?;
    Ok(BrowserBack::installed_step(PlaybackBack {
        next: 0,
        pending: None,
    }))
}

struct CaptureBack {
    request: ValueRef,
    next: u32,
    pending: bool,
    completed: bool,
}

impl CaptureBack {
    fn request<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) {
        io.request_host_call(
            RequestId(self.next),
            HostCallId(0),
            BoundedValueRef::new(self.request, 1).expect("capture request is one byte"),
        )
        .expect("browser capture Host Call");
        self.pending = true;
    }
}

impl<const PORTS: usize> StepBack<PORTS> for CaptureBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(self.next) {
                return invalid(1);
            }
            if outcome.disposition != HostCallDisposition::Completed || outcome.failure.is_some() {
                return host_failure(outcome.disposition);
            }
            let Some(output) = outcome.output else {
                io.consume_host_completion()
                    .expect("observed completed browser capture");
                self.pending = false;
                self.completed = true;
                return StepOutcome::Complete;
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(next) = self.next.checked_add(1) else {
                return identity_exhausted();
            };
            io.consume_host_completion()
                .expect("observed browser capture frame");
            io.send(PortId(0), output.value)
                .expect("ready browser capture output");
            self.pending = false;
            if next >= MAXIMUM_CAPTURE_REQUESTS {
                self.completed = true;
                return StepOutcome::Complete;
            }
            self.next = next;
            self.request(io);
            return StepOutcome::Progress;
        }
        if !self.pending && !self.completed {
            self.request(io);
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.completed = true;
    }
}

struct PlaybackBack {
    next: u32,
    pending: Option<RequestId>,
}

impl<const PORTS: usize> StepBack<PORTS> for PlaybackBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return invalid(3);
            }
            if outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
                || outcome.output.is_some()
            {
                return host_failure(outcome.disposition);
            }
            let Some(next) = self.next.checked_add(1) else {
                return identity_exhausted();
            };
            io.consume_host_completion()
                .expect("observed browser playback completion");
            self.pending = None;
            self.next = next;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return invalid(4);
            }
            let Some(canonical) = input_bytes.input(PortId(0)) else {
                return invalid(5);
            };
            if canonical.len() != value.byte_len as usize {
                return invalid(5);
            }
            let Ok((header, payload)) = conduit_audio::PcmFrameHeader::decode_frame(canonical)
            else {
                return invalid(5);
            };
            if header.validate_payload(payload).is_err() {
                return invalid(6);
            }
            let request = RequestId(self.next);
            let input = BoundedValueRef::new(
                value,
                conduit_audio::MAXIMUM_PCM_FRAME_BYTES
                    + conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN as u32,
            )
            .expect("validated PCM fits the playback operation");
            io.consume(PortId(0)).expect("present PCM frame");
            io.request_host_call(request, HostCallId(0), input)
                .expect("browser playback Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed PCM input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

fn invalid(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

fn host_failure(disposition: HostCallDisposition) -> StepOutcome {
    let code = match disposition {
        HostCallDisposition::Denied => FailureCode::HostCallDenied,
        HostCallDisposition::Cancelled => FailureCode::Cancelled,
        _ => FailureCode::HostCallFailed,
    };
    StepOutcome::Fail(Failure { code, detail: 7 })
}

fn identity_exhausted() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::IdentityCapacityExhausted,
        detail: 8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::HostCallOutcome;

    fn value(slot: u16, byte_len: usize) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: byte_len as u32,
        }
    }

    #[test]
    fn browser_pcm_offers_seal_distinct_resource_and_authority_truth() {
        let capture = capture_offer();
        assert_eq!(
            capture.kind_id.as_str(),
            conduit_semantic_catalog::AUDIO_CAPTURE_PUSH_TO_TALK_KIND
        );
        assert_eq!(
            capture.host_calls[0].contract_id.as_str(),
            CAPTURE_OPERATION
        );
        assert_eq!(
            capture.resource_requirements[0].class_id.as_str(),
            CAPTURE_RESOURCE
        );
        assert_eq!(
            capture.authority_requirements[0].contract_id.as_str(),
            CAPTURE_AUTHORITY
        );
        assert_eq!(
            capture.host_calls[0].target_kind,
            Some(capture.authority_requirements[0].subject_kind.clone())
        );
        assert_eq!(
            capture.outputs[0].temporal,
            conduit_core::PortTemporal::Flow { closes: true }
        );

        let playback = playback_offer();
        assert_eq!(
            playback.kind_id.as_str(),
            conduit_semantic_catalog::AUDIO_PLAY_KIND
        );
        assert_eq!(playback.host_calls[0].contract_id.as_str(), PLAY_OPERATION);
        assert_eq!(
            playback.resource_requirements[0].class_id.as_str(),
            PLAY_RESOURCE
        );
        assert_eq!(
            playback.authority_requirements[0].contract_id.as_str(),
            PLAY_AUTHORITY
        );
    }

    #[test]
    fn capture_preserves_pending_frame_under_output_pressure_and_rearms() {
        let mut operation = CaptureBack {
            request: value(0, 1),
            next: 0,
            pending: false,
            completed: false,
        };
        let mut start = StepIo::test_frame([None], [false], [Some(64)], None, 4);
        assert_eq!(
            operation.step(&mut start, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert_eq!(
            start.test_host_request().map(|request| request.0),
            Some(RequestId(0))
        );
        let frame = value(1, 32);
        let outcome = HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: Some(BoundedValueRef::new(frame, 64).unwrap()),
            failure: None,
        };
        let mut blocked =
            StepIo::test_frame([None], [false], [None], Some((RequestId(0), outcome)), 5);
        assert_eq!(
            operation.step(&mut blocked, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Await
        );
        assert!(operation.pending);
        assert_eq!(operation.next, 0);
        let mut ready = StepIo::test_frame(
            [None],
            [false],
            [Some(64)],
            Some((RequestId(0), outcome)),
            5,
        );
        assert_eq!(
            operation.step(&mut ready, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert_eq!(ready.test_output(PortId(0)), Some(frame));
        assert_eq!(
            ready.test_host_request().map(|request| request.0),
            Some(RequestId(1))
        );
    }

    #[test]
    fn playback_validates_pcm_before_request_and_completes_after_closure() {
        let payload = [0_u8; 4];
        let frame = conduit_audio::PcmFrameHeader::new(
            conduit_audio::PcmSampleRepresentation::Signed16LittleEndian,
            16_000,
            conduit_audio::PcmChannelLayout::Mono,
            2,
            1,
            0,
            false,
        )
        .unwrap()
        .encode_frame(&payload)
        .unwrap();
        let input = value(1, frame.len());
        let mut operation = PlaybackBack {
            next: 0,
            pending: None,
        };
        let mut io = StepIo::test_frame([Some(input)], [false], [None], None, 4);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([Some(&frame)], None),),
            StepOutcome::Progress
        );
        assert_eq!(
            io.test_host_request().map(|request| request.0),
            Some(RequestId(0))
        );
        let completion = HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: None,
            failure: None,
        };
        let mut completed =
            StepIo::test_frame([None], [false], [None], Some((RequestId(0), completion)), 4);
        assert_eq!(
            operation.step(&mut completed, &StepInputBytes::test_frame([None], None),),
            StepOutcome::Progress
        );
        let mut closed = StepIo::test_frame([None], [true], [None], None, 4);
        assert_eq!(
            operation.step(&mut closed, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Complete
        );
    }
}
