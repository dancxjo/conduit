//! Portable frame and bounded Situation-to-Experience adapter for Pete.
use conduit_chat::{
    episode_experience_type, EpisodeExperience, EpisodeExperienceInput, EpisodeObservation,
};
use conduit_core::{
    kind_id, port_id, Back, BackOfferBuilder, CapabilityLimits, CapabilityOffer, Kind,
    PortDescriptor, PortDirection, PortTemporal, TemporalInstant,
};
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};
use conduit_robotics::{BatteryObservation, ChargingState};

use crate::{
    pete_situation_experiences, select_pete_situation, Observed, PeteBodyState, SituationInput,
};

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/cognition_types.rs"));
}

pub use generated::{
    PeteCognitionBattery, PeteCognitionBatteryInput, PeteCognitionFrame, PeteCognitionSafety,
    PeteCognitionSign, PeteCognitionUtterance,
};

#[path = "cognition_inputs.rs"]
mod inputs;
pub use inputs::*;

pub const PETE_COGNITION_SITUATE_KIND: &str = "pete/situate-experience";
pub const MAXIMUM_PETE_COGNITION_FRAME_BYTES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeteCognitionRefusal {
    InvalidObservation,
    Situation,
    Experience,
    Evidence,
    Homeostasis,
}

/// Project a correlated Create battery sample using its source Sign and capture time.
pub fn pete_cognition_battery_from_create(
    observation: &crate::CreatePortableObservation,
    sign: PeteCognitionSign,
    observed_at: conduit_time::NativeTemporalInstant,
) -> Result<Option<PeteCognitionBattery>, PeteCognitionRefusal> {
    let sample = observation.group_zero.charging;
    let Some(battery) = sample
        .battery()
        .map_err(|_| PeteCognitionRefusal::InvalidObservation)?
    else {
        return Ok(None);
    };
    let charging = matches!(
        sample.state,
        ChargingState::Reconditioning | ChargingState::Full | ChargingState::Trickle
    );
    PeteCognitionBattery::new(
        battery.charge_permille(),
        charging,
        battery.millivolts(),
        observed_at,
        sign,
    )
    .map(Some)
    .map_err(|_| PeteCognitionRefusal::InvalidObservation)
}

#[derive(Debug)]
pub enum CreateCognitionCaptureError<E> {
    Deadline,
    Clock(E),
    Start(crate::CreateObservationFailure),
    Read {
        failure: crate::CreateObservationFailure,
        pause_failure: Option<crate::CreateObservationFailure>,
    },
    Pause(crate::CreateObservationFailure),
    Observation(PeteCognitionRefusal),
    Time,
}

/// Capture one correlated sample and pause the stream before publishing it.
/// The clock must be the provider's monotonic millisecond clock.
pub fn capture_pete_cognition_battery<P: crate::CreateUartProvider, E>(
    provider: &mut P,
    sign: PeteCognitionSign,
    clock_basis: String,
    read_timeout_ms: u32,
    mut monotonic_millis: impl FnMut() -> Result<u64, E>,
) -> Result<Option<PeteCognitionBattery>, CreateCognitionCaptureError<E>> {
    use conduit_time::{NativeTemporalInstant, NativeTemporalScale};
    use CreateCognitionCaptureError as Error;

    if !(1..=5_000).contains(&read_timeout_ms) {
        return Err(Error::Deadline);
    }
    let deadline = monotonic_millis()
        .map_err(Error::Clock)?
        .checked_add(u64::from(read_timeout_ms))
        .ok_or(Error::Deadline)?;
    let mut session = crate::CreateObservationSession::new();
    session.start(provider).map_err(Error::Start)?;
    let observation = session.read(provider, deadline);
    let captured_at = monotonic_millis();
    let pause = session.pause(provider);
    let observation = observation.map_err(|failure| Error::Read {
        failure,
        pause_failure: pause.as_ref().err().copied(),
    })?;
    pause.map_err(Error::Pause)?;
    let captured_at = NativeTemporalInstant::new(
        clock_basis,
        1,
        NativeTemporalScale::Milliseconds,
        captured_at.map_err(Error::Clock)?,
        0,
    )
    .map_err(|_| Error::Time)?;
    pete_cognition_battery_from_create(&observation, sign, captured_at).map_err(Error::Observation)
}

impl TryFrom<PeteCognitionBattery> for PeteCognitionBatteryInput {
    type Error = conduit_plot::rust_binding::NativeBindingRefusal;

    fn try_from(battery: PeteCognitionBattery) -> Result<Self, Self::Error> {
        Self::observed(
            *battery.charge_permille(),
            *battery.charging(),
            *battery.millivolts(),
            battery.observed_at().clone(),
            battery.sign().clone(),
        )
    }
}

pub fn situate_pete_frame(
    frame: &PeteCognitionFrame,
) -> Result<EpisodeExperience, PeteCognitionRefusal> {
    let input = prepare_pete_experience_input(frame)?;
    conduit_chat::integrate_episode_experience(
        &input,
        conduit_semantic_catalog::experience_evidence_limits(),
    )
    .map_err(|error| match error {
        conduit_human::ExperienceIntegrationError::Evidence(_) => PeteCognitionRefusal::Evidence,
        conduit_human::ExperienceIntegrationError::Admission(_) => PeteCognitionRefusal::Experience,
    })
}

