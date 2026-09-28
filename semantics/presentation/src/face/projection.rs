//! Lower one admitted application contribution into portable Presentation truth.

use alloc::{format, vec::Vec};

use crate::{
    PresentationAction, PresentationDisclosure, PresentationProperty, PresentationPropertyValue,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    PresentationTemporalFact, PresentationText, TemporalReference,
};

use super::FaceContribution;

#[allow(clippy::too_many_arguments)]
pub(super) fn append_contribution(
    index: usize,
    contribution: &FaceContribution,
    context_subject: &str,
    subjects: &mut Vec<PresentationSubject>,
    relationships: &mut Vec<PresentationRelationship>,
    composition: &mut Vec<crate::PresentationCompositionRelation>,
    properties: &mut Vec<PresentationProperty>,
    text: &mut Vec<PresentationText>,
    actions: &mut Vec<PresentationAction>,
    disclosures: &mut Vec<PresentationDisclosure>,
    temporal_references: &mut Vec<TemporalReference>,
    temporal_facts: &mut Vec<PresentationTemporalFact>,
) {
    append_presentation_fragment(
        index,
        contribution,
        &contribution.presentation,
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
fn append_presentation_fragment(
    index: usize,
    contribution: &FaceContribution,
    fragment: &crate::PresentationFragment,
    context_subject: &str,
    subjects: &mut Vec<PresentationSubject>,
    relationships: &mut Vec<PresentationRelationship>,
    composition: &mut Vec<crate::PresentationCompositionRelation>,
    properties: &mut Vec<PresentationProperty>,
    text: &mut Vec<PresentationText>,
    actions: &mut Vec<PresentationAction>,
    disclosures: &mut Vec<PresentationDisclosure>,
    temporal_references: &mut Vec<TemporalReference>,
    temporal_facts: &mut Vec<PresentationTemporalFact>,
) {
    let provenance = format!("contribution/{}/{index}", contribution.role.token());
    subjects.push(PresentationSubject {
        identity: provenance.clone(),
        role: PresentationRole::Form,
        name: "Presentation contribution provenance".into(),
    });
    relationships.push(PresentationRelationship {
        source: context_subject.into(),
        target: provenance.clone(),
        kind: PresentationRelationshipKind::Contains,
    });
    properties.extend([
        identity_property(
            &provenance,
            "checked-form-id",
            contribution.checked_form_id.as_str(),
        ),
        identity_property(&provenance, "plan-id", contribution.plan_id.as_str()),
        identity_property(
            &provenance,
            "active-play-id",
            contribution.active_play_id.as_str(),
        ),
    ]);
    for subject in &fragment.subjects {
        relationships.push(PresentationRelationship {
            source: provenance.clone(),
            target: subject.identity.clone(),
            kind: PresentationRelationshipKind::Describes,
        });
    }
    for (action_index, action) in fragment.actions.iter().enumerate() {
        properties.push(PresentationProperty {
            subject: provenance.clone(),
            name: format!("action/{action_index}"),
            value: PresentationPropertyValue::Identity(action.identity.clone()),
        });
    }
    for (property_index, property) in fragment.properties.iter().enumerate() {
        properties.push(PresentationProperty {
            subject: provenance.clone(),
            name: format!("property/{property_index}"),
            value: PresentationPropertyValue::Identity(format!(
                "{}/{}",
                property.subject, property.name
            )),
        });
    }
    for (composition_index, relation) in fragment.composition.iter().enumerate() {
        properties.push(PresentationProperty {
            subject: provenance.clone(),
            name: format!("composition/{composition_index}"),
            value: PresentationPropertyValue::Identity(relation.identity.clone()),
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

fn identity_property(subject: &str, name: &str, value: &str) -> PresentationProperty {
    PresentationProperty {
        subject: subject.into(),
        name: name.into(),
        value: PresentationPropertyValue::Identity(value.into()),
    }
}
