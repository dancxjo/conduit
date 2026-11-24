//! Explicit content presentation, separate from redacted structured inspection.
use alloc::{format, string::String, vec::Vec};
use conduit_core::{
    kind_id, BoundedResourceRef, CheckedValueContract, Quantity, StructuredInfoValue,
    StructuredInfoValueShape, Unit,
};
use conduit_face::{
    FaceContributionBasis, FaceDisclosure, FaceDisclosureLevel, FaceFragment, FaceProperty,
    FacePropertyValue, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject, FaceText,
};
use conduit_plot::rust_binding::semantic_core as conduit_core;
use conduit_plot::{rust_binding, CheckedNativeType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbientFaceRefusal {
    TypePreparation,
    WrongType,
    InvalidObservation,
    InvalidBasis,
    ImageValueTooLarge,
    FragmentBounds,
}

/// Retains exact checked contracts once before repeated Face preparation.
/// Methods still allocate bounded output; they are not kernel Step operations.
pub struct PreparedAmbientFaces {
    pub(crate) wallpaper: CheckedNativeType,
    pub(crate) clock_weather: CheckedNativeType,
    pub(crate) home: CheckedNativeType,
    pub(crate) civil: CheckedNativeType,
    pub(crate) wallpaper_observation: CheckedNativeType,
    pub(crate) clock_validator: conduit_core::PreparedStructuredValueValidator,
    pub(crate) home_validator: conduit_core::PreparedStructuredValueValidator,
}

impl PreparedAmbientFaces {
    pub fn prepare() -> Result<Self, AmbientFaceRefusal> {
        let checked = crate::prepare_ambient_display_types()
            .map_err(|_| AmbientFaceRefusal::TypePreparation)?;
        let named = |name: &str| {
            checked
                .native_types
                .iter()
                .find(|ty| ty.name == name)
                .cloned()
                .ok_or(AmbientFaceRefusal::TypePreparation)
        };
        let home = named("AmbientHomeObservation")?;
        let home_validator = conduit_core::PreparedStructuredValueValidator::new(
            &home.value_type,
            crate::AMBIENT_HOME_MAXIMUM_BYTES as usize,
        )
        .map_err(|_| AmbientFaceRefusal::TypePreparation)?;
        let clock_weather = named("AmbientClockWeather")?;
        let clock_validator =
            conduit_core::PreparedStructuredValueValidator::new(&clock_weather.value_type, 4096)
                .map_err(|_| AmbientFaceRefusal::TypePreparation)?;
        Ok(Self {
            home_validator,
            clock_validator,
            wallpaper: named("AmbientWallpaperSelection")?,
            clock_weather,
            home,
            civil: crate::civil::prepare()?,
            wallpaper_observation: named("AmbientWallpaperObservation")?,
        })
    }

    pub fn wallpaper_type(&self) -> &CheckedNativeType {
        &self.wallpaper
    }

    pub fn wallpaper_observation_type(&self) -> &CheckedNativeType {
        &self.wallpaper_observation
    }

    pub(super) fn unavailable_wallpaper(
        &self,
        basis: FaceContributionBasis,
        reason: String,
    ) -> Result<FaceFragment, AmbientFaceRefusal> {
        let mut output = Builder::new(basis, "wallpaper", "Wallpaper")?;
        let image = output.child("image", "Wallpaper unavailable", "media/image")?;
        output.wording(&image, format!("Wallpaper unavailable: {reason}."));
        output.fact(
            &image,
            "availability",
            FacePropertyValue::Text("unavailable".into()),
        );
        output.fact(
            &image,
            "unavailable-reason",
            FacePropertyValue::Text(reason),
        );
        output.finish()
    }
    pub fn clock_canonical_observation(
        &self,
        basis: FaceContributionBasis,
        encoded: &[u8],
    ) -> Result<FaceFragment, AmbientFaceRefusal> {
        self.clock_validator
            .validate(encoded)
            .map_err(|_| AmbientFaceRefusal::InvalidObservation)?;
        let observation = StructuredInfoValue::from_canonical_bytes(encoded)
            .map_err(|_| AmbientFaceRefusal::InvalidObservation)?;
        self.clock_weather(basis, &observation)
    }

    pub fn clock_weather_type(&self) -> &CheckedNativeType {
        &self.clock_weather
    }
    pub fn home_type(&self) -> &CheckedNativeType {
        &self.home
    }

    pub fn wallpaper(
        &self,
        basis: FaceContributionBasis,
        observation: &StructuredInfoValue,
    ) -> Result<FaceFragment, AmbientFaceRefusal> {
        validate(&self.wallpaper, observation)?;
        let mut output = Builder::new(basis, "wallpaper", "Wallpaper")?;
        let metadata = field(observation, "image")?;
        let image = field(metadata, "image")?;
        let title = text(metadata, "title")?;
        let caption = text(metadata, "caption")?;
        let attribution = text(metadata, "attribution")?;
        let license = text(metadata, "license")?;
        let source = text(metadata, "source")?;
        let seconds = unsigned(observation, "rotation_seconds")?;
        let subject = output.child("image", &title, "media/image")?;
        if !caption.is_empty() {
            output.wording(&subject, caption);
        }
        output.wording(
            &subject,
            format!("Photograph: {attribution}. License: {license}. Source: {source}."),
        );
        let cadence = if seconds % 60 == 0 {
            format!("Rotation interval: {} minutes.", seconds / 60)
        } else {
            format!("Rotation interval: {seconds} seconds.")
        };
        output.wording(&subject, cadence);
        output.fact(&subject, "title", FacePropertyValue::Text(title));
        if !text(metadata, "caption")?.is_empty() {
            output.fact(
                &subject,
                "caption",
                FacePropertyValue::Text(text(metadata, "caption")?),
            );
        }
        output.fact(
            &subject,
            "attribution",
            FacePropertyValue::Text(attribution),
        );
        output.fact(&subject, "source", FacePropertyValue::Text(source));
        output.fact(&subject, "license", FacePropertyValue::Text(license));
        output.fact(
            &subject,
            "rotation-seconds",
            FacePropertyValue::Count(seconds),
        );
        for (input, property) in [
            ("selected_at_unix_ms", "selected-at-unix-ms"),
            ("next_rotation_unix_ms", "next-rotation-unix-ms"),
        ] {
            output.fact(
                &subject,
                property,
                FacePropertyValue::Count(unsigned(observation, input)?),
            );
        }
        output.fact(
            &subject,
            "clock-basis",
            FacePropertyValue::Identity("time/unix-utc@1".into()),
        );
        let content = match field(image, "content")?.shape() {
            StructuredInfoValueShape::Leaf(bytes) => bytes.to_vec(),
            _ => return Err(AmbientFaceRefusal::InvalidObservation),
        };
        BoundedResourceRef::validate_encoded(&content)
            .map_err(|_| AmbientFaceRefusal::InvalidObservation)?;
        output.fact(
            &subject,
            "image-content",
            FacePropertyValue::Content(content),
        );
        let bytes = image
            .canonical_bytes()
            .map_err(|_| AmbientFaceRefusal::InvalidObservation)?;
        if bytes.len() > conduit_face::MAX_FACE_VALUE_BYTES as usize {
            return Err(AmbientFaceRefusal::ImageValueTooLarge);
        }
        let profile = image
            .value_type()
            .profile()
            .map_err(|_| AmbientFaceRefusal::InvalidObservation)?;
        let contract = CheckedValueContract::new(
            profile.value_kind().clone(),
            conduit_face::MAX_FACE_VALUE_BYTES,
            Vec::new(),
        )
        .map_err(|_| AmbientFaceRefusal::InvalidObservation)?;
        contract
            .validate(&bytes)
            .map_err(|_| AmbientFaceRefusal::InvalidObservation)?;
        output.fact(
            &subject,
            "image",
            FacePropertyValue::TypedValue { contract, bytes },
        );
        output.finish()
    }

    pub fn clock_weather(
        &self,
        basis: FaceContributionBasis,
        observation: &StructuredInfoValue,
    ) -> Result<FaceFragment, AmbientFaceRefusal> {
        validate(&self.clock_weather, observation)?;
        let mut output = Builder::new(basis, "clock-weather", "Date, time and weather")?;
        let clock = output.child("clock", "Date and time", "time/civil-observation")?;
        let date = text(observation, "local_date")?;
        let time = text(observation, "local_time")?;
        output.fact(
            &clock,
            "local-date-time",
            crate::civil::property(&self.civil, &date, &time)?,
        );
        let zone = text(observation, "timezone")?;
        let offset = signed(observation, "utc_offset_minutes")?;
        let magnitude = offset.unsigned_abs();
        output.wording(
            &clock,
            format!(
                "{date} · {time} · {zone} (UTC{}{:02}:{:02})",
                if offset < 0 { "−" } else { "+" },
                magnitude / 60,
                magnitude % 60
            ),
        );
        for (name, value) in [
            ("local-date", date),
            ("local-time", time),
            ("timezone", zone),
        ] {
            output.fact(&clock, name, FacePropertyValue::Text(value));
        }
        output.fact(
            &clock,
            "utc-offset-minutes",
            FacePropertyValue::Signed(offset),
        );
        output.fact(
            &clock,
            "observed-at-unix-ms",
            FacePropertyValue::Count(unsigned(observation, "observed_at_unix_ms")?),
        );
        output.fact(
            &clock,
            "clock-basis",
            FacePropertyValue::Identity("time/unix-utc@1".into()),
        );
        output.fact(
            &clock,
            "timezone-rule-set",
            FacePropertyValue::Text(text(observation, "timezone_rule_set")?),
        );
        let weather = output.child("weather", "Weather", "weather/conditions")?;
        let StructuredInfoValueShape::Variant { tag, payload } =
            field(observation, "weather")?.shape()
        else {
            return Err(AmbientFaceRefusal::InvalidObservation);
        };
        output.fact(
            &weather,
            "availability",
            FacePropertyValue::Text(tag.into()),
        );
        if tag == "unavailable" {
            let reason = primitive::<String>(payload)?;
            output.wording(&weather, format!("Weather unavailable: {reason}."));
            output.fact(
                &weather,
                "unavailable-reason",
                FacePropertyValue::Text(reason),
            );
        } else {
            let reading = field(payload, "reading")?;
            let location = text(reading, "location")?;
            let conditions = text(reading, "conditions")?;
            let attribution = text(reading, "attribution")?;
            let milli = signed(reading, "temperature_celsius_milli")?;
            let temperature = temperature(milli);
            output.wording(&weather, if tag == "stale" {
                format!("Last reported for {location}: {temperature} °C, {conditions}. Weather is out of date.")
            } else { format!("{location}: {temperature} °C, {conditions}.") });
            output.wording(&weather, format!("Weather source: {attribution}."));
            for (name, value) in [
                ("location", location),
                ("conditions", conditions),
                ("attribution", attribution),
            ] {
                output.fact(&weather, name, FacePropertyValue::Text(value));
            }
            output.fact(
                &weather,
                "temperature-celsius-milli",
                FacePropertyValue::Signed(milli),
            );
            output.fact(
                &weather,
                "temperature-unit",
                FacePropertyValue::Identity(Unit::Celsius.semantic_id()),
            );
            let temperature = Quantity::from_decimal(i128::from(milli), -3, Unit::Celsius)
                .map_err(|_| AmbientFaceRefusal::InvalidObservation)?
                .encode()
                .to_vec();
            let contract = CheckedValueContract::new(
                kind_id(conduit_core::TEMPERATURE_INFO_ID),
                temperature.len() as u32,
                Vec::new(),
            )
            .map_err(|_| AmbientFaceRefusal::InvalidObservation)?;
            output.fact(
                &weather,
                "temperature",
                FacePropertyValue::TypedValue {
                    contract,
                    bytes: temperature,
                },
            );
            for (input, property) in [
                ("observed_at_unix_ms", "observed-at-unix-ms"),
                ("usable_until_unix_ms", "usable-until-unix-ms"),
            ] {
                output.fact(
                    &weather,
                    property,
                    FacePropertyValue::Count(unsigned(reading, input)?),
                );
            }
            output.fact(
                &weather,
                "reference-at-unix-ms",
                FacePropertyValue::Count(unsigned(payload, "reference_at_unix_ms")?),
            );
            output.fact(
                &weather,
                "clock-basis",
                FacePropertyValue::Identity("time/unix-utc@1".into()),
            );
        }
        output.finish()
    }
}

pub(super) fn validate(
    ty: &CheckedNativeType,
    value: &StructuredInfoValue,
) -> Result<(), AmbientFaceRefusal> {
    if value.value_type() != &ty.value_type {
        return Err(AmbientFaceRefusal::WrongType);
    }
    rust_binding::validate_native_contracts(value, &ty.value_contracts)
        .and_then(|_| rust_binding::validate_native_invariants(value, &ty.invariants))
        .map_err(|_| AmbientFaceRefusal::InvalidObservation)
}

fn field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, AmbientFaceRefusal> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(AmbientFaceRefusal::InvalidObservation);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(|field| field.value())
        .ok_or(AmbientFaceRefusal::InvalidObservation)
}
fn primitive<T: rust_binding::NativePrimitive>(
    value: &StructuredInfoValue,
) -> Result<T, AmbientFaceRefusal> {
    rust_binding::primitive_from_structured(value)
        .map_err(|_| AmbientFaceRefusal::InvalidObservation)
}
fn text(value: &StructuredInfoValue, name: &str) -> Result<String, AmbientFaceRefusal> {
    primitive(field(value, name)?)
}
fn unsigned(value: &StructuredInfoValue, name: &str) -> Result<u64, AmbientFaceRefusal> {
    primitive(field(value, name)?)
}
fn signed(value: &StructuredInfoValue, name: &str) -> Result<i64, AmbientFaceRefusal> {
    primitive(field(value, name)?)
}
fn temperature(milli: i64) -> String {
    let magnitude = milli.unsigned_abs();
    let mut value = format!("{}.{:03}", magnitude / 1000, magnitude % 1000);
    while value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.pop();
    }
    if milli < 0 {
        value.insert(0, '-');
    }
    value
}

