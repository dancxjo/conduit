//! Portable semantic projection of exact Presenter topology truth.

use conduit_presentation::{
    Presentation, PresentationAction, PresentationActionAvailability, PresentationDisclosure,
    PresentationDisclosureLevel, PresentationProperty, PresentationPropertyValue,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    PresentationText,
};

use crate::{PresenterTopology, PresenterTopologyRefusal, PRESENTER_TOPOLOGY_SCHEMA};

/// Add exact Presenter-chain truth and typed controls to the portable
/// Presentation consumed by both browser and native Patchbay renderers.
pub fn project_presenter_topology(
    base: &Presentation,
    topology: &PresenterTopology,
) -> Result<Presentation, PresenterTopologyRefusal> {
    topology.validate()?;
    if base.basis.body_id.as_ref() != Some(&topology.body_id)
        || base.basis.source_document_id != topology.source_document_id
        || base.identity.as_str() != topology.presentation_id
    {
        return Err(PresenterTopologyRefusal::StaleRequest);
    }
    let (mut subjects, mut relationships, mut properties, mut text, mut actions, mut disclosures) = (
        base.subjects.clone(),
        base.relationships.clone(),
        base.properties.clone(),
        base.text.clone(),
        base.actions.clone(),
        base.disclosures.clone(),
    );
    let root = format!("presenter-topology/{}", topology.presentation_id);
    subjects.push(PresentationSubject {
        identity: root.clone(),
        role: PresentationRole::Plan,
        label: "Presenter topology".into(),
        accessibility_name: "Exact current Presenter chains from immutable Plan truth".into(),
    });
    properties.push(PresentationProperty {
        subject: root.clone(),
        name: "schema".into(),
        value: PresentationPropertyValue::Text(PRESENTER_TOPOLOGY_SCHEMA.into()),
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
    actions.push(topology_action(&root, "add", "Add Presenter", true));
    actions.push(topology_action(
        &root,
        "toggle-parallel",
        "Toggle parallel chains",
        true,
    ));
    for (chain_order, chain) in topology.chains.iter().enumerate() {
        let chain_subject = format!("{root}/chain/{}", chain.chain_id);
        subjects.push(PresentationSubject {
            identity: chain_subject.clone(),
            role: PresentationRole::Manifestation,
            label: format!("Presenter chain {}", chain.chain_id),
            accessibility_name: format!(
                "Presenter chain {} in order {}",
                chain.chain_id, chain_order
            ),
        });
        relationships.push(PresentationRelationship {
            source: root.clone(),
            target: chain_subject.clone(),
            kind: PresentationRelationshipKind::Contains,
        });
        properties.push(PresentationProperty {
            subject: chain_subject.clone(),
            name: "manifestation".into(),
            value: PresentationPropertyValue::Identity(chain.manifestation_id.clone()),
        });
        actions.push(topology_action(
            &chain_subject,
            "remove",
            "Remove Presenter chain",
            true,
        ));
        actions.push(topology_action(
            &chain_subject,
            "replace",
            "Replace Presenter",
            true,
        ));
        actions.push(topology_action(
            &chain_subject,
            "reorder",
            "Reorder Presenter stages",
            chain.stages.len() > 1,
        ));
        for (stage_order, stage) in chain.stages.iter().enumerate() {
            let stage_subject = format!("{chain_subject}/stage/{}", stage.stage_id);
            subjects.push(PresentationSubject {
                identity: stage_subject.clone(),
                role: PresentationRole::Capability,
                label: stage.implementation_id.as_str().into(),
                accessibility_name: format!(
                    "Presenter stage {} on Host {} Boot {}",
                    stage_order,
                    stage.host_id.as_str(),
                    stage.boot_id.as_str()
                ),
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
    .map_err(|_| PresenterTopologyRefusal::InvalidTopology)
}

fn topology_action(
    target: &str,
    operation: &str,
    label: &str,
    available: bool,
) -> PresentationAction {
    PresentationAction {
        identity: format!("action/presenter-topology/{operation}/{target}"),
        intent: format!("conduit.intent/presenter-topology-{operation}@1"),
        target: target.into(),
        label: label.into(),
        disclosure: PresentationDisclosureLevel::CurrentAction,
        availability: if available {
            PresentationActionAvailability::Available
        } else {
            PresentationActionAvailability::Unavailable {
                reason_code: "IncompatibleType".into(),
                explanation: "A one-stage Presenter chain has no valid reordered sequence.".into(),
            }
        },
    }
}
