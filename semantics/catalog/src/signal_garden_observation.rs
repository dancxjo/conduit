//! Exact structured values for reusable Signal Garden observation composition.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoValue, StructuredInfoValueShape,
};

use crate::{
    garden_clock_observation_type, garden_contact_observation_type, GardenClockObservation,
    GardenContactObservation, GardenEvolutionRefusal,
};

pub const GARDEN_ENRICHED_OBSERVATION_TYPE: &str = "GardenEnrichedObservation";
pub const GARDEN_ENRICHED_OBSERVATION_INFO_ID: &str = "garden/enriched-observation@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GardenEnrichedObservation {
    pub clock: GardenClockObservation,
    pub contact: GardenContactObservation,
}

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
    let intensity = observation.intensity.raw_microunits();
    if !(0..=conduit_core::Scalar::SCALE).contains(&intensity) {
        return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
    }
    StructuredInfoValue::record(
        garden_contact_observation_type(),
        vec![leaf_field(
            "intensity",
            "value/scalar@1",
            intensity.to_string().into_bytes(),
        )?],
    )
}

pub fn garden_enriched_observation_value(
    observation: GardenEnrichedObservation,
) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    StructuredInfoValue::record(
        garden_enriched_observation_type(),
        vec![
            StructuredFieldValue::new(
                "clock",
                crate::garden_clock_observation_value(observation.clock)?,
            )?,
            StructuredFieldValue::new(
                "contact",
                garden_contact_observation_value(observation.contact)?,
            )?,
        ],
    )
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
    if value.value_type() != &garden_enriched_observation_type() {
        return Err(GardenEvolutionRefusal::MalformedEnrichedObservation);
    }
    Ok(GardenEnrichedObservation {
        clock: decode_clock_value(
            record_value_field(&value, "clock")
                .map_err(|_| GardenEvolutionRefusal::MalformedClockObservation)?,
        )?,
        contact: decode_contact_value(
            record_value_field(&value, "contact")
                .map_err(|_| GardenEvolutionRefusal::MalformedContactObservation)?,
        )?,
    })
}

pub fn garden_enriched_observation_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id(GARDEN_ENRICHED_OBSERVATION_INFO_ID),
        vec![
            field("clock", garden_clock_observation_type()),
            field("contact", garden_contact_observation_type()),
        ],
    )
    .expect("reviewed Garden enriched observation record")
}

fn decode_clock_value(
    value: &StructuredInfoValue,
) -> Result<GardenClockObservation, GardenEvolutionRefusal> {
    let encoded = value
        .canonical_bytes()
        .map_err(|_| GardenEvolutionRefusal::MalformedClockObservation)?;
    crate::decode_garden_clock_observation(&encoded)
}

fn decode_contact_value(
    value: &StructuredInfoValue,
) -> Result<GardenContactObservation, GardenEvolutionRefusal> {
    if value.value_type() != &garden_contact_observation_type() {
        return Err(GardenEvolutionRefusal::MalformedContactObservation);
    }
    let raw = record_leaf_text(value, "intensity")
        .and_then(|text| text.parse::<i64>().map_err(|_| ()))
        .map_err(|_| GardenEvolutionRefusal::MalformedContactObservation)?;
    if !(0..=conduit_core::Scalar::SCALE).contains(&raw) {
        return Err(GardenEvolutionRefusal::MalformedContactObservation);
    }
    Ok(GardenContactObservation {
        intensity: conduit_core::Scalar::from_raw_microunits(raw),
    })
}

fn record_value_field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, ()> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(());
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(())
}

fn record_leaf_text<'a>(value: &'a StructuredInfoValue, name: &str) -> Result<&'a str, ()> {
    let value = record_value_field(value, name)?;
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(());
    };
    core::str::from_utf8(bytes).map_err(|_| ())
}

fn leaf_field(
    name: &str,
    kind: &str,
    bytes: Vec<u8>,
) -> Result<StructuredFieldValue, StructuredInfoRefusal> {
    StructuredFieldValue::new(
        name,
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id(kind)).expect("reviewed Garden leaf"),
            bytes,
        )?,
    )
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed Garden observation field")
}
