//! Direct generic binder regressions; timer installation and provider work are absent.
use super::*;
use conduit_plan_lowering::{
    selected_assigned_operation::{
        bind_selected_assigned_configured_operation, AssignedNativeIdentity,
        AssignedOperationInput, SelectedAssignedOperationPlan,
    },
    selected_operation::{
        bind_selected_configured_operation, SelectedCallRefusal, SelectedOperationContract,
    },
};

fn contract(kind: &Kind) -> SelectedOperationContract<'_> {
    SelectedOperationContract {
        kind,
        call: WAIT_HOST_CALL_CONTRACT,
        input_bytes: 8,
        output_bytes: 0,
        resource_bytes: 4096,
    }
}

fn candidate(
    change: impl FnOnce(&mut PlannedGear),
) -> (
    PlanFragment,
    conduit_plan_lowering::lowering::LoweredPlanFragment,
    ActivePlayIdentity,
    PlacementId,
) {
    let (mut fragment, _, _, placement) = fixture::selected();
    change(&mut fragment.placements[0]);
    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = seal_plan(identity, vec![fragment]).fragments.remove(0);
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    (fragment, lowered, active, placement)
}

#[test]
fn generic_ordinary_binding_checks_source_configuration_and_exact_zero_result_shape() {
    let kind = conduit_time::tick_semantic_contract();
    for mutation in 0..7 {
        let (fragment, lowered, active, placement) = candidate(|gear| match mutation {
            1 => gear.configuration[1].value = ConfigurationValue::U64(500),
            2 => gear.configuration[1].value = ConfigurationValue::Bool(true),
            3 => gear.outputs.clear(),
            4 => {
                let mut port = gear.outputs[0].clone();
                port.port_id = "extra".into();
                gear.outputs.push(port);
            }
            5 => {
                let mut port = gear.outputs[0].clone();
                port.direction = PortDirection::Input;
                port.port_id = "extra-input".into();
                gear.inputs.push(port);
            }
            6 => gear.host_calls[0].maximum_output_bytes = 8,
            _ => {}
        });
        let (table, _, claim) = fixture::authorization(fixture::scope(), &fragment, &active);
        let expected = fixture::configuration();
        let check = |configuration: &[ConfigurationEntry]| {
            bind_selected_configured_operation(
                &table,
                &claim,
                contract(&kind),
                configuration,
                SelectedOperationPlan {
                    fragment: &fragment,
                    lowered: &lowered,
                    active: &active,
                    placement_id: &placement,
                },
            )
        };
        if mutation == 0 {
            assert_eq!(check(&expected), Ok(lowered.nodes[0].node));
            assert_eq!(check(&[]), Err(SelectedCallRefusal::WrongBinding));
        } else {
            assert_eq!(
                check(&expected),
                Err(SelectedCallRefusal::WrongBinding),
                "mutation {mutation}"
            );
        }
        assert_eq!(table.inspections().next().unwrap().completed_operations, 0);
    }
}

#[test]
fn generic_assigned_binding_checks_configuration_and_zero_result_shape_in_both_storage_forms() {
    let kind = conduit_time::tick_semantic_contract();
    for borrowed in [false, true] {
        for mutation in 0..7 {
            let (fragment, lowered, active, _) = candidate(|gear| match mutation {
                1 => gear.configuration[1].value = ConfigurationValue::U64(500),
                2 => gear.configuration[1].value = ConfigurationValue::Bool(true),
                3 => gear.outputs.clear(),
                4 => {
                    let mut port = gear.outputs[0].clone();
                    port.port_id = "extra".into();
                    gear.outputs.push(port);
                }
                5 => {
                    let mut port = gear.outputs[0].clone();
                    port.direction = PortDirection::Input;
                    port.port_id = "extra-input".into();
                    gear.inputs.push(port);
                }
                6 => gear.host_calls[0].maximum_output_bytes = 8,
                _ => {}
            });
            let image = conduit_embedded_build::generate_embedded_plan(
                &fragment,
                &lowered,
                conduit_embedded_build::EmbeddedImageBounds {
                    maximum_nodes: 1,
                    maximum_cords: 0,
                    maximum_routes: 1,
                    maximum_route_targets: 1,
                    maximum_host_calls: 1,
                    maximum_resources: 1,
                    maximum_sign_expectations: 16,
                    maximum_configuration_entries: 2,
                    maximum_ports_per_node: 8,
                    maximum_remote_endpoints: 0,
                    maximum_cord_value_slots: 1,
                    maximum_cord_value_bytes: 1024,
                    maximum_sign_items: 64,
                    maximum_sign_bytes: 8192,
                },
            )
            .unwrap();
            let maxima = AssignedPlanMaxima {
                encoded_bytes: 8192,
                runtime_state_bytes: 8192,
                counts: [32; ASSIGNED_PLAN_COUNT_KINDS],
            };
            let bytes =
                conduit_embedded_build::encode_assigned_native_plan(&image, maxima).unwrap();
            let calls = [AssignedIdentity::from_text(WAIT_HOST_CALL_CONTRACT)];
            let resources: Vec<_> = image.resources.iter().map(|r| r.resource).collect();
            let records: Vec<_> = AssignedPlanRecords::new(&bytes)
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            let selected = |active| SelectedAssignedOperationPlan {
                input: if borrowed {
                    AssignedOperationInput::Records {
                        header: &bytes[..ASSIGNED_PLAN_HEADER_BYTES],
                        records: &records,
                    }
                } else {
                    AssignedOperationInput::Contiguous(&bytes)
                },
                maxima,
                requirements: AssignedPlanRequirements {
                    host: AssignedIdentity::from_text(fragment.host_id.as_str()),
                    boot: AssignedIdentity::from_text(fragment.boot_id.as_str()),
                    host_calls: &calls,
                    resources: &resources,
                    remote_bindings: &[],
                },
                computations: AssignedComputationProfile {
                    maximum_text_bytes: 32768,
                    maximum_total_text_bytes: 32768,
                    requirements: &[],
                },
                fragment_id: &fragment.fragment_id,
                active,
                node: lowered.nodes[0].node,
            };

            let (table, _, claim) = fixture::authorization(fixture::scope(), &fragment, &active);
            let check = |configuration: &[ConfigurationEntry]| {
                bind_selected_assigned_configured_operation(
                    &table,
                    &claim,
                    contract(&kind),
                    configuration,
                    AssignedNativeIdentity {
                        capability: "machine/wait/read-write",
                        execution_profile: "conduitos/cooperative-bounded-step@1",
                        artifact: "artifact/wait/fixture",
                        base: "base/wait",
                        base_implementation: "base/wait/fixture@1",
                        mechanism: "machine/wait-attachment",
                        authority: "authority/wait@1",
                        resource_class: "machine/wait-attachment",
                    },
                    selected(&active),
                )
            };
            if mutation == 0 {
                assert_eq!(check(&fixture::configuration()), Ok(lowered.nodes[0].node));
                assert_eq!(check(&[]), Err(SelectedCallRefusal::WrongBinding));
            } else {
                assert_eq!(
                    check(&fixture::configuration()),
                    Err(SelectedCallRefusal::WrongBinding),
                    "mutation {mutation}, borrowed {borrowed}"
                );
            }
            assert_eq!(table.inspections().next().unwrap().completed_operations, 0);
        }
    }
}
