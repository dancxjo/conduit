use super::*;
use conduit_core::{
    port_id, verify_plan, BaseImplementationId, BootId, HostAdvertisement, HostId, HostProfileId,
    KindSemanticLaw, OfferGeneration, PlannedActivationEntry, PortDirection, PROTOCOL_VERSION,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_activations, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CanonicalBackCatalog, KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_todo_plot::{todo_combine_kind, TodoState, TODO_COMBINE_KIND};
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../../plots/todo/live.conduit");

fn authored() -> (
    conduit_plot::CheckedSyntaxDocument,
    conduit_plot::ExpandedAuthoringPlot,
    ProfileCatalog,
) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    startup
        .insert(KindSignature {
            kind: TODO_COMBINE_KIND.into(),
            startup_parameters: vec![],
        })
        .unwrap();
    profile.insert_kind(todo_combine_kind()).unwrap();
    let initial = TodoState::new("Groceries".into())
        .unwrap()
        .encode_info()
        .unwrap();
    startup
        .insert_exact_initial_info(
            kind_id(TODO_STATE_INFO_ID),
            conduit_plot::text_startup_literal("Groceries"),
            initial,
            |bytes| TodoState::decode_info(bytes).is_ok(),
        )
        .unwrap();
    let document = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    let authoring = expand_canonical_plot_for_authoring(&document, "todo/main", &profile).unwrap();
    (document, authoring, profile)
}

fn host(scan: conduit_core::CapabilityOffer) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("std-todo-offer-proof"),
        boot_id: BootId::from("std-todo-offer-proof/boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std-todo-offer-proof/profile"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![scan, crate::flow_activation::todo_combine_offer()],
    }
}

pub(crate) fn authored_todo_plan() -> conduit_core::Plan {
    let (document, authoring, profile) = authored();
    let scan = todo_scan_offer(&TodoState::new("Groceries".into()).unwrap(), 64).unwrap();
    let hosts = [host(scan)];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let empty_bases = BTreeMap::new();
    let empty_lines = BTreeMap::new();
    let options = PlanningOptions {
        connection_bases: &empty_bases,
        line_candidates: &empty_lines,
        connection_item_capacity: 1,
        connection_byte_capacity: 2 * 1635 + 2 * 75,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let bounds = BTreeMap::from([
        (
            ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("commands"),
                track: conduit_core::ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 75,
            },
        ),
        (
            ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("states"),
                track: conduit_core::ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 1635,
            },
        ),
    ]);
    let bases = [BaseImplementationId::from("conduit.base/local@1")];
    plan_expanded_authoring_with_activations(
        &document,
        &authoring,
        &profile,
        &CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &bases,
        options,
        &bounds,
    )
    .expect("exact production Todo scan must plan")
}

#[test]
fn authored_todo_selects_exact_production_offer_and_child() {
    let (_, authoring, _) = authored();
    let initial_state = TodoState::new("Groceries".into()).unwrap();
    let exact_bytes = initial_state.encode_info().unwrap();
    let scan = todo_scan_offer(&initial_state, 64).unwrap();
    assert_eq!(
        scan.semantic_contract,
        authoring.expanded.gears[0].semantic_contract
    );
    assert_eq!(scan.inputs, authoring.expanded.gears[0].inputs);
    assert_eq!(scan.outputs, authoring.expanded.gears[0].outputs);
    assert_eq!(scan.limits.max_queue_bytes, 2 * 1635 + 2 * 75);
    let KindSemanticLaw::FlowScan(law) = &scan.semantic_contract.laws[1] else {
        panic!("exact offer lost scan semantic law")
    };
    assert_eq!(law.initial_accumulator, exact_bytes);
    assert_eq!(law.accumulator.maximum_bytes, 1635);
    assert!(exact_bytes.len() < 1635);

    let plan = authored_todo_plan();
    assert!(verify_plan(&plan));
    let PlannedActivationEntry::Scan(selected) = &plan.activations[0] else {
        panic!("planned Todo did not retain scan activation")
    };
    assert_eq!(selected.initial_accumulator, exact_bytes);
    assert_eq!(selected.limits.maximum_items, 64);
    assert_eq!(selected.retained_accumulator_bytes, 1635);
    assert_eq!(selected.retained_item_bytes, 75);
    let owner = &plan.fragments[0].placements[0];
    validate_planned_todo_scan(owner, selected).unwrap();
    let mut fixture_identity = owner.clone();
    fixture_identity.capability_id = conduit_core::CapabilityId::from("flow/scan/todo-proof");
    assert!(validate_planned_todo_scan(&fixture_identity, selected).is_err());
    let mut padded = selected.clone();
    padded.initial_accumulator.push(0);
    assert!(validate_planned_todo_scan(owner, &padded).is_err());
    assert_eq!(
        selected.selected_plan.fragments[0].placements[0].implementation_id,
        crate::flow_activation::todo_combine_offer()
            .implementation
            .implementation_id
    );
}

#[test]
fn offer_refuses_out_of_range_and_distinct_initial_forms() {
    let initial = TodoState::new("Groceries".into()).unwrap();
    assert!(todo_scan_offer(&initial, 0).is_err());
    assert!(todo_scan_offer(&initial, 65).is_err());
    let (_, authoring, _) = authored();
    let other = todo_scan_offer(&TodoState::new("Other".into()).unwrap(), 64).unwrap();
    assert_ne!(
        other.semantic_contract,
        authoring.expanded.gears[0].semantic_contract
    );
    let hosts = [host(other)];
    assert!(default_expanded_placements(&authoring.expanded, &hosts).is_err());
}

#[test]
fn live_reference_host_does_not_advertise_unplayable_scan() {
    let advertisement = crate::composition::reference_advertisement(crate::StdHostConfig {
        host_id: HostId::from("std-todo-live-refusal"),
        boot_id: BootId::from("std-todo-live-refusal/boot"),
        offer_generation: OfferGeneration(1),
    });
    assert!(!advertisement
        .capabilities
        .iter()
        .any(|offer| { offer.kind_id.as_str() == conduit_semantic_catalog::FLOW_SCAN_KIND }));
}
