//! Portable semantic projection of exact Mask topology truth.

use conduit_presentation::{
    Presentation, PresentationAction, PresentationActionAvailability, PresentationDisclosure,
    PresentationDisclosureLevel, PresentationProperty, PresentationPropertyValue,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    PresentationText,
};

use crate::{MaskTopology, MaskTopologyRefusal, MASK_TOPOLOGY_SCHEMA};

/// Add exact Mask-chain truth and typed controls to the portable
/// Presentation consumed by both browser and native Patchbay renderers.
pub fn project_mask_topology(
    base: &Presentation,
    topology: &MaskTopology,
) -> Result<Presentation, MaskTopologyRefusal> {
    topology.validate()?;
    if base.basis.body_id.as_ref() != Some(&topology.body_id)
        || base.basis.source_document_id != topology.source_document_id
        || base.identity.as_str() != topology.presentation_id
    {
        return Err(MaskTopologyRefusal::StaleRequest);
    }
    let (mut subjects, mut relationships, mut properties, mut text, mut actions, mut disclosures) = (
        base.subjects.clone(),
        base.relationships.clone(),
        base.properties.clone(),
        base.text.clone(),
        base.actions.clone(),
        base.disclosures.clone(),
    );
    let root = format!("mask-topology/{}", topology.presentation_id);
    subjects.push(PresentationSubject {
        identity: root.clone(),
        role: PresentationRole::Plan,
        name: "Mask topology".into(),
    });
    properties.push(PresentationProperty {
        subject: root.clone(),
        name: "schema".into(),
        value: PresentationPropertyValue::Text(MASK_TOPOLOGY_SCHEMA.into()),
    });
    properties.push(PresentationProperty {
        subject: root.clone(),
        name: "parallel-chain-count".into(),
        value: PresentationPropertyValue::Count(topology.chains.len() as u64),
    });
    disclosures.push(PresentationDisclosure {
        subject: root.clone(),
        level: PresentationDisclosureLevel::Primary,
    });
    actions.push(topology_action(&root, "add", "Add Mask", true));
    actions.push(topology_action(
        &root,
        "toggle-parallel",
        "Toggle parallel chains",
        true,
    ));
    for chain in &topology.chains {
        let chain_subject = format!("{root}/chain/{}", chain.chain_id);
        subjects.push(PresentationSubject {
            identity: chain_subject.clone(),
            role: PresentationRole::Manifestation,
            name: format!("Mask chain {}", chain.chain_id),
        });
        relationships.push(PresentationRelationship {
            source: root.clone(),
            target: chain_subject.clone(),
            kind: PresentationRelationshipKind::Contains,
        });
        properties.push(PresentationProperty {
            subject: chain_subject.clone(),
            name: "show".into(),
            value: PresentationPropertyValue::Identity(chain.manifestation_id.clone()),
        });
        actions.push(topology_action(
            &chain_subject,
            "remove",
            "Remove Mask chain",
            true,
        ));
        actions.push(topology_action(
            &chain_subject,
            "replace",
            "Replace Mask",
            true,
        ));
        actions.push(topology_action(
            &chain_subject,
            "reorder",
            "Reorder Mask stages",
            chain.stages.len() > 1,
        ));
        for (stage_order, stage) in chain.stages.iter().enumerate() {
            let stage_subject = format!("{chain_subject}/stage/{}", stage.stage_id);
            subjects.push(PresentationSubject {
                identity: stage_subject.clone(),
                role: PresentationRole::Capability,
                name: stage.implementation_id.as_str().into(),
            });
            relationships.push(PresentationRelationship {
                source: chain_subject.clone(),
                target: stage_subject.clone(),
                kind: PresentationRelationshipKind::Contains,
            });
            for (name, value) in [
                ("placement", stage.placement_id.as_str()),
                ("capability", stage.capability_id.as_str()),
                ("implementation", stage.implementation_id.as_str()),
                ("host", stage.host_id.as_str()),
                ("boot", stage.boot_id.as_str()),
                ("input-kind", stage.input_kind.as_str()),
            ] {
                properties.push(PresentationProperty {
                    subject: stage_subject.clone(),
                    name: name.into(),
                    value: PresentationPropertyValue::Text(value.into()),
                });
            }
            properties.push(PresentationProperty {
                subject: stage_subject.clone(),
                name: "capacity-cost".into(),
                value: PresentationPropertyValue::Count(stage.capacity_cost.into()),
            });
            text.push(PresentationText {
                subject: stage_subject,
                text: format!(
                    "stage={} implementation={} host={} boot={} capacity={}",
                    stage_order,
                    stage.implementation_id.as_str(),
                    stage.host_id.as_str(),
                    stage.boot_id.as_str(),
                    stage.capacity_cost
                ),
            });
        }
    }
    Presentation::new_with_semantics(
        base.revision,
        base.basis.clone(),
        subjects,
        relationships,
        properties,
        text,
        actions,
        disclosures,
    )
    .map_err(|_| MaskTopologyRefusal::InvalidTopology)
}

fn topology_action(
    target: &str,
    operation: &str,
    label: &str,
    available: bool,
) -> PresentationAction {
    PresentationAction {
        identity: format!("action/mask-topology/{operation}/{target}"),
        intent: format!("conduit.intent/mask-topology-{operation}@1"),
        target: target.into(),
        name: label.into(),
        arguments: vec![],
        disclosure: PresentationDisclosureLevel::CurrentAction,
        availability: if available {
            PresentationActionAvailability::Available
        } else {
            PresentationActionAvailability::Unavailable {
                reason_code: "IncompatibleType".into(),
                explanation: "A one-stage Mask chain has no valid reordered sequence.".into(),
            }
        },
    }
}