struct Builder {
    fragment: FaceFragment,
    root: String,
}
impl Builder {
    fn new(
        basis: FaceContributionBasis,
        role: &str,
        name: &str,
    ) -> Result<Self, AmbientFaceRefusal> {
        let root = crate::subject_identity::subject_root(&basis, role)?;
        Ok(Self {
            root: root.clone(),
            fragment: FaceFragment {
                basis,
                subjects: crate::projection_memory::one(
                    8,
                    FaceSubject {
                        identity: root.clone(),
                        role: FaceRole::Document,
                        name: name.into(),
                    },
                ),
                relationships: Vec::with_capacity(8),
                composition: Vec::with_capacity(2),
                properties: Vec::with_capacity(32),
                text: Vec::with_capacity(8),
                actions: Vec::new(),
                disclosures: crate::projection_memory::one(
                    8,
                    FaceDisclosure {
                        subject: root,
                        level: FaceDisclosureLevel::Primary,
                    },
                ),
                temporal_references: Vec::new(),
                temporal_facts: Vec::new(),
            },
        })
    }
    fn child(&mut self, key: &str, name: &str, role: &str) -> Result<String, AmbientFaceRefusal> {
        let subject = crate::subject_identity::child_identity(&self.root, key);
        self.fragment.subjects.push(FaceSubject {
            identity: subject.clone(),
            role: FaceRole::Semantic(kind_id(role)),
            name: name.into(),
        });
        self.fragment.relationships.push(FaceRelationship {
            source: self.root.clone(),
            target: subject.clone(),
            kind: FaceRelationshipKind::Contains,
        });
        self.fragment.disclosures.push(FaceDisclosure {
            subject: subject.clone(),
            level: FaceDisclosureLevel::Primary,
        });
        Ok(subject)
    }
    fn wording(&mut self, subject: &str, text: String) {
        self.fragment.text.push(FaceText {
            subject: subject.into(),
            text,
        });
    }
    fn fact(&mut self, subject: &str, name: &str, value: FacePropertyValue) {
        self.fragment.properties.push(FaceProperty {
            subject: subject.into(),
            name: name.into(),
            value,
        });
    }
    fn finish(self) -> Result<FaceFragment, AmbientFaceRefusal> {
        self.fragment
            .validate_bounds()
            .map_err(|_| AmbientFaceRefusal::FragmentBounds)?;
        Ok(self.fragment)
    }
}

#[path = "backing.rs"]
mod backing;
