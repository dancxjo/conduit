//! Installed kernel Backs for the stages unique to a spoken Mask.

use super::back::{BackBudget, BackFactory, InstalledBack};
use crate::spoken_mask_runtime::{
    ArtifactAcknowledgedShowBack, ClosingNoInteractionBack, GeneratedManifestationToSpeechBack,
    PresentationToGenerativeRequestBack,
};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostedValueStore,
    PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) static PRESENTATION_REQUEST_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::PRESENTATION_REQUEST_IMPLEMENTATION,
    budget: adapter_budget,
    prepare: prepare_presentation_request,
};
pub(super) static GENERATED_SPEECH_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::GENERATED_SPEECH_IMPLEMENTATION,
    budget: adapter_budget,
    prepare: prepare_generated_speech,
};
pub(super) static SPOKEN_ARTIFACT_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::SPOKEN_ARTIFACT_IMPLEMENTATION,
    budget: artifact_budget,
    prepare: prepare_artifact,
};
pub(super) static ARTIFACT_SHOW_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::ARTIFACT_SHOW_IMPLEMENTATION,
    budget: show_budget,
    prepare: prepare_show,
};
pub(super) static NO_INTERACTION_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::NO_INTERACTION_IMPLEMENTATION,
    budget: no_interaction_budget,
    prepare: prepare_no_interaction,
};

pub(super) struct SpokenArtifactBack {
    pending: Option<RequestId>,
    next_request: u32,
    drain_marker: ValueRef,
    closing: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for SpokenArtifactBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) || outcome.failure.is_some() {
                return outcome.failure.map_or_else(|| fail(31), StepOutcome::Fail);
            }
            match (self.closing, outcome.disposition, outcome.output) {
                (false, HostCallDisposition::Completed, None) => {
                    io.consume_host_completion()
                        .expect("written spoken PCM block");
                    self.pending = None;
                    StepOutcome::Progress
                }
                (true, HostCallDisposition::Completed, Some(output)) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("acknowledged spoken artifact");
                    io.send(PortId(0), output.value)
                        .expect("ready spoken artifact receipt");
                    io.discard(self.drain_marker)
                        .expect("release spoken artifact drain marker");
                    self.pending = None;
                    StepOutcome::Complete
                }
                _ => fail(32),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() || self.closing {
                return fail(33);
            }
            let Ok(input) = BoundedValueRef::new(
                value,
                conduit_std_offers::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BYTES,
            ) else {
                return fail(34);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return fail(35);
            };
            io.consume(PortId(0)).expect("present spoken PCM block");
            io.request_host_call(request, HostCallId(0), input)
                .expect("spoken artifact write Host Call");
            self.next_request = next;
            self.pending = Some(request);
            StepOutcome::Progress
        } else if io.input_closed(PortId(0)) && self.pending.is_none() && !self.closing {
            let request = RequestId(self.next_request);
            let input = BoundedValueRef::new(self.drain_marker, 1)
                .expect("spoken artifact drain marker is bounded");
            io.consume_closed(PortId(0))
                .expect("observed spoken PCM closure");
            io.request_host_call(request, HostCallId(0), input)
                .expect("spoken artifact finish Host Call");
            self.pending = Some(request);
            self.closing = true;
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }

    fn retains_host_call_input(&self, _: RequestId, value: ValueRef) -> bool {
        value == self.drain_marker
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.closing = true;
    }
}

fn prepare_presentation_request(
    placement: &PlannedGear,
    _: &mut HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(
        placement,
        conduit_std_offers::PRESENTATION_REQUEST_IMPLEMENTATION,
    )?;
    Ok(InstalledBack::SpokenPresentationRequest(
        PresentationToGenerativeRequestBack::new(
            conduit_presentation::MAX_GENERATIVE_PRESENTER_INPUT_BYTES as u32,
        )
        .map_err(str::to_string)?,
    ))
}

fn prepare_generated_speech(
    placement: &PlannedGear,
    _: &mut HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(
        placement,
        conduit_std_offers::GENERATED_SPEECH_IMPLEMENTATION,
    )?;
    Ok(InstalledBack::SpokenGeneratedSpeech(
        GeneratedManifestationToSpeechBack::new(
            conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32,
        )
        .map_err(str::to_string)?,
    ))
}

fn prepare_artifact(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(
        placement,
        conduit_std_offers::SPOKEN_ARTIFACT_IMPLEMENTATION,
    )?;
    let drain_marker = values
        .store(&[0])
        .map_err(|error| format!("store spoken artifact drain marker: {error:?}"))?;
    Ok(InstalledBack::SpokenArtifact(SpokenArtifactBack {
        pending: None,
        next_request: 0,
        drain_marker,
        closing: false,
    }))
}

