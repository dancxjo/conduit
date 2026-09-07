//! Finite, deterministic observation-driven state for Signal Garden compositions.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, Scalar, StructuredFieldType, StructuredFieldValue, StructuredInfoRefusal,
    StructuredInfoType, StructuredInfoValue, StructuredInfoValueShape, SCALAR_INFO_ID,
};

pub const GARDEN_STATE_TYPE: &str = "GardenState";
pub const GARDEN_CLOCK_OBSERVATION_TYPE: &str = "GardenClockObservation";
pub const GARDEN_CONTACT_OBSERVATION_TYPE: &str = "GardenContactObservation";
pub const GARDEN_STATE_INFO_ID: &str = "garden/state@1";
pub const GARDEN_CLOCK_OBSERVATION_INFO_ID: &str = "garden/clock-observation@1";
pub const GARDEN_CONTACT_OBSERVATION_INFO_ID: &str = "garden/contact-observation@1";
pub const GARDEN_MAXIMUM_STEPS: u16 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GardenState {
    pub vitality: Scalar,
    pub activity: Scalar,
    pub step: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GardenClockObservation {
    pub phase: Scalar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GardenContactObservation {
    pub intensity: Scalar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GardenEvolutionRefusal {
    MalformedState,
    MalformedClockObservation,
    MalformedContactObservation,
    MalformedEnrichedObservation,
    StepCapacityExceeded,
    ArithmeticOverflow,
}

pub fn evolve_garden_minimal(
    prior: GardenState,
    clock: GardenClockObservation,
) -> Result<GardenState, GardenEvolutionRefusal> {
    validate_state(prior)?;
    validate_scalar(clock.phase).map_err(|_| GardenEvolutionRefusal::MalformedClockObservation)?;
    let step = prior
        .step
        .checked_add(1)
        .filter(|step| *step <= GARDEN_MAXIMUM_STEPS)
        .ok_or(GardenEvolutionRefusal::StepCapacityExceeded)?;
    let previous = prior.vitality.raw_microunits();
    let delta = clock.phase.raw_microunits() - previous;
    let vitality = previous
        .checked_add(delta / 4)
        .ok_or(GardenEvolutionRefusal::ArithmeticOverflow)?;
    Ok(GardenState {
        vitality: Scalar::from_raw_microunits(vitality),
        activity: Scalar::from_raw_microunits(delta.unsigned_abs().min(Scalar::SCALE as u64) as i64),
        step,
    })
}

pub fn evolve_garden_enriched(
    prior: GardenState,
    clock: GardenClockObservation,
    contact: GardenContactObservation,
) -> Result<GardenState, GardenEvolutionRefusal> {
    validate_scalar(contact.intensity)
        .map_err(|_| GardenEvolutionRefusal::MalformedContactObservation)?;
    let mut next = evolve_garden_minimal(prior, clock)?;
    let boosted = next
        .vitality
        .raw_microunits()
        .checked_add(contact.intensity.raw_microunits() / 8)
        .ok_or(GardenEvolutionRefusal::ArithmeticOverflow)?
        .min(Scalar::SCALE);
    next.vitality = Scalar::from_raw_microunits(boosted);
    Ok(next)
}

pub fn evolve_garden_enriched_observation(
    prior: GardenState,
    observation: crate::GardenEnrichedObservation,
) -> Result<GardenState, GardenEvolutionRefusal> {
    evolve_garden_enriched(prior, observation.clock, observation.contact)
}

pub fn garden_state_value(
    state: GardenState,
) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    validate_state(state).map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)?;
    StructuredInfoValue::record(
        garden_state_type(),
        vec![
            scalar_field("activity", state.activity)?,
            count_field("step", u64::from(state.step))?,
            scalar_field("vitality", state.vitality)?,
        ],
    )
}

pub fn garden_clock_observation_value(
    observation: GardenClockObservation,
) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    validate_scalar(observation.phase)
        .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)?;
    StructuredInfoValue::record(
        garden_clock_observation_type(),
        vec![scalar_field("phase", observation.phase)?],
    )
}

