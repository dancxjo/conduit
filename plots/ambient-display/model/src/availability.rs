//! Acquisition truth is preserved independently of image presentation support.
use crate::{AmbientFaceRefusal, PreparedAmbientFaces};
use alloc::string::String;
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
use conduit_face::{
    FaceContributionBasis, FaceFragment, FaceProperty, FacePropertyValue, FaceText,
};
use conduit_plot::rust_binding;
use conduit_plot::rust_binding::semantic_core as conduit_core;

impl PreparedAmbientFaces {
    pub fn wallpaper_observation(
        &self,
        basis: FaceContributionBasis,
        observation: &StructuredInfoValue,
    ) -> Result<FaceFragment, AmbientFaceRefusal> {
        crate::face::validate(self.wallpaper_observation_type(), observation)?;
        let StructuredInfoValueShape::Variant { tag, payload } = observation.shape() else {
            return Err(AmbientFaceRefusal::InvalidObservation);
        };
        let reason = |value: &StructuredInfoValue| -> Result<String, AmbientFaceRefusal> {
            rust_binding::primitive_from_structured(value)
                .map_err(|_| AmbientFaceRefusal::InvalidObservation)
        };
        if tag == "unavailable" {
            return self.unavailable_wallpaper(basis, reason(payload)?);
        }
        let (selection, stale_reason) = if tag == "available" {
            (payload, None)
        } else if tag == "stale" {
            let StructuredInfoValueShape::Record(fields) = payload.shape() else {
                return Err(AmbientFaceRefusal::InvalidObservation);
            };
            let field = |name| {
                fields
                    .iter()
                    .find(|f| f.name() == name)
                    .map(|f| f.value())
                    .ok_or(AmbientFaceRefusal::InvalidObservation)
            };
            (field("selection")?, Some(reason(field("reason")?)?))
        } else {
            return Err(AmbientFaceRefusal::InvalidObservation);
        };
        let mut fragment = self.wallpaper(basis, selection)?;
        let subject = fragment.subjects[1].identity.clone();
        fragment.properties.push(FaceProperty {
            subject: subject.clone(),
            name: "availability".into(),
            value: FacePropertyValue::Text(tag.into()),
        });
        if let Some(reason) = stale_reason {
            fragment.text.push(FaceText {
                subject: subject.clone(),
                text: alloc::format!("Last available wallpaper: {reason}."),
            });
            fragment.properties.push(FaceProperty {
                subject,
                name: "stale-reason".into(),
                value: FacePropertyValue::Text(reason),
            });
        }
        fragment
            .validate_bounds()
            .map_err(|_| AmbientFaceRefusal::FragmentBounds)?;
        Ok(fragment)
    }
}
