//! Finite, deterministic observation-driven state for Signal Garden compositions.

use alloc::{vec, vec::Vec};
pub use conduit_alife::{
    GardenClockObservation, GardenContactObservation, GardenEvolutionRefusal, GardenState,
};
use conduit_core::{Scalar, StructuredInfoRefusal, StructuredInfoType, StructuredInfoValue};
use conduit_plot::rust_binding::NativeRustBinding;

pub const GARDEN_STATE_TYPE: &str = "GardenState";
pub const GARDEN_CLOCK_OBSERVATION_TYPE: &str = "GardenClockObservation";
pub const GARDEN_CONTACT_OBSERVATION_TYPE: &str = "GardenContactObservation";
pub const GARDEN_STATE_INFO_ID: &str = "garden/state@1";
pub const GARDEN_CLOCK_OBSERVATION_INFO_ID: &str = "garden/clock-observation@1";
pub const GARDEN_CONTACT_OBSERVATION_INFO_ID: &str = "garden/contact-observation@1";
pub const GARDEN_MAXIMUM_STEPS: u16 = 16;

pub fn deterministic_garden_observations() -> (
    GardenState,
    GardenClockObservation,
    GardenContactObservation,
) {
    (
        GardenState {
            vitality: Scalar::from_decimal_parts(4, -1).expect("exact initial vitality"),
            activity: Scalar::ZERO,
            step: 0,
        },
        GardenClockObservation {
            phase: Scalar::from_decimal_parts(8, -1).expect("exact initial observation"),
        },
        GardenContactObservation {
            intensity: Scalar::from_decimal_parts(8, -1).expect("exact initial observation"),
        },
    )
}

pub fn evolve_garden_minimal(
    prior: GardenState,
    clock: GardenClockObservation,
) -> Result<GardenState, GardenEvolutionRefusal> {
    validate_state(&prior)?;
    validate_scalar(&clock.phase).map_err(|_| GardenEvolutionRefusal::MalformedClockObservation)?;
    let step = prior
        .step
        .checked_add(1)
        .filter(|step| *step <= GARDEN_MAXIMUM_STEPS)
        .ok_or(GardenEvolutionRefusal::StepCapacityExceeded)?;
    let numeric = |_| GardenEvolutionRefusal::ArithmeticOverflow;
    let delta = clock.phase.checked_sub(&prior.vitality).map_err(numeric)?;
    let vitality = prior.vitality.checked_add(&delta.checked_div(&Scalar::integer(4)).map_err(numeric)?).map_err(numeric)?;
    let activity = if delta.compare(&Scalar::ZERO).map_err(numeric)? == Some(core::cmp::Ordering::Less) { delta.checked_neg().map_err(numeric)? } else { delta };
    Ok(GardenState { vitality, activity, step })
}

pub fn evolve_garden_enriched(
    prior: GardenState,
    clock: GardenClockObservation,
    contact: GardenContactObservation,
) -> Result<GardenState, GardenEvolutionRefusal> {
    validate_scalar(&contact.intensity)
        .map_err(|_| GardenEvolutionRefusal::MalformedContactObservation)?;
    let mut next = evolve_garden_minimal(prior, clock)?;
    let numeric = |_| GardenEvolutionRefusal::ArithmeticOverflow;
    let boosted = next.vitality.checked_add(&contact.intensity.checked_div(&Scalar::integer(8)).map_err(numeric)?).map_err(numeric)?;
    next.vitality = if boosted.compare(&Scalar::ONE).map_err(numeric)? == Some(core::cmp::Ordering::Greater) { Scalar::ONE } else { boosted };
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
    validate_state(&state).map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)?;
    state
        .into_structured()
        .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)
}

pub fn garden_clock_observation_value(
    observation: GardenClockObservation,
) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    validate_scalar(&observation.phase)
        .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)?;
    observation
        .into_structured()
        .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)
}

pub fn decode_garden_state(encoded: &[u8]) -> Result<GardenState, GardenEvolutionRefusal> {
    let value = StructuredInfoValue::from_canonical_bytes(encoded)
        .map_err(|_| GardenEvolutionRefusal::MalformedState)?;
    let state =
        GardenState::from_structured(value).map_err(|_| GardenEvolutionRefusal::MalformedState)?;
    validate_state(&state)?;
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
    let observation = GardenClockObservation::from_structured(value)
        .map_err(|_| GardenEvolutionRefusal::MalformedClockObservation)?;
    let phase = &observation.phase;
    validate_scalar(phase).map_err(|_| GardenEvolutionRefusal::MalformedClockObservation)?;
    Ok(observation)
}

fn validate_state(state: &GardenState) -> Result<(), GardenEvolutionRefusal> {
    if state.step > GARDEN_MAXIMUM_STEPS
        || validate_scalar(&state.vitality).is_err()
        || validate_scalar(&state.activity).is_err()
    {
        Err(GardenEvolutionRefusal::MalformedState)
    } else {
        Ok(())
    }
}

pub(super) fn validate_scalar(value: &Scalar) -> Result<(), ()> {
    let lower = value.compare(&Scalar::ZERO).map_err(|_| ())?.ok_or(())?;
    let upper = value.compare(&Scalar::ONE).map_err(|_| ())?.ok_or(())?;
    if lower != core::cmp::Ordering::Less && upper != core::cmp::Ordering::Greater { Ok(()) } else { Err(()) }
}

pub fn garden_state_type() -> StructuredInfoType {
    GardenState::semantic_type().expect("checked Garden state Type")
}

pub fn garden_clock_observation_type() -> StructuredInfoType {
    GardenClockObservation::semantic_type().expect("checked Garden clock observation Type")
}