pub fn decode_garden_state(encoded: &[u8]) -> Result<GardenState, GardenEvolutionRefusal> {
    let value = StructuredInfoValue::from_canonical_bytes(encoded)
        .map_err(|_| GardenEvolutionRefusal::MalformedState)?;
    if value.value_type() != &garden_state_type() {
        return Err(GardenEvolutionRefusal::MalformedState);
    }
    let state = GardenState {
        activity: scalar_record_field(&value, "activity")
            .map_err(|_| GardenEvolutionRefusal::MalformedState)?,
        step: u16::try_from(
            count_record_field(&value, "step")
                .map_err(|_| GardenEvolutionRefusal::MalformedState)?,
        )
        .map_err(|_| GardenEvolutionRefusal::MalformedState)?,
        vitality: scalar_record_field(&value, "vitality")
            .map_err(|_| GardenEvolutionRefusal::MalformedState)?,
    };
    validate_state(state)?;
    Ok(state)
}

pub fn decode_garden_clock_observation(
    encoded: &[u8],
) -> Result<GardenClockObservation, GardenEvolutionRefusal> {
    let value = StructuredInfoValue::from_canonical_bytes(encoded)
        .map_err(|_| GardenEvolutionRefusal::MalformedClockObservation)?;
    if value.value_type() != &garden_clock_observation_type() {
        return Err(GardenEvolutionRefusal::MalformedClockObservation);
    }
    let phase = scalar_record_field(&value, "phase")
        .map_err(|_| GardenEvolutionRefusal::MalformedClockObservation)?;
    validate_scalar(phase).map_err(|_| GardenEvolutionRefusal::MalformedClockObservation)?;
    Ok(GardenClockObservation { phase })
}

fn scalar_field(name: &str, value: Scalar) -> Result<StructuredFieldValue, StructuredInfoRefusal> {
    StructuredFieldValue::new(
        name,
        StructuredInfoValue::leaf(
            leaf(SCALAR_INFO_ID),
            value.raw_microunits().to_string().into_bytes(),
        )?,
    )
}

fn count_field(name: &str, value: u64) -> Result<StructuredFieldValue, StructuredInfoRefusal> {
    StructuredFieldValue::new(
        name,
        StructuredInfoValue::leaf(leaf("value/count@1"), value.to_string().into_bytes())?,
    )
}

fn scalar_record_field(value: &StructuredInfoValue, name: &str) -> Result<Scalar, ()> {
    let raw = record_leaf_text(value, name)?
        .parse::<i64>()
        .map_err(|_| ())?;
    Ok(Scalar::from_raw_microunits(raw))
}

fn count_record_field(value: &StructuredInfoValue, name: &str) -> Result<u64, ()> {
    record_leaf_text(value, name)?.parse().map_err(|_| ())
}

fn record_leaf_text<'a>(value: &'a StructuredInfoValue, name: &str) -> Result<&'a str, ()> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(());
    };
    let field = fields.iter().find(|field| field.name() == name).ok_or(())?;
    let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
        return Err(());
    };
    core::str::from_utf8(bytes).map_err(|_| ())
}

fn validate_state(state: GardenState) -> Result<(), GardenEvolutionRefusal> {
    if state.step > GARDEN_MAXIMUM_STEPS
        || validate_scalar(state.vitality).is_err()
        || validate_scalar(state.activity).is_err()
    {
        Err(GardenEvolutionRefusal::MalformedState)
    } else {
        Ok(())
    }
}

fn validate_scalar(value: Scalar) -> Result<(), ()> {
    if (0..=Scalar::SCALE).contains(&value.raw_microunits()) {
        Ok(())
    } else {
        Err(())
    }
}

pub fn garden_state_type() -> StructuredInfoType {
    record(
        GARDEN_STATE_INFO_ID,
        vec![
            field("activity", leaf(SCALAR_INFO_ID)),
            field("step", leaf("value/count@1")),
            field("vitality", leaf(SCALAR_INFO_ID)),
        ],
    )
}

pub fn garden_clock_observation_type() -> StructuredInfoType {
    record(
        GARDEN_CLOCK_OBSERVATION_INFO_ID,
        vec![field("phase", leaf(SCALAR_INFO_ID))],
    )
}

pub fn garden_contact_observation_type() -> StructuredInfoType {
    record(
        GARDEN_CONTACT_OBSERVATION_INFO_ID,
        vec![field("intensity", leaf(SCALAR_INFO_ID))],
    )
}