/// Prepare sourced generic inputs. CurrentExperience admission belongs to its Gear.
pub fn prepare_pete_experience_input(
    frame: &PeteCognitionFrame,
) -> Result<EpisodeExperienceInput, PeteCognitionRefusal> {
    use PeteCognitionRefusal as R;
    let instant = |instant: &conduit_time::NativeTemporalInstant| {
        TemporalInstant::try_from(instant.clone()).map_err(|_| R::InvalidObservation)
    };
    let battery = match frame.battery() {
        PeteCognitionBatteryInput::Observed(battery) => Some(battery),
        _ => None,
    };
    let body = frame.body();
    let utterance = frame.utterance();
    let policy = *frame.temporal_policy();
    let battery_observation = battery
        .as_ref()
        .map(|battery| {
            Ok(Observed {
                value: BatteryObservation::new(*battery.charge_permille(), *battery.millivolts())
                    .map_err(|_| R::InvalidObservation)?,
                sign_id: battery.sign().get().as_str().into(),
                observed_at: instant(battery.observed_at())?,
            })
        })
        .transpose()?;
    let safety_observation = body
        .as_ref()
        .map(|body| {
            Ok(Observed {
                value: PeteBodyState {
                    motion_available: *body.motion_available(),
                    safety_inhibited: *body.safety_inhibited(),
                },
                sign_id: body.sign().get().as_str().into(),
                observed_at: instant(body.observed_at())?,
            })
        })
        .transpose()?;
    let situation = select_pete_situation(
        instant(frame.reference())?,
        *policy.maximum_current_age_ticks(),
        match frame.battery() {
            PeteCognitionBatteryInput::Unavailable(source) => SituationInput::Unavailable {
                source_identity: source.source_identity().clone(),
            },
            _ => battery_observation
                .clone()
                .map_or(SituationInput::Missing, SituationInput::Observed),
        },
        SituationInput::Observed(Observed {
            value: utterance.content().clone(),
            sign_id: utterance.sign().get().as_str().into(),
            observed_at: instant(utterance.observed_at())?,
        }),
        safety_observation
            .clone()
            .map_or(SituationInput::Missing, SituationInput::Observed),
    )
    .map_err(|_| R::Situation)?;
    let mut inputs = pete_situation_experiences(&situation, policy).map_err(|_| R::Experience)?;
    let homeostasis = super::cognition_homeostasis::homeostatic_evidence(
        &situation,
        battery_observation
            .as_ref()
            .zip(battery.as_ref())
            .map(|(observed, input)| (observed, *input.charging())),
        policy,
        safety_observation.as_ref(),
    )?;
    inputs.items.push(homeostasis);
    let evidence = conduit_human::prepare_experience_evidence(
        &inputs.items,
        &inputs.relationships,
        *frame.revision(),
        frame.provenance_revision(),
    )
    .map_err(|_| R::Evidence)?;
    let trigger = EpisodeObservation::new(
        utterance.content().clone(),
        utterance.sign().get().clone(),
        *utterance.retain(),
    )
    .map_err(|_| R::InvalidObservation)?;
    let input =
        conduit_human::ExperienceIntegrationInput::new(evidence, policy, frame.reference().clone())
            .map_err(|_| R::Evidence)?;
    EpisodeExperienceInput::new(input, trigger).map_err(|_| R::Evidence)
}

/// Byte boundary for a Host that has selected the exact frame Back.
pub fn situate_pete_frame_bytes(bytes: &[u8]) -> Result<Vec<u8>, u16> {
    use conduit_plot::rust_binding::NativeRustBinding;
    if bytes.len() > MAXIMUM_PETE_COGNITION_FRAME_BYTES {
        return Err(1);
    }
    let frame = PeteCognitionFrame::decode(bytes).map_err(|_| 1_u16)?;
    let episode = situate_pete_frame(&frame).map_err(|refusal| match refusal {
        PeteCognitionRefusal::InvalidObservation => 2_u16,
        PeteCognitionRefusal::Situation => 3,
        PeteCognitionRefusal::Experience => 4,
        PeteCognitionRefusal::Evidence => 5,
        PeteCognitionRefusal::Homeostasis => 6,
    })?;
    episode.encode().map_err(|_| 5)
}

pub fn pete_cognition_situate_contract() -> Kind {
    let port = |name, ty: conduit_core::StructuredInfoType, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: ty.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(PETE_COGNITION_SITUATE_KIND),
        kind_contract_revision: "conduit.pete/situate-experience@1".into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![port(
            "frame",
            PeteCognitionFrame::semantic_type().unwrap(),
            PortDirection::Input,
        )],
        outputs: vec![port(
            "episode",
            episode_experience_type(),
            PortDirection::Output,
        )],
        configuration: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
        },
    }
}

pub fn pete_cognition_situate_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        pete_cognition_situate_contract(),
        Back {
            capability_id: "pete-situate-experience".into(),
            execution_profile_id: "conduit.std/pete-situate-experience-kernel@1".into(),
            implementation_id: "std/pete-situate-experience@1".into(),
            artifact_id: "conduit-std-host/pete-situate-experience@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

pub fn install_pete_cognition_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert_structured_type(
        "PeteCognitionFrame",
        PeteCognitionFrame::semantic_type().unwrap(),
    )?;
    startup.insert(KindSignature {
        kind: PETE_COGNITION_SITUATE_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert_kind(pete_cognition_situate_contract())
        .map_err(|error| error.to_string())?;
    startup.insert(KindSignature {
        kind: PETE_COGNITION_INPUTS_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert_kind(pete_cognition_inputs_contract())
        .map_err(|error| error.to_string())
}
