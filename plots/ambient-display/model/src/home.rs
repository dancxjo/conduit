//! One composed ambient encounter, submitted as one Body contribution.
use crate::{AmbientFaceRefusal, PreparedAmbientFaces};
use alloc::{format, string::String};
use conduit_core::StructuredInfoValue;
use conduit_face::{
    FaceCompositionKind, FaceCompositionRelation, FaceContributionBasis, FaceDisclosure,
    FaceDisclosureLevel, FaceFragment, FaceRelationship, FaceRelationshipKind, FaceRole,
    FaceSubject,
};

impl PreparedAmbientFaces {
    /// Bounded decode/projection during independently paid Host Call work.
    /// The actual route supplies its scope basis; this function grants no
    /// publication authority and performs no acquisition or kernel transition.
    pub fn home_canonical_observation(
        &self,
        basis: FaceContributionBasis,
        encoded: &[u8],
    ) -> Result<FaceFragment, AmbientFaceRefusal> {
        if encoded.len() > crate::AMBIENT_HOME_MAXIMUM_BYTES as usize {
            return Err(AmbientFaceRefusal::InvalidObservation);
        }
        // Validate the exact prepared schema without allocating before parsing.
        // Arbitrary caller-supplied Type trees never enter the allocating path.
        conduit_core::validate_canonical_structured_value(encoded)
            .map_err(|_| AmbientFaceRefusal::InvalidObservation)?;
        self.home_validator
            .validate(encoded)
            .map_err(|why| match why {
                conduit_core::StructuredInfoRefusal::WrongType => AmbientFaceRefusal::WrongType,
                _ => AmbientFaceRefusal::InvalidObservation,
            })?;
        let observation = StructuredInfoValue::from_canonical_bytes(encoded)
            .map_err(|_| AmbientFaceRefusal::InvalidObservation)?;
        self.home_observation(basis, &observation)
    }

    pub fn home_observation(
        &self,
        basis: FaceContributionBasis,
        observation: &StructuredInfoValue,
    ) -> Result<FaceFragment, AmbientFaceRefusal> {
        if observation.value_type() != &self.home_type().value_type {
            return Err(AmbientFaceRefusal::WrongType);
        }
        let conduit_core::StructuredInfoValueShape::Record(fields) = observation.shape() else {
            return Err(AmbientFaceRefusal::WrongType);
        };
        let value = |name| {
            fields
                .iter()
                .find(|field| field.name() == name)
                .map(|field| field.value())
                .ok_or(AmbientFaceRefusal::InvalidObservation)
        };
        self.home(basis, value("wallpaper")?, value("clock_weather")?)
    }

    /// Compose the two reusable domains under the exact same admitted parent
    /// realization. This creates no Body, navigation session, timer or effect.
    /// The caller must commit the result through its existing execution gate.
    pub fn home(
        &self,
        basis: FaceContributionBasis,
        wallpaper: &StructuredInfoValue,
        clock_weather: &StructuredInfoValue,
    ) -> Result<FaceFragment, AmbientFaceRefusal> {
        let mut image = self.wallpaper_observation(basis.clone(), wallpaper)?;
        let mut clock = self.clock_weather(basis.clone(), clock_weather)?;
        let digest = conduit_core::semantic_digest(
            "ambient-home-subject@1",
            format!("{basis:?}").as_bytes(),
        );
        let suffix: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        let root = crate::projection_memory::subject_identity(&suffix, "home");
        for (index, child) in [&image.subjects[0], &clock.subjects[0]]
            .into_iter()
            .enumerate()
        {
            image.relationships.push(FaceRelationship {
                source: root.clone(),
                target: child.identity.clone(),
                kind: FaceRelationshipKind::Contains,
            });
            let mut composition_id =
                String::with_capacity(crate::projection_memory::IDENTITY_BYTES);
            composition_id.push_str(&root);
            composition_id.push_str("/composition/");
            composition_id.push(char::from(b'0' + index as u8));
            image.composition.push(FaceCompositionRelation {
                identity: composition_id,
                source: child.identity.clone(),
                target: root.clone(),
                kind: if index == 0 {
                    FaceCompositionKind::Subordinate
                } else {
                    FaceCompositionKind::Associate
                },
            });
        }
        image.subjects.insert(
            0,
            FaceSubject {
                identity: root.clone(),
                role: FaceRole::Document,
                name: "Home".into(),
            },
        );
        image.disclosures.push(FaceDisclosure {
            subject: root,
            level: FaceDisclosureLevel::Primary,
        });
        image.subjects.append(&mut clock.subjects);
        image.relationships.append(&mut clock.relationships);
        image.composition.append(&mut clock.composition);
        image.properties.append(&mut clock.properties);
        image.text.append(&mut clock.text);
        image.actions.append(&mut clock.actions);
        image.disclosures.append(&mut clock.disclosures);
        image
            .temporal_references
            .append(&mut clock.temporal_references);
        image.temporal_facts.append(&mut clock.temporal_facts);
        image
            .validate_bounds()
            .map_err(|_| AmbientFaceRefusal::FragmentBounds)?;
        Ok(image)
    }
}
