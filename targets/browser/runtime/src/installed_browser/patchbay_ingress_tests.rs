//! Preparation checks only; no advertised owner route or Host execution proof.
use super::*;

fn placement(offer: &CapabilityOffer) -> PlannedGear {
    conduit_core::planned_gear_from_parts! {
        realization_properties: offer.realization_properties.clone(),
        semantic_contract: offer.semantic_contract.clone(),
        placement_id: "patchbay-ingress".into(), gear_id: "commands".into(),
        kind_id: offer.kind_id.clone(), kind_contract_revision: offer.kind_contract_revision.clone(),
        execution_profile_id: offer.implementation.execution_profile_id.clone(), configuration: vec![],
        host_id: "browser-host".into(), boot_id: "browser-boot".into(), offer_generation: conduit_core::OfferGeneration(1),
        capability_id: offer.capability_id.clone(), implementation_id: offer.implementation.implementation_id.clone(),
        artifact_id: offer.implementation.artifact_id.clone(), base: None, realization_characteristics: vec![],
        limits: offer.limits.clone(), inputs: offer.inputs.clone(), outputs: offer.outputs.clone(),
        terminal_transductions: vec![], host_calls: offer.host_calls.clone(), resources: vec![], authority: vec![], pool_references: vec![],
    }
}

#[test]
fn canonical_sink_matches_mask_output_and_refuses_legacy_or_widened_admission() {
    let offer = offer();
    let mut startup = conduit_plot::StartupCatalog::new();
    conduit_face::install_mask_plot_value_aliases(&mut startup).unwrap();
    startup
        .insert(conduit_plot::KindSignature {
            kind: patchbay_application::PATCHBAY_INTERACTION_KIND.into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let mut profile = conduit_plot::ProfileCatalog::new();
    profile
        .insert_kind(patchbay_application::patchbay_interaction_contract())
        .unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(include_str!(
            "../../../../../plots/patchbay/interaction.conduit"
        )),
        &startup,
    )
    .unwrap();
    let authored = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "patchbay/interaction",
        &profile,
    )
    .unwrap();
    assert_eq!(authored.expanded.gears.len(), 1);
    let input = &offer.inputs[0];
    let emitted = conduit_face::interaction_outputs();
    assert_eq!(input.value_kind, emitted[0].value_kind);
    assert_eq!(input.temporal, emitted[0].temporal);
    assert_eq!(input.direction, conduit_core::PortDirection::Input);
    assert_eq!(offer.limits.max_queue_items, 1);
    assert_eq!(offer.limits.max_queue_bytes, BYTES);
    assert_eq!(offer.host_calls[0].maximum_input_bytes, BYTES);
    assert_eq!(offer.host_calls[0].maximum_output_bytes, 0);
    assert!(offer.outputs.is_empty());
    let valid = placement(&offer);
    assert!(prepare(&valid).is_ok());
    let mut legacy = valid.clone();
    legacy.inputs[0].value_kind = conduit_core::kind_id(conduit_face::APPLICATION_EVENT_INFO_ID);
    assert!(prepare(&legacy).is_err());
    let mut open = valid.clone();
    open.inputs[0].temporal = conduit_core::PortTemporal::Flow { closes: false };
    assert!(prepare(&open).is_err());
    let mut widened = valid.clone();
    widened.limits.max_queue_bytes += 1;
    assert!(prepare(&widened).is_err());
    let mut widened_call = valid;
    widened_call.host_calls[0].maximum_input_bytes += 1;
    assert!(prepare(&widened_call).is_err());
}

#[test]
fn sealed_ingress_plan_has_exact_finite_local_fore_and_requires_offered_back() {
    let mut host =
        crate::installed_browser::advertisement("ingress-host".into(), "ingress-boot".into());
    assert!(PlannedPatchbayIngress::prepare(&host).is_err());
    host.capabilities.push(offer());
    let planned = PlannedPatchbayIngress::prepare(&host).unwrap();
    assert!(conduit_core::verify_plan(planned.plan()));
    assert_eq!(planned.plan().fragments[0].host_id, host.host_id);
    assert_eq!(planned.plan().fragments[0].boot_id, host.boot_id);
    assert_eq!(planned.input().item_capacity, 1);
    assert_eq!(planned.input().byte_capacity, BYTES);
    assert_eq!(planned.lowered().cord_value_bytes, BYTES);
    assert_eq!(planned.lowered().cord_value_slots, 1);
    assert_eq!(planned.lowered().host_calls.len(), 1);
    assert_eq!(planned.lowered().nodes.len(), 1);
    let mut wrong = host;
    wrong.capabilities.last_mut().unwrap().host_calls[0].maximum_input_bytes -= 1;
    assert!(PlannedPatchbayIngress::prepare(&wrong).is_err());
}
