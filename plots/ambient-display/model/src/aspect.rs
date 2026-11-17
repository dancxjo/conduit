//! Separate semantic contributions over one checked ambient observation.
use crate::{AmbientFaceRefusal, PreparedAmbientFaces};
use alloc::string::String;
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
use conduit_face::{FaceContributionBasis, FaceFragment};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbientDisplayAspect {
    Home,
    Wallpaper,
    ClockWeather,
}
impl AmbientDisplayAspect {
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Home => crate::AMBIENT_HOME_CONTRIBUTOR_KIND,
            Self::Wallpaper => "face/ambient-wallpaper",
            Self::ClockWeather => "face/ambient-clock-weather",
        }
    }
    pub const fn revision(self) -> &'static str {
        match self {
            Self::Home => crate::AMBIENT_HOME_CONTRIBUTOR_REVISION,
            Self::Wallpaper => "ambient/wallpaper-contribution@1",
            Self::ClockWeather => "ambient/clock-weather-contribution@1",
        }
    }
    pub fn from_kind(kind: &str) -> Option<Self> {
        [Self::Home, Self::Wallpaper, Self::ClockWeather]
            .into_iter()
            .find(|a| a.kind() == kind || crate::ambient_resident_aspect_name(*a) == Some(kind))
    }
}
pub fn ambient_aspect_contributor_kind(
    aspect: AmbientDisplayAspect,
) -> Result<conduit_core::Kind, String> {
    let mut kind = crate::ambient_home_contributor_kind()?;
    kind.kind_id = conduit_core::kind_id(aspect.kind());
    kind.kind_contract_revision = aspect.revision().into();
    kind.validate()
        .map_err(|why| alloc::format!("ambient aspect Kind: {why:?}"))?;
    Ok(kind)
}
impl PreparedAmbientFaces {
    /// Validates the entire exact observation before selecting one aspect. An
    /// invalid sibling never becomes acceptable merely because it is hidden.
    pub fn aspect_canonical_observation(
        &self,
        aspect: AmbientDisplayAspect,
        basis: FaceContributionBasis,
        encoded: &[u8],
    ) -> Result<FaceFragment, AmbientFaceRefusal> {
        if encoded.len() > crate::AMBIENT_HOME_MAXIMUM_BYTES as usize {
            return Err(AmbientFaceRefusal::InvalidObservation);
        }
        conduit_core::validate_canonical_structured_value(encoded)
            .and_then(|_| self.home_validator.validate(encoded))
            .map_err(|_| AmbientFaceRefusal::InvalidObservation)?;
        let observation = StructuredInfoValue::from_canonical_bytes(encoded)
            .map_err(|_| AmbientFaceRefusal::InvalidObservation)?;
        self.aspect_observation(aspect, basis, &observation)
    }
    pub fn aspect_observation(
        &self,
        aspect: AmbientDisplayAspect,
        basis: FaceContributionBasis,
        observation: &StructuredInfoValue,
    ) -> Result<FaceFragment, AmbientFaceRefusal> {
        crate::face::validate(self.home_type(), observation)?;
        if aspect == AmbientDisplayAspect::Home {
            return self.home_observation(basis, observation);
        }
        let StructuredInfoValueShape::Record(fields) = observation.shape() else {
            return Err(AmbientFaceRefusal::WrongType);
        };
        let name = match aspect {
            AmbientDisplayAspect::Wallpaper => "wallpaper",
            _ => "clock_weather",
        };
        let value = fields
            .iter()
            .find(|f| f.name() == name)
            .ok_or(AmbientFaceRefusal::InvalidObservation)?
            .value();
        match aspect {
            AmbientDisplayAspect::Wallpaper => self.wallpaper_observation(basis, value),
            AmbientDisplayAspect::ClockWeather => self.clock_weather(basis, value),
            AmbientDisplayAspect::Home => unreachable!(),
        }
    }
}

#[cfg(test)]
#[path = "aspect/tests.rs"]
mod tests;
