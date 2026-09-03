//! Age-first model context for Pete's exact Create observations.

use crate::{CreateObservationChannel, CreateObservationSnapshot};
use conduit_ai::{
    ClockBasis, InterpretationEvidence, InterpretationRequest,
    TemporalReference as AiTemporalReference,
};
use conduit_core::{
    BootId, HostId, OfferGeneration, SignId, TemporalInstant, TemporalRelation, TemporalScale,
    UNIX_UTC_CLOCK_BASIS,
};
use conduit_presentation::{
    project_model_temporal_context, ModelTemporalContextFact, Presentation, PresentationBasis,
    PresentationError, PresentationProperty, PresentationPropertyValue, PresentationRole,
    PresentationSubject, PresentationTemporalFact, PresentationTemporalRole, TemporalReference,
};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_PETE_MODEL_CONTEXT_BYTES: usize = conduit_ai::MAXIMUM_INTERPRETATION_TEXT_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeteCreateObservationFreshness {
    Current,
    Expired,
    Indeterminate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeteCreateObservationIdentity {
    pub subject: String,
    pub robot_identity: String,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub observation_generation: u32,
    pub channel: String,
}

/// Transient context for one model turn. `observed.relative_time` deliberately
/// serializes first; exact observation and clock provenance follow it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeteCreateModelContext {
    pub observed: ModelTemporalContextFact,
    pub freshness: PeteCreateObservationFreshness,
    pub maximum_age_ticks: u32,
    pub observation: PeteCreateObservationIdentity,
    pub request: InterpretationRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeteCreateModelContextRefusal {
    InvalidObservation,
    ObservationInstantMismatch,
    ObservationAfterReference,
    InvalidTemporalTruth,
    UnsupportedModelClock,
    InvalidModelRequest,
    InvalidPresentation(PresentationError),
    ContextTooLarge,
}

pub fn project_pete_create_model_context(
    snapshot: &CreateObservationSnapshot,
    channel: CreateObservationChannel,
    sign_id: SignId,
    source: TemporalInstant,
    reference: TemporalReference,
) -> Result<PeteCreateModelContext, PeteCreateModelContextRefusal> {
    if snapshot.robot_identity.is_empty()
        || snapshot.observation_generation == 0
        || snapshot.maximum_age_ticks == 0
    {
        return Err(PeteCreateModelContextRefusal::InvalidObservation);
    }
    if source.ticks != snapshot.observed_at_tick {
        return Err(PeteCreateModelContextRefusal::ObservationInstantMismatch);
    }

    let observation = observation_identity(snapshot, channel);
    let fact = PresentationTemporalFact::new(
        observation.subject.clone(),
        PresentationTemporalRole::Observation,
        Some(sign_id.clone()),
        source,
        &reference,
    )
    .map_err(|_| PeteCreateModelContextRefusal::InvalidTemporalTruth)?;
    let freshness = freshness(fact.relation, snapshot.maximum_age_ticks)?;
    let presentation = Presentation::new_with_semantics_and_temporal(
        u64::from(snapshot.observation_generation),
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_form_id: None,
            expanded_form_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![sign_id.clone()],
        },
        vec![PresentationSubject {
            identity: observation.subject.clone(),
            role: PresentationRole::Info,
            label: format!("Pete Create {} observation", channel_name(channel)),
            accessibility_name: format!("Pete Create {} observation", channel_name(channel)),
        }],
        vec![],
        observation_properties(&observation, snapshot.maximum_age_ticks),
        vec![],
        vec![],
        vec![],
        vec![reference],
        vec![fact],
    )
    .map_err(PeteCreateModelContextRefusal::InvalidPresentation)?;
    let mut projected = project_model_temporal_context(&presentation)
        .map_err(PeteCreateModelContextRefusal::InvalidPresentation)?;
    let observed = projected
        .pop()
        .ok_or(PeteCreateModelContextRefusal::InvalidObservation)?;
    let context_json = serde_json::to_string(&(&observed, freshness, &observation))
        .map_err(|_| PeteCreateModelContextRefusal::InvalidObservation)?;
    if context_json.len() > MAXIMUM_PETE_MODEL_CONTEXT_BYTES {
        return Err(PeteCreateModelContextRefusal::ContextTooLarge);
    }
    let request = InterpretationRequest {
        evidence: vec![InterpretationEvidence {
            sign_id,
            observation: format!(
                "{} observed {}; freshness {:?}",
                observation.channel, observed.relative_time, freshness
            ),
        }],
        context: context_json,
        temporal_reference: ai_reference(&observed.reference)?,
        temporal_intent: None,
    };
    request
        .validate()
        .map_err(|_| PeteCreateModelContextRefusal::InvalidModelRequest)?;
    let context = PeteCreateModelContext {
        observed,
        freshness,
        maximum_age_ticks: snapshot.maximum_age_ticks,
        observation,
        request,
    };
    if serde_json::to_vec(&context)
        .map_err(|_| PeteCreateModelContextRefusal::InvalidObservation)?
        .len()
        > MAXIMUM_PETE_MODEL_CONTEXT_BYTES
    {
        return Err(PeteCreateModelContextRefusal::ContextTooLarge);
    }
    Ok(context)
}

