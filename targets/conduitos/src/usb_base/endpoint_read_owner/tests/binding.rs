use super::*;

#[test]
fn matching_ids_cannot_substitute_call_tables_or_semantic_value_contracts() {
    let (fragment, mut lowered, active, placement) = fixture::selected();
    lowered.host_calls[0].binding.maximum_output_bytes += 1;
    assert_eq!(
        fixture::bind(
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement
            },
            7
        )
        .err(),
        Some(EndpointReadOwnerRefusal::WrongBinding)
    );
    let mut fragment = fragment;
    let conduit_core::KindSemanticLaw::ValueContracts(contracts) =
        &mut fragment.placements[0].semantic_contract.laws[0]
    else {
        panic!("value contracts")
    };
    contracts[0].contract.maximum_bytes -= 1;
    let identity = conduit_core::PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = conduit_core::seal_plan(identity, alloc::vec![fragment])
        .fragments
        .remove(0);
    let active =
        conduit_core::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    assert_eq!(
        fixture::bind(
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement
            },
            7
        )
        .err(),
        Some(EndpointReadOwnerRefusal::WrongBinding)
    );
}

#[test]
fn selected_endpoint_read_owner_requires_exact_binding_and_current_possession() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let input = fixture::input();
    assert_eq!(
        owner
            .begin(
                NodeId(node.0 + 1),
                HostCallId(0),
                RequestId(owner.next_request),
                &input
            )
            .err(),
        Some(EndpointReadOwnerRefusal::WrongBinding)
    );
    assert_eq!(
        owner
            .begin(node, HostCallId(1), RequestId(owner.next_request), &input)
            .err(),
        Some(EndpointReadOwnerRefusal::WrongBinding)
    );
    assert!(
        owner
            .begin(
                node,
                HostCallId(0),
                RequestId(owner.next_request),
                &input[..input.len() - 1]
            )
            .is_err()
    );
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        0
    );
    owner.claim.boot_id = conduit_core::BootId::from("boot/foreign");
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
            .err(),
        Some(EndpointReadOwnerRefusal::Capability(
            BaseCapabilityRefusal::WrongScope
        ))
    );
    assert!(owner.pending.is_none());
}