pub fn garden_contact_observation_type() -> StructuredInfoType {
    GardenContactObservation::semantic_type().expect("checked Garden contact observation Type")
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

#[cfg(test)]
mod tests {
    use super::*;
    fn micros(value: &Scalar) -> i64 { conduit_core::MathValueRef::validate(&value.canonical_bytes().unwrap()).unwrap().checked_microscalar().unwrap().raw_microunits() }

    fn state(step: u16) -> GardenState {
        GardenState {
            vitality: Scalar::from_decimal_parts(400_000, -6).unwrap(),
            activity: Scalar::ZERO,
            step,
        }
    }

    #[test]
    fn deterministic_sequence_reproduces_exact_minimal_and_enriched_state() {
        let clock = GardenClockObservation {
            phase: Scalar::from_decimal_parts(800_000, -6).unwrap(),
        };
        let minimal = evolve_garden_minimal(state(0), clock.clone()).unwrap();
        assert_eq!(micros(&minimal.vitality), 500_000);
        assert_eq!(micros(&minimal.activity), 400_000);
        assert_eq!(minimal.step, 1);
        assert_eq!(evolve_garden_minimal(state(0), clock.clone()), Ok(minimal.clone()));

        let enriched = evolve_garden_enriched(
            state(0),
            clock.clone(),
            GardenContactObservation {
                intensity: Scalar::from_decimal_parts(800_000, -6).unwrap(),
            },
        )
        .unwrap();
        assert_eq!(micros(&enriched.vitality), 600_000);
        assert_eq!(enriched.activity, minimal.activity);
        assert_eq!(enriched.step, minimal.step);
    }

    #[test]
    fn canonical_state_and_clock_values_round_trip_with_exact_types() {
        let prior = state(3);
        let clock = GardenClockObservation {
            phase: Scalar::from_decimal_parts(700_000, -6).unwrap(),
        };
        let encoded_state = garden_state_value(prior.clone())
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let encoded_clock = garden_clock_observation_value(clock.clone())
            .unwrap()
            .canonical_bytes()
            .unwrap();

        assert_eq!(decode_garden_state(&encoded_state), Ok(prior.clone()));
        assert_eq!(decode_garden_clock_observation(&encoded_clock), Ok(clock.clone()));
        assert_eq!(
            decode_garden_state(&encoded_clock),
            Err(GardenEvolutionRefusal::MalformedState)
        );
    }

    #[test]
    fn deterministic_observations_preserve_exact_types_and_enriched_result() {
        let (prior, clock, contact) = deterministic_garden_observations();
        let prior_bytes = garden_state_value(prior.clone())
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let clock_bytes = garden_clock_observation_value(clock.clone())
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let contact_bytes = crate::garden_contact_observation_value(contact.clone())
            .unwrap()
            .canonical_bytes()
            .unwrap();

        assert_eq!(decode_garden_state(&prior_bytes), Ok(prior.clone()));
        assert_eq!(decode_garden_clock_observation(&clock_bytes), Ok(clock.clone()));
        assert_eq!(
            crate::decode_garden_contact_observation(&contact_bytes),
            Ok(contact.clone())
        );
        let next = evolve_garden_enriched(prior.clone(), clock.clone(), contact.clone()).unwrap();
        assert_eq!(micros(&next.vitality), 600_000);
        assert_eq!(micros(&next.activity), 400_000);
        assert_eq!(next.step, 1);
    }

    #[test]
    fn contact_and_composed_observations_round_trip_without_type_erasure() {
        let clock = GardenClockObservation {
            phase: Scalar::from_decimal_parts(700_000, -6).unwrap(),
        };
        let contact = GardenContactObservation {
            intensity: Scalar::from_decimal_parts(300_000, -6).unwrap(),
        };
        let observation = crate::combine_garden_observations(clock.clone(), contact.clone()).unwrap();
        let encoded_contact = crate::garden_contact_observation_value(contact.clone())
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let encoded_observation = crate::garden_enriched_observation_value(observation.clone())
            .unwrap()
            .canonical_bytes()
            .unwrap();

        assert_eq!(
            crate::decode_garden_contact_observation(&encoded_contact),
            Ok(contact.clone())
        );
        assert_eq!(
            crate::decode_garden_enriched_observation(&encoded_observation),
            Ok(observation.clone())
        );
        assert_eq!(
            evolve_garden_enriched_observation(state(0), observation.clone()),
            evolve_garden_enriched(state(0), clock.clone(), contact.clone())
        );
    }

    #[test]
    fn source_type_and_capacity_refusals_remain_distinct() {
        let clock = GardenClockObservation {
            phase: Scalar::from_decimal_parts(500_000, -6).unwrap(),
        };
        assert_eq!(
            evolve_garden_minimal(state(GARDEN_MAXIMUM_STEPS), clock.clone()),
            Err(GardenEvolutionRefusal::StepCapacityExceeded)
        );
        assert_eq!(
            evolve_garden_minimal(
                state(0),
                GardenClockObservation {
                    phase: Scalar::from_decimal_parts(-1, -6).unwrap(),
                },
            ),
            Err(GardenEvolutionRefusal::MalformedClockObservation)
        );
        assert_eq!(
            evolve_garden_enriched(
                state(0),
                clock.clone(),
                GardenContactObservation {
                    intensity: Scalar::from_decimal_parts(1_000_000 + 1, -6).unwrap(),
                },
            ),
            Err(GardenEvolutionRefusal::MalformedContactObservation)
        );
        let mut malformed = state(0);
        malformed.vitality = Scalar::from_decimal_parts(-1, -6).unwrap();
        assert_eq!(
            evolve_garden_minimal(malformed, clock.clone()),
            Err(GardenEvolutionRefusal::MalformedState)
        );
    }
}