pub fn garden_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (GARDEN_STATE_TYPE, garden_state_type()),
        (
            GARDEN_CLOCK_OBSERVATION_TYPE,
            garden_clock_observation_type(),
        ),
        (
            GARDEN_CONTACT_OBSERVATION_TYPE,
            garden_contact_observation_type(),
        ),
        (
            crate::GARDEN_ENRICHED_OBSERVATION_TYPE,
            crate::garden_enriched_observation_type(),
        ),
    ]
}

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed Garden leaf")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed Garden field")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed Garden record")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(step: u16) -> GardenState {
        GardenState {
            vitality: Scalar::from_raw_microunits(400_000),
            activity: Scalar::ZERO,
            step,
        }
    }

    #[test]
    fn deterministic_sequence_reproduces_exact_minimal_and_enriched_state() {
        let clock = GardenClockObservation {
            phase: Scalar::from_raw_microunits(800_000),
        };
        let minimal = evolve_garden_minimal(state(0), clock).unwrap();
        assert_eq!(minimal.vitality.raw_microunits(), 500_000);
        assert_eq!(minimal.activity.raw_microunits(), 400_000);
        assert_eq!(minimal.step, 1);
        assert_eq!(evolve_garden_minimal(state(0), clock), Ok(minimal));

        let enriched = evolve_garden_enriched(
            state(0),
            clock,
            GardenContactObservation {
                intensity: Scalar::from_raw_microunits(800_000),
            },
        )
        .unwrap();
        assert_eq!(enriched.vitality.raw_microunits(), 600_000);
        assert_eq!(enriched.activity, minimal.activity);
        assert_eq!(enriched.step, minimal.step);
    }

    #[test]
    fn canonical_state_and_clock_values_round_trip_with_exact_types() {
        let prior = state(3);
        let clock = GardenClockObservation {
            phase: Scalar::from_raw_microunits(700_000),
        };
        let encoded_state = garden_state_value(prior)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let encoded_clock = garden_clock_observation_value(clock)
            .unwrap()
            .canonical_bytes()
            .unwrap();

        assert_eq!(decode_garden_state(&encoded_state), Ok(prior));
        assert_eq!(decode_garden_clock_observation(&encoded_clock), Ok(clock));
        assert_eq!(
            decode_garden_state(&encoded_clock),
            Err(GardenEvolutionRefusal::MalformedState)
        );
    }

    #[test]
    fn contact_and_composed_observations_round_trip_without_type_erasure() {
        let clock = GardenClockObservation {
            phase: Scalar::from_raw_microunits(700_000),
        };
        let contact = GardenContactObservation {
            intensity: Scalar::from_raw_microunits(300_000),
        };
        let observation = crate::combine_garden_observations(clock, contact).unwrap();
        let encoded_contact = crate::garden_contact_observation_value(contact)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let encoded_observation = crate::garden_enriched_observation_value(observation)
            .unwrap()
            .canonical_bytes()
            .unwrap();

        assert_eq!(
            crate::decode_garden_contact_observation(&encoded_contact),
            Ok(contact)
        );
        assert_eq!(
            crate::decode_garden_enriched_observation(&encoded_observation),
            Ok(observation)
        );
        assert_eq!(
            evolve_garden_enriched_observation(state(0), observation),
            evolve_garden_enriched(state(0), clock, contact)
        );
    }

    #[test]
    fn source_type_and_capacity_refusals_remain_distinct() {
        let clock = GardenClockObservation {
            phase: Scalar::from_raw_microunits(500_000),
        };
        assert_eq!(
            evolve_garden_minimal(state(GARDEN_MAXIMUM_STEPS), clock),
            Err(GardenEvolutionRefusal::StepCapacityExceeded)
        );
        assert_eq!(
            evolve_garden_minimal(
                state(0),
                GardenClockObservation {
                    phase: Scalar::from_raw_microunits(-1),
                },
            ),
            Err(GardenEvolutionRefusal::MalformedClockObservation)
        );
        assert_eq!(
            evolve_garden_enriched(
                state(0),
                clock,
                GardenContactObservation {
                    intensity: Scalar::from_raw_microunits(Scalar::SCALE + 1),
                },
            ),
            Err(GardenEvolutionRefusal::MalformedContactObservation)
        );
        let mut malformed = state(0);
        malformed.vitality = Scalar::from_raw_microunits(-1);
        assert_eq!(
            evolve_garden_minimal(malformed, clock),
            Err(GardenEvolutionRefusal::MalformedState)
        );
    }
}
