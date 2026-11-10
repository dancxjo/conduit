//! Portable frame and bounded Situation-to-Experience adapter for Pete.
use conduit_chat::{episode_experience_type, EpisodeExperience, EpisodeObservation};
use conduit_core::{
    kind_id, port_id, Back, BackOfferBuilder, CapabilityLimits, CapabilityOffer, Kind,
    PortDescriptor, PortDirection, PortTemporal, TemporalInstant,
};
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};
use conduit_robotics::BatteryObservation;

use crate::{
    integrate_pete_experience, project_pete_experience_evidence, select_pete_situation, Observed,
    PeteBodyState, SituationInput,
};

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/cognition_types.rs"));
}

pub use generated::{
    PeteCognitionBattery, PeteCognitionFrame, PeteCognitionSafety, PeteCognitionSign,
    PeteCognitionUtterance,
};

pub const PETE_COGNITION_SITUATE_KIND: &str = "pete/situate-experience";
pub const MAXIMUM_PETE_COGNITION_FRAME_BYTES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeteCognitionRefusal {
    InvalidObservation,
    Situation,
    Experience,
    Evidence,
}

pub fn situate_pete_frame(
    frame: &PeteCognitionFrame,
) -> Result<EpisodeExperience, PeteCognitionRefusal> {
    use PeteCognitionRefusal as R;
    let instant = |instant: &conduit_time::NativeTemporalInstant| {
        TemporalInstant::try_from(instant.clone()).map_err(|_| R::InvalidObservation)
    };
    let battery = frame.battery();
    let body = frame.body();
    let utterance = frame.utterance();
    let policy = *frame.temporal_policy();
    let situation = select_pete_situation(
        instant(frame.reference())?,
        *policy.maximum_current_age_ticks(),
        SituationInput::Observed(Observed {
            value: BatteryObservation::new(*battery.charge_permille(), *battery.millivolts())
                .map_err(|_| R::InvalidObservation)?,
            sign_id: battery.sign().get().as_str().into(),
            observed_at: instant(battery.observed_at())?,
        }),
        SituationInput::Observed(Observed {
            value: utterance.content().clone(),
            sign_id: utterance.sign().get().as_str().into(),
            observed_at: instant(utterance.observed_at())?,
        }),
        SituationInput::Observed(Observed {
            value: PeteBodyState {
                motion_available: *body.motion_available(),
                safety_inhibited: *body.safety_inhibited(),
            },
            sign_id: body.sign().get().as_str().into(),
            observed_at: instant(body.observed_at())?,
        }),
    )
    .map_err(|_| R::Situation)?;
    let experience =
        integrate_pete_experience(&situation, policy, &[]).map_err(|_| R::Experience)?;
    let evidence = project_pete_experience_evidence(
        &experience,
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
    EpisodeExperience::new(evidence, trigger).map_err(|_| R::Evidence)
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
        .map_err(|error| error.to_string())
}