fn ai_reference(
    reference: &TemporalReference,
) -> Result<AiTemporalReference, PeteCreateModelContextRefusal> {
    if reference.instant.scale != TemporalScale::Milliseconds {
        return Err(PeteCreateModelContextRefusal::UnsupportedModelClock);
    }
    let clock_basis = if reference.instant.clock_basis == UNIX_UTC_CLOCK_BASIS {
        ClockBasis::UnixEpochMilliseconds
    } else {
        ClockBasis::MonotonicMilliseconds {
            identity: reference.instant.clock_basis.clone(),
        }
    };
    let value = AiTemporalReference {
        reference_at: reference.instant.ticks,
        clock_basis,
    };
    value
        .validate()
        .map_err(|_| PeteCreateModelContextRefusal::UnsupportedModelClock)?;
    Ok(value)
}

fn freshness(
    relation: TemporalRelation,
    maximum_age_ticks: u32,
) -> Result<PeteCreateObservationFreshness, PeteCreateModelContextRefusal> {
    let maximum_age_ticks = u64::from(maximum_age_ticks);
    match relation {
        TemporalRelation::Present => Ok(PeteCreateObservationFreshness::Current),
        TemporalRelation::Past { maximum_ticks, .. } if maximum_ticks <= maximum_age_ticks => {
            Ok(PeteCreateObservationFreshness::Current)
        }
        TemporalRelation::Past { minimum_ticks, .. } if minimum_ticks > maximum_age_ticks => {
            Ok(PeteCreateObservationFreshness::Expired)
        }
        TemporalRelation::Past { .. } | TemporalRelation::Indeterminate => {
            Ok(PeteCreateObservationFreshness::Indeterminate)
        }
        TemporalRelation::Future { .. } => {
            Err(PeteCreateModelContextRefusal::ObservationAfterReference)
        }
    }
}

fn observation_identity(
    snapshot: &CreateObservationSnapshot,
    channel: CreateObservationChannel,
) -> PeteCreateObservationIdentity {
    PeteCreateObservationIdentity {
        subject: format!("pete/create-observation/{}", channel_name(channel)),
        robot_identity: snapshot.robot_identity.clone(),
        host_id: snapshot.host_id.clone(),
        boot_id: snapshot.boot_id.clone(),
        offer_generation: snapshot.offer_generation,
        observation_generation: snapshot.observation_generation,
        channel: channel_name(channel).into(),
    }
}

fn observation_properties(
    observation: &PeteCreateObservationIdentity,
    maximum_age_ticks: u32,
) -> Vec<PresentationProperty> {
    let subject = &observation.subject;
    [
        (
            "robot-identity",
            PresentationPropertyValue::Identity(observation.robot_identity.clone()),
        ),
        (
            "host-id",
            PresentationPropertyValue::Identity(observation.host_id.as_str().into()),
        ),
        (
            "boot-id",
            PresentationPropertyValue::Identity(observation.boot_id.as_str().into()),
        ),
        (
            "offer-generation",
            PresentationPropertyValue::Count(observation.offer_generation.0),
        ),
        (
            "observation-generation",
            PresentationPropertyValue::Count(u64::from(observation.observation_generation)),
        ),
        (
            "channel",
            PresentationPropertyValue::Text(observation.channel.clone()),
        ),
        (
            "maximum-age-ticks",
            PresentationPropertyValue::Count(u64::from(maximum_age_ticks)),
        ),
    ]
    .into_iter()
    .map(|(name, value)| PresentationProperty {
        subject: subject.clone(),
        name: name.into(),
        value,
    })
    .collect()
}

const fn channel_name(channel: CreateObservationChannel) -> &'static str {
    match channel {
        CreateObservationChannel::Contact => "contact",
        CreateObservationChannel::Cliff => "cliff",
        CreateObservationChannel::WheelDrop => "wheel-drop",
        CreateObservationChannel::Proximity => "proximity",
        CreateObservationChannel::VirtualWall => "virtual-wall",
        CreateObservationChannel::Infrared => "infrared",
        CreateObservationChannel::Buttons => "buttons",
        CreateObservationChannel::Charging => "charging",
        CreateObservationChannel::Battery => "battery",
        CreateObservationChannel::Odometry => "odometry",
        CreateObservationChannel::BumpAggregate => "bump",
    }
}

#[cfg(test)]
#[path = "pete_model_context_tests.rs"]
mod tests;
