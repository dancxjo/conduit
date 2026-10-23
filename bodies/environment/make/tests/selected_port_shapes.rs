//! CPU capability fixture: rich and ordinary assigned binders agree on exact
//! finite source/sink port shapes. No initialized physical Base is claimed.
extern crate alloc;
use conduit_core as c;
use conduit_embedded_build as e;
use conduit_plan_lowering::{selected_assigned_operation::*, selected_operation::*};
#[path = "../../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
mod fixture;
#[path = "../../../../architecture/plan-lowering/tests/common/native_possession.rs"]
mod possession;
#[test]
fn source_sink_and_single_pair_shapes_bind_exactly() {
    for (inputs, outputs) in [(0, 0), (0, 1), (1, 0), (1, 1), (2, 1), (1, 2)] {
        check(inputs, outputs);
    }
}
fn check(inputs: usize, outputs: usize) {
    let ports = |count, direction| {
        (0..count)
            .map(|i| c::PortDescriptor {
                port_id: format!("port/{i}").into(),
                value_kind: "value/fixture".into(),
                direction,
                temporal: c::PortTemporal::Value,
                abnormal_kind: None,
            })
            .collect()
    };
    let kind = c::Kind {
        kind_id: "fixture/source-or-sink".into(),
        kind_contract_revision: "fixture/source-or-sink@1".into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: ports(inputs, c::PortDirection::Input),
        outputs: ports(outputs, c::PortDirection::Output),
        configuration: vec![],
        semantic_laws: vec![],
        limits: c::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 256,
        },
    };
    let mut scope = possession::scope(kind.kind_id.as_str());
    let (fragment, lowered, active, placement) =
        fixture::selected(&scope, &kind, "call/join", 256, "radio", "join");
    scope.plan_id = fragment.plan_id.clone();
    scope.active_play_id = active.active_play_id.clone();
    let (table, _handle, claim) = possession::possession(scope);
    let image = e::generate_embedded_plan(
        &fragment,
        &lowered,
        e::EmbeddedImageBounds {
            maximum_nodes: 1,
            maximum_cords: 0,
            maximum_routes: 1,
            maximum_route_targets: 1,
            maximum_host_calls: 1,
            maximum_resources: 1,
            maximum_sign_expectations: 16,
            maximum_configuration_entries: 0,
            maximum_ports_per_node: 8,
            maximum_remote_endpoints: 0,
            maximum_cord_value_slots: 1,
            maximum_cord_value_bytes: 1024,
            maximum_sign_items: 64,
            maximum_sign_bytes: 8192,
        },
    )
    .unwrap();
    let maxima = c::AssignedPlanMaxima {
        encoded_bytes: 8192,
        runtime_state_bytes: 8192,
        counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
    };
    let bytes = e::encode_assigned_native_plan(&image, maxima).unwrap();
    let calls = [c::AssignedIdentity::from_text("call/join")];
    let resources: Vec<_> = image.resources.iter().map(|r| r.resource).collect();

    let definition = || SelectedOperationContract {
        kind: &kind,
        call: "call/join",
        input_bytes: 256,
        output_bytes: 256,
        resource_bytes: 1,
    };
    let rich = bind_selected_operation(
        &table,
        &claim,
        definition(),
        SelectedOperationPlan {
            fragment: &fragment,
            lowered: &lowered,
            active: &active,
            placement_id: &placement,
        },
    );
    let native = || AssignedNativeIdentity {
        capability: "cap/join",
        execution_profile: "conduitos/cooperative-bounded-step@1",
        artifact: "artifact/join/fixture",
        base: "base/join",
        base_implementation: "base/join/fixture@1",
        mechanism: "machine/radio-attachment",
        authority: "authority/network-config@1",
        resource_class: "machine/radio-attachment",
    };
    let selected = || SelectedAssignedOperationPlan {
        bytes: &bytes,
        maxima,
        requirements: c::AssignedPlanRequirements {
            host: c::AssignedIdentity::from_text(fragment.host_id.as_str()),
            boot: c::AssignedIdentity::from_text(fragment.boot_id.as_str()),
            host_calls: &calls,
            resources: &resources,
            remote_bindings: &[],
        },
        computations: c::AssignedComputationProfile {
            maximum_text_bytes: 32768,
            maximum_total_text_bytes: 32768,
            requirements: &[],
        },
        fragment_id: &fragment.fragment_id,
        active: &active,
        node: lowered.nodes[0].node,
    };
    let assigned =
        bind_selected_assigned_operation(&table, &claim, definition(), native(), selected());
    let expected = if inputs <= 1 && outputs <= 1 {
        Ok(lowered.nodes[0].node)
    } else {
        Err(SelectedCallRefusal::WrongBinding)
    };
    assert_eq!(rich, expected);
    assert_eq!(assigned, expected);
    // Trusted implementation shape cannot be substituted by the Plan, even
    // when the caller retains the same semantic Kind identity and possession.
    let mut wrong_kind = kind.clone();
    wrong_kind.inputs = if inputs == 0 {
        ports(1, c::PortDirection::Input)
    } else {
        vec![]
    };
    let wrong_definition = || SelectedOperationContract {
        kind: &wrong_kind,
        ..definition()
    };
    assert_eq!(
        bind_selected_operation(
            &table,
            &claim,
            wrong_definition(),
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement,
            }
        ),
        Err(SelectedCallRefusal::WrongBinding)
    );
    assert_eq!(
        bind_selected_assigned_operation(&table, &claim, wrong_definition(), native(), selected()),
        Err(SelectedCallRefusal::WrongBinding)
    );
}
