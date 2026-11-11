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
