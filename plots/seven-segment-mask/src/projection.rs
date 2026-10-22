//! Borrowed, prepared view of existing Face properties, not another Face owner.
use conduit_core::{Quantity, QuantityRole, Unit};
use conduit_face::{Face, FacePropertyValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceProjectionRefusal {
    InvalidFace,
    TooLarge,
    MissingProperty,
    DuplicateProperty,
    WrongPropertyType,
    InvalidQuantity,
}

pub struct PreparedEnvironmentFace<'a> {
    pub(crate) content_id: &'a str,
    pub(crate) revision: u64,
    pub(crate) temperature: Option<Quantity>,
    pub(crate) humidity: Option<Quantity>,
    pub(crate) available: bool,
    pub(crate) fresh: bool,
    pub(crate) minute_of_day: Option<u16>,
    pub(crate) clock_fresh: bool,
}

impl<'a> PreparedEnvironmentFace<'a> {
    /// Preparation checks the canonical Face identity and finite projection.
    /// Call once per revision; a timed Mask can reuse this immutable view.
    pub fn prepare(face: &'a Face, subject: &str) -> Result<Self, FaceProjectionRefusal> {
        use FaceProjectionRefusal as R;
        if face.subjects.len() > 4 || face.properties.len() > 16 {
            return Err(R::TooLarge);
        }
        face.validate().map_err(|_| R::InvalidFace)?;
        if !face.subjects.iter().any(|s| s.identity == subject) {
            return Err(R::MissingProperty);
        }
        let property = |name: &str| {
            let mut matches = face
                .properties
                .iter()
                .filter(|p| p.subject == subject && p.name == name);
            let value = matches.next().map(|p| &p.value);
            if matches.next().is_some() {
                Err(R::DuplicateProperty)
            } else {
                Ok(value)
            }
        };
        let flag = |name| match property(name)? {
            Some(FacePropertyValue::Flag(value)) => Ok(*value),
            None => Err(R::MissingProperty),
            _ => Err(R::WrongPropertyType),
        };
        let quantity = |name: &str, target: Unit, role: QuantityRole| {
            let Some(value) = property(name)? else {
                return Ok(None);
            };
            let FacePropertyValue::TypedValue { contract, bytes } = value else {
                return Err(R::WrongPropertyType);
            };
            contract.validate(bytes).map_err(|_| R::InvalidQuantity)?;
            let value = Quantity::decode(bytes).map_err(|_| R::InvalidQuantity)?;
            if value.family() != target.family() || value.role() != role {
                return Err(R::InvalidQuantity);
            }
            Ok(Some(value))
        };
        let available = match property("sensor-status")? {
            Some(FacePropertyValue::Text(text)) if text == "available" => true,
            Some(FacePropertyValue::Text(text))
                if matches!(text.as_str(), "unavailable" | "warming") =>
            {
                false
            }
            None => return Err(R::MissingProperty),
            _ => return Err(R::WrongPropertyType),
        };
        let humidity = quantity("relative-humidity", Unit::Percent, QuantityRole::Linear)?;
        if let Some(value) = humidity {
            if value
                .compare(Quantity::new(0, Unit::Percent))
                .map_err(|_| R::InvalidQuantity)?
                .is_lt()
                || value
                    .compare(Quantity::new(100, Unit::Percent))
                    .map_err(|_| R::InvalidQuantity)?
                    .is_gt()
            {
                return Err(R::InvalidQuantity);
            }
        }
        let minute_of_day = match property("minute-of-day")? {
            Some(FacePropertyValue::Count(value)) if *value < 1440 => Some(*value as u16),
            None => None,
            _ => return Err(R::WrongPropertyType),
        };
        Ok(Self {
            content_id: face.identity.as_str(),
            revision: face.revision,
            temperature: quantity("temperature", Unit::Celsius, QuantityRole::Point)?,
            humidity,
            available,
            fresh: flag("fresh")?,
            minute_of_day,
            clock_fresh: if minute_of_day.is_some() {
                flag("clock-fresh")?
            } else {
                false
            },
        })
    }
}
