use std::path::PathBuf;

use conduit_core::{ArtifactId, BootId, CapabilityId, HostId, ImplementationId, OfferGeneration};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};

use crate::{
    replan_with_implementation, FormEditor, GearRealizationError, GearRealizationInspection,
    PatchbayGraph, PatchbayLayout, RealizationDisposition,
};

fn specimen() -> (conduit_form::ExpandedCanonicalForm, PatchbayGraph) {
    let editor = FormEditor::from_source(
        PathBuf::from("gear-reverse.conduit"),
        "form gear-reverse {\n    literal: text/literal(\"hello\")\n}\n".into(),
    )
    .unwrap();
    let form = editor.expand_form("gear-reverse").unwrap();
    let graph = PatchbayGraph::from_expanded(&form).unwrap();
    (form, graph)
}

fn two_hosts() -> Vec<conduit_core::HostAdvertisement> {
    let first = StdHost::new_with_composition(
        StdHostConfig {
            host_id: HostId::from("maker-host-a"),
            boot_id: BootId::from("maker-boot-a"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::reference(),
    )
    .advertisement()
    .clone();
    let mut second = first.clone();
    second.host_id = HostId::from("maker-host-b");
    second.boot_id = BootId::from("maker-boot-b");
    for offer in &mut second.capabilities {
        offer.capability_id = CapabilityId::from(format!("{}-b", offer.capability_id.as_str()));
        offer.implementation.implementation_id = ImplementationId::from(format!(
            "{}-b",
            offer.implementation.implementation_id.as_str()
        ));
        offer.implementation.artifact_id =
            ArtifactId::from(format!("{}-b", offer.implementation.artifact_id.as_str()));
    }
    vec![first, second]
}

#[test]
fn flipping_is_durable_presentation_state_and_preserves_the_exact_gear_context() {
    let (_, graph) = specimen();
    let gear = graph.subject_ref(&graph.gears[0].identity).unwrap();
    let identities = (
        graph.source_document_id.clone(),
        graph.checked_form_id.clone(),
        graph.expanded_form_id.clone(),
        graph.gears[0].gear_id.clone(),
    );
    let mut layout = PatchbayLayout::default();
    layout.move_gear(&graph, &gear, 420, 180).unwrap();
    assert!(layout.flip_gear(&graph, &gear).unwrap());
    assert!(layout.is_reversed(&gear.subject_identity));
    let encoded = serde_json::to_vec(&layout).unwrap();
    let mut reopened: PatchbayLayout = serde_json::from_slice(&encoded).unwrap();
    reopened.validate().unwrap();
    assert_eq!(reopened.position(&gear.subject_identity), Some((420, 180)));
    assert!(!reopened.flip_gear(&graph, &gear).unwrap());
    assert_eq!(
        identities,
        (
            graph.source_document_id,
            graph.checked_form_id,
            graph.expanded_form_id,
            graph.gears[0].gear_id.clone(),
        )
    );
}

#[test]
fn reverse_inspection_derives_selected_and_compatible_realizations_from_plan_and_hosts() {
    let (form, graph) = specimen();
    let hosts = two_hosts();
    let placements = default_expanded_placements(&form, &hosts).unwrap();
    let plan = plan_expanded_canonical(
        &form,
        &hosts,
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
    )
    .unwrap();
    let gear = graph.subject_ref(&graph.gears[0].identity).unwrap();
    let inspection =
        GearRealizationInspection::inspect(&graph, &gear, Some(&plan), &hosts).unwrap();
    assert_eq!(inspection.alternatives.len(), 2);
    assert_eq!(
        inspection
            .alternatives
            .iter()
            .filter(|candidate| candidate.disposition == RealizationDisposition::Selected)
            .count(),
        1
    );
    assert_eq!(
        inspection
            .alternatives
            .iter()
            .filter(|candidate| candidate.disposition == RealizationDisposition::Compatible)
            .count(),
        1
    );
    let selected = inspection.selected.as_ref().unwrap();
    assert!(inspection.alternatives.iter().any(|candidate| {
        candidate.disposition == RealizationDisposition::Selected
            && candidate.host_id == selected.host_id
            && candidate.boot_id == selected.boot_id
            && candidate.implementation_id == selected.implementation_id
    }));
}

#[test]
fn requesting_an_alternative_replans_without_mutating_form_or_prior_plan() {
    let (form, graph) = specimen();
    let hosts = two_hosts();
    let placements = default_expanded_placements(&form, &hosts).unwrap();
    let plan = plan_expanded_canonical(
        &form,
        &hosts,
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
    )
    .unwrap();
    let prior = plan.clone();
    let gear = graph.subject_ref(&graph.gears[0].identity).unwrap();
    let inspection =
        GearRealizationInspection::inspect(&graph, &gear, Some(&plan), &hosts).unwrap();
    let alternative = inspection
        .alternatives
        .iter()
        .find(|candidate| candidate.disposition == RealizationDisposition::Compatible)
        .unwrap();
    let replacement = replan_with_implementation(
        &form,
        &plan,
        &hosts,
        &gear,
        &alternative.host_id,
        &alternative.capability_id,
    )
    .unwrap();
    assert_ne!(replacement.plan_id, plan.plan_id);
    assert_eq!(prior, plan);
    assert_eq!(replacement.source_document_id, plan.source_document_id);
    assert_eq!(replacement.checked_form_id, plan.checked_form_id);
    assert_eq!(replacement.expanded_form_id, plan.expanded_form_id);
    assert!(replacement.fragments.iter().any(|fragment| {
        fragment.placements.iter().any(|placement| {
            placement.gear_id == graph.gears[0].gear_id
                && placement.host_id == alternative.host_id
                && placement.capability_id == alternative.capability_id
        })
    }));
    assert_eq!(
        replan_with_implementation(
            &form,
            &plan,
            &hosts,
            &gear,
            &HostId::from("invented"),
            &CapabilityId::from("invented"),
        ),
        Err(GearRealizationError::UnknownAlternative)
    );
}
