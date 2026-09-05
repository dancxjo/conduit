//! Finite, deterministic observation-driven state for Signal Garden compositions.

use alloc::{vec, vec::Vec};
use conduit_core::{kind_id, Scalar, StructuredFieldType, StructuredInfoType, SCALAR_INFO_ID};

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
