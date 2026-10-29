#![no_std]
//! Environmental contribution to the existing Body Face. Quantity units and
//! precision survive unchanged; a downstream Mask and Look own their display.
extern crate alloc;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, CheckedValueContract, Quantity, Unit, QUANTITY_ENCODED_LEN, RATIO_INFO_ID,
    TEMPERATURE_INFO_ID,
};
use conduit_face::{
    FaceContributionBasis, FaceFragment, FaceProperty, FacePropertyValue, FaceRole, FaceSubject,
};

pub const SUBJECT: &str = "environment";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SensorFailure {
    CaptureTimeout,
    ProviderFailed,
    GeometryRefused,
    ShortFrame,
    MalformedLength,
    Checksum,
    InvalidPayload,
}
impl SensorFailure {
    pub const fn label(self) -> &'static str {
        match self {
            Self::CaptureTimeout => "capture-timeout",
            Self::ProviderFailed => "provider-failed",
            Self::GeometryRefused => "geometry-refused",
            Self::ShortFrame => "short-frame",
            Self::MalformedLength => "malformed-length",
            Self::Checksum => "checksum",
            Self::InvalidPayload => "invalid-payload",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SensorStatus {
    Warming,
    Available,
    Unavailable(SensorFailure),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContributionRefusal {
    InvalidTemperature,
    InvalidHumidity,
    InvalidPressure,
    MissingTemperature,
    FreshWithoutObservation,
    InvalidFragment,
}

/// Construct one finite contribution. This allocates the ordinary Face objects
/// and belongs in admitted Face construction, not an allocation-sealed Back.
/// The Body composes and validates the caller's exact contribution provenance.
pub fn contribution(
    temperature: Option<Quantity>,
    relative_humidity: Option<Quantity>,
    fresh: bool,
    status: SensorStatus,
    basis: FaceContributionBasis,
) -> Result<FaceFragment, ContributionRefusal> {
    use ContributionRefusal as R;
    if status == SensorStatus::Available && temperature.is_none() {
        return Err(R::MissingTemperature);
    }
    if fresh && temperature.is_none() {
        return Err(R::FreshWithoutObservation);
    }
    let mut properties = Vec::with_capacity(5);
    for (name, kind, value, refusal) in [
        (
            "temperature",
            TEMPERATURE_INFO_ID,
            temperature,
            R::InvalidTemperature,
        ),
        (
            "relative-humidity",
            RATIO_INFO_ID,
            relative_humidity,
            R::InvalidHumidity,
        ),
    ] {
        if let Some(value) = value {
            let contract =
                CheckedValueContract::new(kind_id(kind), QUANTITY_ENCODED_LEN as u32, vec![])
                    .map_err(|_| refusal)?;
            contract.validate(&value.encode()).map_err(|_| refusal)?;
            if name == "relative-humidity"
                && (value
                    .compare(Quantity::new(0, Unit::Percent))
                    .map_err(|_| refusal)?
                    .is_lt()
                    || value
                        .compare(Quantity::new(100, Unit::Percent))
                        .map_err(|_| refusal)?
                        .is_gt())
            {
                return Err(refusal);
            }
            properties.push(property(
                name,
                FacePropertyValue::TypedValue {
                    contract,
                    bytes: value.encode().to_vec(),
                },
            ));
        }
    }
    properties.push(property("fresh", FacePropertyValue::Flag(fresh)));
    properties.push(property(
        "sensor-status",
        FacePropertyValue::Text(
            match status {
                SensorStatus::Warming => "warming",
                SensorStatus::Available => "available",
                SensorStatus::Unavailable(_) => "unavailable",
            }
            .into(),
        ),
    ));
    if let SensorStatus::Unavailable(reason) = status {
        properties.push(property(
            "sensor-failure",
            FacePropertyValue::Text(reason.label().into()),
        ));
    }
    let fragment = FaceFragment {
        basis,
        subjects: vec![FaceSubject {
            identity: SUBJECT.into(),
            role: FaceRole::Info,
            name: "Room conditions".into(),
        }],
        relationships: vec![],
        composition: vec![],
        properties,
        text: vec![],
        actions: vec![],
        disclosures: vec![],
        temporal_references: vec![],
        temporal_facts: vec![],
    };
    fragment.validate_bounds().map_err(|_| R::InvalidFragment)?;
    Ok(fragment)
}
fn property(name: &str, value: FacePropertyValue) -> FaceProperty {
    FaceProperty {
        subject: SUBJECT.into(),
        name: name.into(),
        value,
    }
}

mod prepared;
mod result;
pub use prepared::{PreparedObservationFace, PublicationRefusal};

pub use result::{decode_result as decode_publication_result, DecodedPublication};

/// Common successful-reading model; these values convey no admission authority.
pub use conduit_environment_measurement::{EnvironmentalReading, ReadingField, ReadingRefusal};

/// Contribute one common reading through the ordinary Face/provenance boundary.
/// Unsupported properties are absent, with explicit support labels. A failed
/// current read may retain a prior reading with `fresh=false` and Unavailable
/// status; unsupported never substitutes for a failure. This allocates ordinary
/// Face objects and requires independently admitted construction workspace.
pub fn contribution_reading(
    reading: EnvironmentalReading,
    fresh: bool,
    status: SensorStatus,
    basis: FaceContributionBasis,
) -> Result<FaceFragment, ContributionRefusal> {
    use alloc::boxed::Box;
    use conduit_core::{IntervalEndpoint, ValueConstraint, QUANTITY_INFO_ID};
    let humidity = match reading.relative_humidity() {
        ReadingField::Available(value) => Some(value),
        ReadingField::Unsupported => None,
    };
    let mut fragment = contribution(Some(reading.temperature()), humidity, fresh, status, basis)?;
    for (name, field) in [
        ("relative-humidity-support", reading.relative_humidity()),
        (
            "atmospheric-pressure-support",
            reading.atmospheric_pressure(),
        ),
    ] {
        fragment.properties.push(property(
            name,
            FacePropertyValue::Text(
                match field {
                    ReadingField::Available(_) => "supported",
                    ReadingField::Unsupported => "unsupported",
                }
                .into(),
            ),
        ));
    }
    if let ReadingField::Available(value) = reading.atmospheric_pressure() {
        // A Pascal endpoint checks the exact pressure family/role as well as
        // positivity; generic Quantity Info alone would accept other families.
        let contract = CheckedValueContract::new(
            kind_id(QUANTITY_INFO_ID),
            QUANTITY_ENCODED_LEN as u32,
            vec![ValueConstraint::QuantityRange {
                minimum: Some(Box::new(Quantity::new(0, Unit::Pascal))),
                maximum: None,
                minimum_endpoint: IntervalEndpoint::Exclusive,
                maximum_endpoint: IntervalEndpoint::Inclusive,
            }],
        )
        .map_err(|_| ContributionRefusal::InvalidPressure)?;
        contract
            .validate(&value.encode())
            .map_err(|_| ContributionRefusal::InvalidPressure)?;
        fragment.properties.push(property(
            "atmospheric-pressure",
            FacePropertyValue::TypedValue {
                contract,
                bytes: value.encode().to_vec(),
            },
        ));
    }
    fragment
        .validate_bounds()
        .map_err(|_| ContributionRefusal::InvalidFragment)?;
    Ok(fragment)
}
