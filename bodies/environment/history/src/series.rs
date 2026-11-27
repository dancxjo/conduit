//! Application field mapping into shared quantity-series semantics.
use crate::{
    CommittedHistoryRefusal, HistoryFaceRefusal, HistoryFaceWindow, PreparedCommittedHistoryFace,
    PreparedHistoryFace,
};
use alloc::{format, vec::Vec};
use conduit_face::{
    FaceContributionBasis, FaceFragment, FaceProperty, FacePropertyValue as V, FaceRelationship,
    FaceRelationshipKind as K, FaceRole, FaceSubject, QUANTITY_SERIES_ROLE,
};
impl PreparedHistoryFace {
    /// Prepare ordinary series intent from an explicitly bounded history view.
    /// Existing recording/clock provenance and the complete reading stay intact.
    pub fn series_contribution(
        &mut self,
        basis: FaceContributionBasis,
        window: HistoryFaceWindow,
    ) -> Result<FaceFragment, HistoryFaceRefusal> {
        let fragment = self.contribution_window(basis, window)?;
        attach(fragment, self.subject())
    }
}
impl PreparedCommittedHistoryFace<'_> {
    /// Keep the actual selected publication evidence on the shared source subject.
    pub fn series_contribution(
        &mut self,
        basis: FaceContributionBasis,
    ) -> Result<FaceFragment, CommittedHistoryRefusal> {
        let fragment = self.contribution(basis)?;
        let source = fragment
            .subjects
            .first()
            .ok_or(CommittedHistoryRefusal::History(
                HistoryFaceRefusal::Fragment,
            ))?
            .identity
            .clone();
        attach(fragment, &source).map_err(CommittedHistoryRefusal::History)
    }
}
fn attach(mut fragment: FaceFragment, source: &str) -> Result<FaceFragment, HistoryFaceRefusal> {
    let items: Vec<_> = fragment
        .relationships
        .iter()
        .filter(|relation| relation.source == source && relation.kind == K::Contains)
        .map(|relation| relation.target.clone())
        .collect();
    for item in &items {
        let available = match fragment
            .properties
            .iter()
            .find(|property| property.subject == *item && property.name == "sensor-status")
            .map(|property| &property.value)
        {
            Some(V::Text(value)) if value == "available" => true,
            Some(V::Text(value)) if value == "unavailable" => false,
            _ => return Err(HistoryFaceRefusal::Fragment),
        };
        let reason = fragment
            .properties
            .iter()
            .find(|property| property.subject == *item && property.name == "sensor-failure")
            .map(|property| property.value.clone());
        fragment.properties.push(FaceProperty {
            subject: item.clone(),
            name: "series-source".into(),
            value: V::Identity(source.into()),
        });
        fragment.properties.push(FaceProperty {
            subject: item.clone(),
            name: "series-available".into(),
            value: V::Flag(available),
        });
        if let Some(reason) = reason {
            fragment.properties.push(FaceProperty {
                subject: item.clone(),
                name: "series-unavailability".into(),
                value: reason,
            });
        }
    }
    // These choices are application policy; shared interpretation knows neither
    // DHT11 fields nor the number of environmental channels.
    for (field, name) in [
        ("temperature", "Recorded temperature"),
        ("relative-humidity", "Recorded relative humidity"),
    ] {
        let subject = format!("{source}/series/{field}");
        fragment.subjects.push(FaceSubject {
            identity: subject.clone(),
            role: FaceRole::Semantic(QUANTITY_SERIES_ROLE.into()),
            name: name.into(),
        });
        fragment.properties.push(FaceProperty {
            subject: subject.clone(),
            name: "series-source".into(),
            value: V::Identity(source.into()),
        });
        fragment.properties.push(FaceProperty {
            subject: subject.clone(),
            name: "series-value-property".into(),
            value: V::Text(field.into()),
        });
        for item in &items {
            fragment.relationships.push(FaceRelationship {
                source: subject.clone(),
                target: item.clone(),
                kind: K::Contains,
            });
        }
    }
    fragment
        .validate_bounds()
        .map_err(|_| HistoryFaceRefusal::Fragment)?;
    Ok(fragment)
}
