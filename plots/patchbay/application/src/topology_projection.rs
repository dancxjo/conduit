//! Portable semantic projection of exact Mask topology truth.

use alloc::{format, vec};
use conduit_face::{
    Face, FaceAction, FaceActionAvailability, FaceDisclosure, FaceDisclosureLevel, FaceProperty,
    FacePropertyValue, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject, FaceText,
};

use crate::{MaskTopology, MaskTopologyRefusal, MASK_TOPOLOGY_SCHEMA};

/// Add exact Mask-chain truth and typed controls to the portable
/// Face consumed by both browser and native Patchbay renderers.
pub fn project_mask_topology(
    base: &Face,
    topology: &MaskTopology,
) -> Result<Face, MaskTopologyRefusal> {
    topology.validate()?;
    if base.basis.body_id.as_ref() != Some(&topology.body_id)
        || base.basis.source_document_id != topology.source_document_id
        || base.identity.as_str() != topology.face_id
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
    let root = format!("mask-topology/{}", topology.face_id);
    subjects.push(FaceSubject {
        identity: root.clone(),
        role: FaceRole::Plan,
        name: "Mask topology".into(),
    });
    properties.push(FaceProperty {
        subject: root.clone(),
        name: "schema".into(),
        value: FacePropertyValue::Text(MASK_TOPOLOGY_SCHEMA.into()),
    });
    properties.push(FaceProperty {
        subject: root.clone(),
        name: "parallel-chain-count".into(),
        value: FacePropertyValue::Count(topology.chains.len() as u64),
    });
    disclosures.push(FaceDisclosure {
        subject: root.clone(),
        level: FaceDisclosureLevel::Primary,
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
        subjects.push(FaceSubject {
            identity: chain_subject.clone(),
            role: FaceRole::Manifestation,
            name: format!("Mask chain {}", chain.chain_id),
        });
        relationships.push(FaceRelationship {
            source: root.clone(),
            target: chain_subject.clone(),
            kind: FaceRelationshipKind::Contains,
        });
        properties.push(FaceProperty {
            subject: chain_subject.clone(),
            name: "show".into(),
            value: FacePropertyValue::Identity(chain.manifestation_id.clone()),
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
            subjects.push(FaceSubject {
                identity: stage_subject.clone(),
                role: FaceRole::Capability,
                name: stage.implementation_id.as_str().into(),
            });
            relationships.push(FaceRelationship {
                source: chain_subject.clone(),
                target: stage_subject.clone(),
                kind: FaceRelationshipKind::Contains,
            });
            for (name, value) in [
                ("placement", stage.placement_id.as_str()),
                ("capability", stage.capability_id.as_str()),
                ("implementation", stage.implementation_id.as_str()),
                ("host", stage.host_id.as_str()),
                ("boot", stage.boot_id.as_str()),
                ("input-kind", stage.input_kind.as_str()),
            ] {
                properties.push(FaceProperty {
                    subject: stage_subject.clone(),
                    name: name.into(),
                    value: FacePropertyValue::Text(value.into()),
                });
            }
            properties.push(FaceProperty {
                subject: stage_subject.clone(),
                name: "capacity-cost".into(),
                value: FacePropertyValue::Count(stage.capacity_cost.into()),
            });
            text.push(FaceText {
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
    Face::new_with_semantics(
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

fn topology_action(target: &str, operation: &str, label: &str, available: bool) -> FaceAction {
    FaceAction {
        identity: format!("action/mask-topology/{operation}/{target}"),
        intent: format!("conduit.intent/mask-topology-{operation}@1"),
        target: target.into(),
        name: label.into(),
        arguments: vec![],
        disclosure: FaceDisclosureLevel::CurrentAction,
        availability: if available {
            FaceActionAvailability::Available
        } else {
            FaceActionAvailability::Unavailable {
                reason_code: "IncompatibleType".into(),
                explanation: "A one-stage Mask chain has no valid reordered sequence.".into(),
            }
        },
    }
}
