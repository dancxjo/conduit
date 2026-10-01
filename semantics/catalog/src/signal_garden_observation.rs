//! Exact structured values for reusable Signal Garden observation composition.

use conduit_core::{StructuredInfoRefusal, StructuredInfoType, StructuredInfoValue};
use conduit_form::rust_binding::NativeRustBinding;

use crate::{GardenClockObservation, GardenContactObservation, GardenEvolutionRefusal};
pub use conduit_alife::GardenEnrichedObservation;

pub const GARDEN_ENRICHED_OBSERVATION_TYPE: &str = "GardenEnrichedObservation";
pub const GARDEN_ENRICHED_OBSERVATION_INFO_ID: &str = "garden/enriched-observation@1";

pub fn combine_garden_observations(
    clock: GardenClockObservation,
    contact: GardenContactObservation,
) -> Result<GardenEnrichedObservation, GardenEvolutionRefusal> {
    let clock = crate::garden_clock_observation_value(clock)
        .map_err(|_| GardenEvolutionRefusal::MalformedClockObservation)?;
    let contact = garden_contact_observation_value(contact)
        .map_err(|_| GardenEvolutionRefusal::MalformedContactObservation)?;
    Ok(GardenEnrichedObservation {
        clock: decode_clock_value(&clock)?,
        contact: decode_contact_value(&contact)?,
    })
}

pub fn garden_contact_observation_value(
    observation: GardenContactObservation,
) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    let intensity = observation.intensity;
    if !(0..=conduit_core::Scalar::SCALE).contains(&intensity.raw_microunits()) {
        return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
    }
    observation
        .into_structured()
        .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)
}

pub fn garden_enriched_observation_value(
    observation: GardenEnrichedObservation,
) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    observation
        .into_structured()
        .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)
}

pub fn decode_garden_contact_observation(
    encoded: &[u8],
) -> Result<GardenContactObservation, GardenEvolutionRefusal> {
    let value = StructuredInfoValue::from_canonical_bytes(encoded)
        .map_err(|_| GardenEvolutionRefusal::MalformedContactObservation)?;
    decode_contact_value(&value)
}

pub fn decode_garden_enriched_observation(
    encoded: &[u8],
) -> Result<GardenEnrichedObservation, GardenEvolutionRefusal> {
    let value = StructuredInfoValue::from_canonical_bytes(encoded)
        .map_err(|_| GardenEvolutionRefusal::MalformedEnrichedObservation)?;
    GardenEnrichedObservation::from_structured(value)
        .map_err(|_| GardenEvolutionRefusal::MalformedEnrichedObservation)
}

pub fn garden_enriched_observation_type() -> StructuredInfoType {
    GardenEnrichedObservation::semantic_type().expect("checked Garden enriched observation Type")
}

fn decode_clock_value(
    value: &StructuredInfoValue,
) -> Result<GardenClockObservation, GardenEvolutionRefusal> {
    GardenClockObservation::from_structured(value.clone())
        .map_err(|_| GardenEvolutionRefusal::MalformedClockObservation)
}

fn decode_contact_value(
    value: &StructuredInfoValue,
) -> Result<GardenContactObservation, GardenEvolutionRefusal> {
    GardenContactObservation::from_structured(value.clone())
        .map_err(|_| GardenEvolutionRefusal::MalformedContactObservation)
}