fn prepare_show(
    placement: &PlannedGear,
    _: &mut HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement, conduit_std_offers::ARTIFACT_SHOW_IMPLEMENTATION)?;
    Ok(InstalledBack::SpokenArtifactShow(
        ArtifactAcknowledgedShowBack::new(
            conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32,
            4_096,
        )
        .map_err(str::to_string)?,
    ))
}

fn prepare_no_interaction(
    placement: &PlannedGear,
    _: &mut HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement, conduit_std_offers::NO_INTERACTION_IMPLEMENTATION)?;
    Ok(InstalledBack::SpokenNoInteraction(ClosingNoInteractionBack))
}

fn validate(placement: &PlannedGear, implementation: &str) -> Result<(), String> {
    let offer = conduit_std_offers::spoken_mask_offers()
        .into_iter()
        .find(|offer| offer.implementation.implementation_id.as_str() == implementation)
        .ok_or_else(|| "unknown spoken Mask implementation".to_string())?;
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.host_calls != offer.host_calls
        || !placement.configuration.is_empty()
    {
        return Err("planned spoken Mask stage differs from installed realization".into());
    }
    Ok(())
}

fn adapter_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement, placement.implementation_id.as_str())?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: conduit_presentation::MAX_GENERATIVE_PRESENTER_INPUT_BYTES as u32 * 2,
        host_requests: 1,
        sign_items: 16,
        maximum_value_bytes: conduit_presentation::MAX_GENERATIVE_PRESENTER_INPUT_BYTES as u32,
    })
}

fn artifact_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(
        placement,
        conduit_std_offers::SPOKEN_ARTIFACT_IMPLEMENTATION,
    )?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: 4_097,
        host_requests: usize::from(conduit_semantic_catalog::AUDIO_PLAY_ALSA_MAXIMUM_BLOCKS) + 1,
        sign_items: 64,
        maximum_value_bytes: 4_096,
    })
}

fn show_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement, conduit_std_offers::ARTIFACT_SHOW_IMPLEMENTATION)?;
    Ok(BackBudget {
        value_items: 3,
        value_bytes: conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32 + 266_240,
        host_requests: 2,
        sign_items: 24,
        maximum_value_bytes: 262_144,
    })
}

fn no_interaction_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement, conduit_std_offers::NO_INTERACTION_IMPLEMENTATION)?;
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 4,
        maximum_value_bytes: 0,
    })
}

const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

pub(super) struct SpokenArtifactHost {
    session: crate::hosted_wav_artifact::WavArtifactSession,
    plan_id: conduit_core::PlanId,
    active_play_id: conduit_core::ActivePlayId,
    placement_id: conduit_core::PlacementId,
}

pub(super) fn prepare_artifact_hosts(
    fragment: &conduit_core::PlanFragment,
    active_play: &conduit_core::ActivePlayIdentity,
    selection: Option<&crate::hosted_wav_artifact::WavArtifactSelection>,
) -> Result<Vec<Option<SpokenArtifactHost>>, String> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            if placement.implementation_id.as_str()
                != conduit_std_offers::SPOKEN_ARTIFACT_IMPLEMENTATION
            {
                return Ok(None);
            }
            validate(
                placement,
                conduit_std_offers::SPOKEN_ARTIFACT_IMPLEMENTATION,
            )?;
            let selection = selection
                .ok_or_else(|| "spoken Mask has no selected artifact destination".to_string())?;
            if selection.boot_id != placement.boot_id
                || selection.offer_generation != placement.offer_generation
            {
                return Err("spoken Mask artifact destination is stale".into());
            }
            Ok(Some(SpokenArtifactHost {
                session: crate::hosted_wav_artifact::WavArtifactSession::prepare(selection.clone()),
                plan_id: fragment.plan_id.clone(),
                active_play_id: active_play.active_play_id.clone(),
                placement_id: placement.placement_id.clone(),
            }))
        })
        .collect()
}

pub(super) fn execute_artifact(
    host: &mut SpokenArtifactHost,
    input: &[u8],
    completion_sign_id: conduit_core::SignId,
) -> Result<Option<Vec<u8>>, String> {
    if input == [0] {
        host.session.finish()?;
        let report = host.session.report();
        let content_sha256 = host
            .session
            .content_sha256()
            .ok_or_else(|| "completed spoken artifact has no digest".to_string())?;
        let receipt = conduit_presentation::SpokenMaskArtifactReceipt {
            artifact_identity: format!("artifact/wav/{content_sha256}"),
            content_sha256,
            pcm_bytes: report.pcm_bytes,
            frames: report.frames,
            blocks: report.blocks,
            plan_id: host.plan_id.clone(),
            active_play_id: host.active_play_id.clone(),
            placement_id: host.placement_id.clone(),
            completion_sign_id,
        };
        serde_json::to_vec(&receipt)
            .map(Some)
            .map_err(|error| format!("encode spoken artifact receipt: {error}"))
    } else {
        host.session.write_frame(input)?;
        Ok(None)
    }
}
