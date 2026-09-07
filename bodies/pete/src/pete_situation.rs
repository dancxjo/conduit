//! Finite Pete-specific selection over reusable typed observation families.

use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, KindId, PortDescriptor,
    PortDirection, PortTemporal, SignId, TemporalInstant, TemporalRelation,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};
use conduit_robotics::{BatteryObservation, ROBOTICS_BATTERY_INFO_ID};
use serde::Serialize;

pub const PETE_SITUATION_SELECT_KIND: &str = "pete/situation-select";
pub const PETE_SITUATION_REVISION: &str = "conduit.pete/situation-select@1";
pub const PETE_SITUATION_INFO_KIND: &str = "pete/situation@1";
pub const PETE_BODY_STATE_INFO_KIND: &str = "body/current-safety-availability@1";
pub const PETE_UTTERANCE_INFO_KIND: &str = "human/utterance-observation@1";
pub const MAXIMUM_PETE_SITUATION_BYTES: usize = 16_384;
pub const MAXIMUM_PETE_UTTERANCE_BYTES: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observed<T> {
    pub value: T,
    pub sign_id: SignId,
    pub observed_at: TemporalInstant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SituationInput<T> {
    Observed(Observed<T>),
    Missing,
    Unavailable {
        source_identity: String,
    },
    Contradictory {
        first: Observed<T>,
        second: Observed<T>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PeteBodyState {
    pub motion_available: bool,
    pub safety_inhibited: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PeteBatteryValue {
    pub charge_permille: u16,
    pub millivolts: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PeteUtteranceValue {
    pub text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum SituationFactState {
    Current,
    Stale,
    Indeterminate,
    Unavailable,
    Missing,
    Contradictory,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SituationProvenance {
    pub sign_id: String,
    pub observed_at: TemporalInstant,
    pub relation: TemporalRelation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PeteSituationFact<T> {
    pub value_kind: String,
    pub state: SituationFactState,
    pub value: Option<T>,
    pub alternatives: Vec<T>,
    pub provenance: Vec<SituationProvenance>,
    pub unavailable_source: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PeteSituation {
    pub reference: TemporalInstant,
    pub maximum_age_ticks: u64,
    pub battery: PeteSituationFact<PeteBatteryValue>,
    pub utterance: PeteSituationFact<PeteUtteranceValue>,
    pub body: PeteSituationFact<PeteBodyState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeteSituationRefusal {
    InvalidMaximumAge,
    InvalidUtterance,
    InvalidUnavailableSource,
    InvalidTemporalTruth,
    FutureObservation,
    DuplicateContradictionSource,
    Encoding,
    SituationTooLarge,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeteSituationContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

pub fn select_pete_situation(
    reference: TemporalInstant,
    maximum_age_ticks: u64,
    battery: SituationInput<BatteryObservation>,
    utterance: SituationInput<String>,
    body: SituationInput<PeteBodyState>,
) -> Result<PeteSituation, PeteSituationRefusal> {
    if maximum_age_ticks == 0 {
        return Err(PeteSituationRefusal::InvalidMaximumAge);
    }
    reference
        .validate()
        .map_err(|_| PeteSituationRefusal::InvalidTemporalTruth)?;
    let situation = PeteSituation {
        battery: project(
            battery,
            &reference,
            maximum_age_ticks,
            ROBOTICS_BATTERY_INFO_ID,
            |value| PeteBatteryValue {
                charge_permille: value.charge_permille(),
                millivolts: value.millivolts(),
            },
        )?,
        utterance: project(
            validate_utterance(utterance)?,
            &reference,
            maximum_age_ticks,
            PETE_UTTERANCE_INFO_KIND,
            |text| PeteUtteranceValue { text },
        )?,
        body: project(
            body,
            &reference,
            maximum_age_ticks,
            PETE_BODY_STATE_INFO_KIND,
            |value| value,
        )?,
        reference,
        maximum_age_ticks,
    };
    let encoded = serde_json::to_vec(&situation).map_err(|_| PeteSituationRefusal::Encoding)?;
    if encoded.len() > MAXIMUM_PETE_SITUATION_BYTES {
        return Err(PeteSituationRefusal::SituationTooLarge);
    }
    Ok(situation)
}

fn project<T, U, F>(
    input: SituationInput<T>,
    reference: &TemporalInstant,
    maximum_age_ticks: u64,
    value_kind: &str,
    map: F,
) -> Result<PeteSituationFact<U>, PeteSituationRefusal>
where
    F: Fn(T) -> U,
{
    let (state, value, alternatives, provenance, unavailable_source) = match input {
        SituationInput::Observed(observed) => {
            let provenance = temporal_provenance(&observed, reference)?;
            let state = classify_relation(provenance.relation, maximum_age_ticks)?;
            (
                state,
                Some(map(observed.value)),
                vec![],
                vec![provenance],
                None,
            )
        }
        SituationInput::Missing => (SituationFactState::Missing, None, vec![], vec![], None),
        SituationInput::Unavailable { source_identity } => {
            if source_identity.is_empty() || source_identity.len() > 256 {
                return Err(PeteSituationRefusal::InvalidUnavailableSource);
            }
            (
                SituationFactState::Unavailable,
                None,
                vec![],
                vec![],
                Some(source_identity),
            )
        }
        SituationInput::Contradictory { first, second } => {
            if first.sign_id == second.sign_id {
                return Err(PeteSituationRefusal::DuplicateContradictionSource);
            }
            let first_provenance = temporal_provenance(&first, reference)?;
            let second_provenance = temporal_provenance(&second, reference)?;
            let alternatives = vec![map(first.value), map(second.value)];
            (
                SituationFactState::Contradictory,
                None,
                alternatives,
                vec![first_provenance, second_provenance],
                None,
            )
        }
    };
    Ok(PeteSituationFact {
        value_kind: value_kind.into(),
        state,
        value,
        alternatives,
        provenance,
        unavailable_source,
    })
}

fn temporal_provenance<T>(
    observed: &Observed<T>,
    reference: &TemporalInstant,
) -> Result<SituationProvenance, PeteSituationRefusal> {
    if observed.sign_id.as_str().is_empty() {
        return Err(PeteSituationRefusal::InvalidTemporalTruth);
    }
    let relation = observed
        .observed_at
        .relation_to(reference)
        .map_err(|_| PeteSituationRefusal::InvalidTemporalTruth)?;
    if matches!(relation, TemporalRelation::Future { .. }) {
        return Err(PeteSituationRefusal::FutureObservation);
    }
    Ok(SituationProvenance {
        sign_id: observed.sign_id.as_str().into(),
        observed_at: observed.observed_at.clone(),
        relation,
    })
}

fn classify_relation(
    relation: TemporalRelation,
    maximum_age_ticks: u64,
) -> Result<SituationFactState, PeteSituationRefusal> {
    match relation {
        TemporalRelation::Present => Ok(SituationFactState::Current),
        TemporalRelation::Past { maximum_ticks, .. } if maximum_ticks <= maximum_age_ticks => {
            Ok(SituationFactState::Current)
        }
        TemporalRelation::Past { minimum_ticks, .. } if minimum_ticks > maximum_age_ticks => {
            Ok(SituationFactState::Stale)
        }
        TemporalRelation::Past { .. } | TemporalRelation::Indeterminate => {
            Ok(SituationFactState::Indeterminate)
        }
        TemporalRelation::Future { .. } => Err(PeteSituationRefusal::FutureObservation),
    }
}

fn validate_utterance(
    input: SituationInput<String>,
) -> Result<SituationInput<String>, PeteSituationRefusal> {
    let valid = |value: &String| !value.is_empty() && value.len() <= MAXIMUM_PETE_UTTERANCE_BYTES;
    match &input {
        SituationInput::Observed(value) if !valid(&value.value) => {
            Err(PeteSituationRefusal::InvalidUtterance)
        }
        SituationInput::Contradictory { first, second }
            if !valid(&first.value) || !valid(&second.value) =>
        {
            Err(PeteSituationRefusal::InvalidUtterance)
        }
        _ => Ok(input),
    }
}

pub fn pete_situation_contract() -> PeteSituationContract {
    PeteSituationContract {
        kind_id: kind_id(PETE_SITUATION_SELECT_KIND),
        kind_contract_revision: KindContractRevision::from(PETE_SITUATION_REVISION),
        inputs: vec![
            port("battery", ROBOTICS_BATTERY_INFO_ID, PortDirection::Input),
            port("utterance", PETE_UTTERANCE_INFO_KIND, PortDirection::Input),
            port("body", PETE_BODY_STATE_INFO_KIND, PortDirection::Input),
        ],
        outputs: vec![port(
            "situation",
            PETE_SITUATION_INFO_KIND,
            PortDirection::Output,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 3,
            max_queue_bytes: MAXIMUM_PETE_SITUATION_BYTES as u32,
        },
    }
}

pub fn install_pete_situation_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for (alias, kind) in [
        ("BatteryObservation", ROBOTICS_BATTERY_INFO_ID),
        ("UtteranceObservation", PETE_UTTERANCE_INFO_KIND),
        ("BodyCurrentState", PETE_BODY_STATE_INFO_KIND),
        ("PeteSituation", PETE_SITUATION_INFO_KIND),
    ] {
        startup.insert_value_kind_alias(alias, kind_id(kind))?;
    }
    startup.insert(KindSignature {
        kind: PETE_SITUATION_SELECT_KIND.into(),
        startup_parameters: vec![],
    })?;
    let contract = pete_situation_contract();
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}

#[cfg(test)]
#[path = "pete_situation_tests.rs"]
mod tests;
