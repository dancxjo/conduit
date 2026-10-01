use super::{common, identity};
use crate::flow_activation::{install_planned_activation, BrowserActivationHost};
use conduit_composite::{FlowSelectCoordinator, FlowSelectState};
use conduit_core::{
    kind_id, port_id, prepare_plan_on_hosts, Plan, PlannedActivationEffectMultiplicity,
    PlannedActivationEntry, PlannedActivationFront, PlannedActivationLimits, PortDirection,
    ValuePayload, BOOL_INFO_ID,
};
use std::collections::BTreeMap;

fn predicate_plan() -> Plan {
    let source = r#"
form browser-predicate (
 >> value: Scalar
 accepted: Boolean >>
) {
 threshold: scalar/literal(0.5)
 compare: logic/compare("gt")
 value >> compare.left
 threshold >> compare.right
 compare.out >> accepted
}
"#;
    let (startup, catalog) = crate::installed_browser::catalogs().unwrap();
    let checked =
        conduit_form::check_syntax_document(&conduit_form::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded =
        conduit_form::expand_canonical_form_for_authoring(&checked, "browser-predicate", &catalog)
            .unwrap()
            .expanded;
    let identity = identity();
    let hosts = [crate::installed_browser::advertisement(
        identity.host_id,
        identity.boot_id,
    )];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let planned = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &crate::installed_browser::local_bases(),
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let mut fragment = planned.fragments[0].clone();
    let compare = fragment
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::LOGIC_COMPARE_KIND
        })
        .unwrap();
    fragment.fore_ports = vec![
        front(
            "value",
            PortDirection::Input,
            compare,
            "left",
            conduit_core::SCALAR_INFO_ID,
            conduit_core::SCALAR_ENCODED_LEN as u32,
        ),
        front(
            "accepted",
            PortDirection::Output,
            compare,
            "out",
            BOOL_INFO_ID,
            conduit_core::BOOL_ENCODED_LEN as u32,
        ),
    ];
    conduit_core::seal_plan_with_completion(
        conduit_core::FormIdentity {
            source_document_id: planned.source_document_id,
            checked_form_id: planned.checked_form_id,
            expanded_form_id: planned.expanded_form_id,
        },
        planned.completion_policy,
        vec![fragment],
    )
}

fn front(
    name: &str,
    direction: PortDirection,
    placement: &conduit_core::PlannedGear,
    gear_port: &str,
    value_kind: &str,
    byte_capacity: u32,
) -> conduit_core::PlannedForePort {
    conduit_core::PlannedForePort {
        front_port_id: port_id(name),
        direction,
        placement_id: placement.placement_id.clone(),
        gear_port_id: port_id(gear_port),
        value_kind: kind_id(value_kind),
        value_contract: None,
        abnormal_kind: None,
        track: conduit_core::ConnectionTrack::Payload,
        temporal: conduit_core::PortTemporal::Value,
        pressure_policy: conduit_core::DeliveryPressurePolicy::PreserveOrder,
        item_capacity: 1,
        byte_capacity,
    }
}

fn select_plan(child: Plan) -> Plan {
    let outer = common::fragment();
    let sign_budget = child.fragments[0].sign_storage_budget;
    let maximum_queue_bytes = child.fragments[0]
        .fore_ports
        .iter()
        .map(|port| port.byte_capacity)
        .max()
        .unwrap();
    conduit_core::seal_plan_with_activation_entries(
        conduit_core::FormIdentity {
            source_document_id: outer.source_document_id.clone(),
            checked_form_id: outer.checked_form_id.clone(),
            expanded_form_id: outer.expanded_form_id.clone(),
        },
        conduit_core::PlanCompletionPolicy::Live,
        vec![],
        vec![PlannedActivationEntry::Unary(
            conduit_core::PlannedActivation {
                activation_id: "installed-select".into(),
                owner_placement_id: conduit_core::PlacementId::from("placement"),
                selected_plan_id: child.plan_id.clone(),
                selected_plan: Box::new(child),
                input: PlannedActivationFront {
                    front_port_id: port_id("value"),
                    value_kind: kind_id(conduit_core::SCALAR_INFO_ID),
                    abnormal_kind: None,
                },
                output: PlannedActivationFront {
                    front_port_id: port_id("accepted"),
                    value_kind: kind_id(BOOL_INFO_ID),
                    abnormal_kind: None,
                },
                limits: PlannedActivationLimits {
                    maximum_active: 1,
                    maximum_queue_items: 1,
                    maximum_queue_bytes,
                    maximum_items: 1,
                },
                terminal_policy:
                    conduit_core::PlannedActivationTerminalPolicy::DrainThenPropagateExact,
                cancellation_policy: conduit_core::PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
                effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
                per_activation_sign_budget: sign_budget,
            },
        )],
        vec![outer],
    )
}

#[test]
fn installed_browser_predicate_executes_through_exact_receipt_and_definition() {
    let child = predicate_plan();
    assert!(conduit_core::verify_plan(&child));
    assert!(child.fragments[0]
        .placements
        .iter()
        .all(|placement| placement.host_calls.is_empty()));
    let plan = select_plan(child.clone());
    assert!(conduit_core::verify_plan(&plan));
    let mut host = BrowserActivationHost::for_plan(identity(), &plan).unwrap();
    let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    let receipts = prepared.subordinate_receipts();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].0, "installed-select");
    assert_eq!(receipts[0].1.fragment_id(), &child.fragments[0].fragment_id);
    let definition = conduit_composite::KernelCompositeDefinition::from_planned_activation(
        &plan,
        &prepared,
        "installed-select",
    )
    .unwrap();
    assert_eq!(definition.internal_plan.plan_id, child.plan_id);

    let unary = install_planned_activation(&plan, &mut prepared, "installed-select", &mut host)
        .unwrap()
        .into_unary()
        .unwrap();
    assert!(prepared.subordinate_receipts().is_empty());
    let mut select = FlowSelectCoordinator::from_prepared_activation(unary).unwrap();
    let input = conduit_core::Scalar::from_raw_microunits(750_000)
        .encode()
        .to_vec();
    select
        .admit(
            0,
            ValuePayload {
                value_kind: kind_id(conduit_core::SCALAR_INFO_ID),
                encoded: input.clone(),
            },
        )
        .unwrap();
    for _ in 0..64 {
        if matches!(select.step().unwrap(), FlowSelectState::OutputReady { .. }) {
            break;
        }
    }
    assert_eq!(select.output().unwrap().1.encoded, input);
}
