use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{CapabilityOffer, PlannedGear, PortDirection};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostedValueStore,
    PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) const DRAIN_MARKER: [u8; 1] = [0xff];
pub(super) const HOST_CALL: &str = conduit_std_offers::AUDIO_PLAY_ALSA_HW_OPERATION;

pub(super) static AUDIO_PLAY_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct AudioPlayBack {
    pending: Option<RequestId>,
    next_request: u32,
    drain_marker: ValueRef,
    draining: bool,
    closed: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for AudioPlayBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(60);
            }
            if let Some(failure) = outcome.failure {
                return StepOutcome::Fail(failure);
            }
            if outcome.disposition != HostCallDisposition::Completed || outcome.output.is_some() {
                return step_fail(61);
            }
            io.consume_host_completion()
                .expect("observed audio playback completion");
            self.pending = None;
            return if self.draining {
                StepOutcome::Complete
            } else {
                StepOutcome::Progress
            };
        }

        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some()
                || self.closed
                || self.next_request
                    >= u32::from(conduit_semantic_catalog::AUDIO_PLAY_ALSA_MAXIMUM_BLOCKS)
            {
                return step_fail(60);
            }
            let Ok(input) = BoundedValueRef::new(
                value,
                conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
            ) else {
                return step_fail(62);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return step_fail(62);
            };
            io.consume(PortId(0)).expect("present audio playback block");
            io.request_host_call(request, HostCallId(0), input)
                .expect("audio playback Host Call");
            self.next_request = next;
            self.pending = Some(request);
            self.draining = false;
            return StepOutcome::Progress;
        }

        if io.input_closed(PortId(0)) && self.pending.is_none() && !self.closed {
            let request = RequestId(self.next_request);
            let Ok(input) = BoundedValueRef::new(
                self.drain_marker,
                conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
            ) else {
                return step_fail(62);
            };
            io.consume_closed(PortId(0))
                .expect("observed audio playback closure");
            io.request_host_call(request, HostCallId(0), input)
                .expect("audio playback drain Host Call");
            self.next_request = self.next_request.saturating_add(1);
            self.pending = Some(request);
            self.draining = true;
            self.closed = true;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.closed = true;
    }
}

const fn step_fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

impl AudioPlayBack {}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: DRAIN_MARKER.len() as u32,
        host_requests: usize::from(conduit_semantic_catalog::AUDIO_PLAY_ALSA_MAXIMUM_BLOCKS) + 1,
        sign_items: 64,
        maximum_value_bytes: conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    let drain_marker = values
        .store(&DRAIN_MARKER)
        .map_err(|error| format!("store audio/play drain marker: {error:?}"))?;
    Ok(InstalledBack::AudioPlay(AudioPlayBack {
        pending: None,
        next_request: 0,
        drain_marker,
        draining: false,
        closed: false,
    }))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = offer();
    let resource = placement.resources.first();
    let authority = placement.authority.first();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.limits != offer.limits
        || placement.inputs.len() != 1
        || placement.inputs[0].port_id.as_str() != "audio"
        || placement.inputs[0].direction != PortDirection::Input
        || placement.resources.len() != 1
        || resource.is_none_or(|binding| {
            binding.class_id.as_str() != conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS
                || binding.units != 1
                || binding.protected.is_some()
                || binding.compute.is_some()
        })
        || placement.authority.len() != 1
        || authority.is_none_or(|binding| {
            binding.contract_id.as_str() != conduit_std_offers::AUDIO_PLAYBACK_AUTHORITY_CONTRACT
                || binding.host_call_contract_id.as_str() != HOST_CALL
                || binding.subject_kind.as_str() != conduit_audio::AUDIO_PCM_INFO_ID
                || binding.host_id != placement.host_id
                || binding.boot_id != placement.boot_id
                || binding.capability_id != placement.capability_id
        })
        || !placement.configuration.is_empty()
    {
        return Err(
            "planned audio/play identity/resource/authority does not match installation"
                .to_string(),
        );
    }
    Ok(())
}

pub(super) fn offer() -> CapabilityOffer {
    conduit_std_offers::audio_play_alsa_hw_offer()
}

