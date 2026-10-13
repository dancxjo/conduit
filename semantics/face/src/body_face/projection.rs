//! Lower one admitted application contribution into portable Face truth.

use alloc::{format, vec::Vec};

use crate::{
    FaceAction, FaceDisclosure, FaceProperty, FacePropertyValue, FaceRelationship,
    FaceRelationshipKind, FaceRole, FaceSubject, FaceTemporalFact, FaceText, TemporalReference,
};

use super::FaceContribution;

#[allow(clippy::too_many_arguments)]
pub(super) fn append_contribution(
    index: usize,
    contribution: &FaceContribution,
    context_subject: &str,
    subjects: &mut Vec<FaceSubject>,
    relationships: &mut Vec<FaceRelationship>,
    composition: &mut Vec<crate::FaceCompositionRelation>,
    properties: &mut Vec<FaceProperty>,
    text: &mut Vec<FaceText>,
    actions: &mut Vec<FaceAction>,
    disclosures: &mut Vec<FaceDisclosure>,
    temporal_references: &mut Vec<TemporalReference>,
    temporal_facts: &mut Vec<FaceTemporalFact>,
) {
    append_face_fragment(
        index,
        contribution,
        &contribution.face,
        context_subject,
        subjects,
        relationships,
        composition,
        properties,
        text,
        actions,
        disclosures,
        temporal_references,
        temporal_facts,
    );
}

#[allow(clippy::too_many_arguments)]
fn append_face_fragment(
    index: usize,
    contribution: &FaceContribution,
    fragment: &crate::FaceFragment,
    context_subject: &str,
    subjects: &mut Vec<FaceSubject>,
    relationships: &mut Vec<FaceRelationship>,
    composition: &mut Vec<crate::FaceCompositionRelation>,
    properties: &mut Vec<FaceProperty>,
    text: &mut Vec<FaceText>,
    actions: &mut Vec<FaceAction>,
    disclosures: &mut Vec<FaceDisclosure>,
    temporal_references: &mut Vec<TemporalReference>,
    temporal_facts: &mut Vec<FaceTemporalFact>,
) {
    let provenance = format!("contribution/{}/{index}", contribution.role.token());
    subjects.push(FaceSubject {
        identity: provenance.clone(),
        role: FaceRole::Plot,
        name: "Face contribution provenance".into(),
    });
    relationships.push(FaceRelationship {
        source: context_subject.into(),
        target: provenance.clone(),
        kind: FaceRelationshipKind::Contains,
    });
    properties.extend([
        identity_property(
            &provenance,
            "checked-plot-id",
            contribution.checked_plot_id.as_str(),
        ),
        identity_property(&provenance, "plan-id", contribution.plan_id.as_str()),
        identity_property(
            &provenance,
            "active-play-id",
            contribution.active_play_id.as_str(),
        ),
    ]);
    for subject in &fragment.subjects {
        relationships.push(FaceRelationship {
            source: provenance.clone(),
            target: subject.identity.clone(),
            kind: FaceRelationshipKind::Describes,
        });
    }
    for (action_index, action) in fragment.actions.iter().enumerate() {
        properties.push(FaceProperty {
            subject: provenance.clone(),
            name: format!("action/{action_index}"),
            value: FacePropertyValue::Identity(action.identity.clone()),
        });
    }
    for (property_index, property) in fragment.properties.iter().enumerate() {
        properties.push(FaceProperty {
            subject: provenance.clone(),
            name: format!("property/{property_index}"),
            value: FacePropertyValue::Identity(format!("{}/{}", property.subject, property.name)),
        });
    }
    for (composition_index, relation) in fragment.composition.iter().enumerate() {
        properties.push(FaceProperty {
            subject: provenance.clone(),
            name: format!("composition/{composition_index}"),
            value: FacePropertyValue::Identity(relation.identity.clone()),
        });
    }
    subjects.extend(fragment.subjects.iter().cloned());
    relationships.extend(fragment.relationships.iter().cloned());
    composition.extend(fragment.composition.iter().cloned());
    properties.extend(fragment.properties.iter().cloned());
    text.extend(fragment.text.iter().cloned());
    actions.extend(fragment.actions.iter().cloned());
    disclosures.extend(fragment.disclosures.iter().cloned());
    temporal_references.extend(fragment.temporal_references.iter().cloned());
    temporal_facts.extend(fragment.temporal_facts.iter().cloned());
}

fn identity_property(subject: &str, name: &str, value: &str) -> FaceProperty {
    FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value: FacePropertyValue::Identity(value.into()),
    }
}