pub(super) fn prepare_session(
    placement: &PlannedGear,
    selected: Option<&crate::hosted_audio::HostedPlaybackSelection>,
) -> Result<crate::hosted_audio::PlaybackSession, String> {
    validate(placement)?;
    let selected = selected.ok_or_else(|| {
        "planned audio/play has no exact selected hosted playback resource".to_string()
    })?;
    if selected.boot_id != placement.boot_id
        || selected.offer_generation != placement.offer_generation
        || placement.resources[0].pool_id != selected.pool_id()
        || placement.realization_characteristics
            != selected
                .realization_advertisement(placement.host_id.clone())
                .characteristics
    {
        return Err("planned audio/play resource is stale or differs from selection".to_string());
    }
    Ok(crate::hosted_audio::PlaybackSession::resolved(
        selected.clone(),
    ))
}

pub(super) fn execute(
    session: &mut crate::hosted_audio::PlaybackSession,
    input: &[u8],
) -> conduit_kernel::HostCallOutcome {
    let result = if input == DRAIN_MARKER {
        session.drain()
    } else {
        session.write_frame(input)
    };
    match result {
        Ok(()) => conduit_kernel::HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: None,
            failure: None,
        },
        Err(error) => failure_outcome(error),
    }
}

fn failure_outcome(error: crate::hosted_audio::PlaybackFailure) -> conduit_kernel::HostCallOutcome {
    use crate::hosted_audio::PlaybackFailure;
    let (disposition, code, detail) = match error {
        PlaybackFailure::StaleObservation => (
            conduit_kernel::HostCallDisposition::Denied,
            conduit_kernel::FailureCode::HostCallDenied,
            70,
        ),
        PlaybackFailure::DeviceBusy => (
            conduit_kernel::HostCallDisposition::Denied,
            conduit_kernel::FailureCode::HostCallDenied,
            71,
        ),
        PlaybackFailure::OpenFailed => (
            conduit_kernel::HostCallDisposition::Failed,
            conduit_kernel::FailureCode::HostCallFailed,
            72,
        ),
        PlaybackFailure::InvalidPcm | PlaybackFailure::DiscontinuousInput => (
            conduit_kernel::HostCallDisposition::Failed,
            conduit_kernel::FailureCode::InvalidInput,
            73,
        ),
        PlaybackFailure::Underrun => (
            conduit_kernel::HostCallDisposition::Failed,
            conduit_kernel::FailureCode::HostCallFailed,
            74,
        ),
        PlaybackFailure::ProviderLost => (
            conduit_kernel::HostCallDisposition::Failed,
            conduit_kernel::FailureCode::HostCallFailed,
            75,
        ),
        PlaybackFailure::WriteFailed => (
            conduit_kernel::HostCallDisposition::Failed,
            conduit_kernel::FailureCode::HostCallFailed,
            76,
        ),
        PlaybackFailure::DrainFailed => (
            conduit_kernel::HostCallDisposition::Failed,
            conduit_kernel::FailureCode::HostCallFailed,
            77,
        ),
        PlaybackFailure::CloseFailed => (
            conduit_kernel::HostCallDisposition::Failed,
            conduit_kernel::FailureCode::HostCallFailed,
            78,
        ),
        PlaybackFailure::InvalidLifecycle => (
            conduit_kernel::HostCallDisposition::Failed,
            conduit_kernel::FailureCode::InvalidLifecycle,
            79,
        ),
    };
    conduit_kernel::HostCallOutcome {
        disposition,
        output: None,
        failure: Some(conduit_kernel::Failure { code, detail }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{
        scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
        HostCallOutcome,
    };

    #[test]
    fn input_is_serialized_and_close_requests_exact_drain() {
        let mut operation = AudioPlayBack {
            pending: None,
            next_request: 0,
            drain_marker: ValueRef {
                slot: 9,
                generation: 1,
                byte_len: 1,
            },
            draining: false,
            closed: false,
        };
        let value = ValueRef {
            slot: 2,
            generation: 1,
            byte_len: 100,
        };
        let mut io = StepIo::test_frame([Some(value)], [false], [None], None, 8);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert_eq!(
            io.test_host_request()
                .map(|request| (request.0, request.2.value)),
            Some((RequestId(0), value))
        );
        assert!(io.test_consumed(PortId(0)));
        let mut io = StepIo::test_frame(
            [None],
            [false],
            [None],
            Some((
                RequestId(0),
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )),
            8,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert!(io.test_host_completion_consumed());
        let mut io = StepIo::test_frame([None], [true], [None], None, 8);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert_eq!(
            io.test_host_request()
                .map(|request| (request.0, request.2.value)),
            Some((RequestId(1), operation.drain_marker))
        );
        assert!(io.test_consumed_closed(PortId(0)));
    }
}
