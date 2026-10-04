//! Startup cuts depend on exact declared state ports, never a kind name.
use super::*;
use alloc::vec;
use conduit_core::*;

fn fixture() -> (Vec<PlannedGear>, Vec<PlannedConnection>) {
    let plot = crate::tests::plot();
    let host = crate::tests::host();
    let hosts = [host];
    let placements = crate::default_placements(&plot, &hosts).unwrap();
    let plan = crate::plan(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    let fragment = &plan.fragments[0];
    let mut placements = fragment.placements.clone();
    let value = CheckedValueContract::new(kind_id(BOOL_INFO_ID), 1, vec![]).unwrap();
    let kind = conduit_semantic_catalog::seeded_state_semantic_contract(
        &value,
        &StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
    )
    .unwrap();
    let state = placements
        .iter_mut()
        .find(|placement| placement.placement_id == fragment.connections[0].source_placement_id)
        .unwrap();
    state.inputs = kind.inputs.clone();
    state.outputs = kind.outputs.clone();
    state.semantic_contract = kind.semantic_contract();
    state.limits = kind.limits;
    state.host_calls.clear();
    let mut connections = fragment.connections.clone();
    let mut feedback = connections[0].clone();
    core::mem::swap(
        &mut feedback.source_placement_id,
        &mut feedback.sink_placement_id,
    );
    feedback.sink_port_id = port_id("next");
    connections.push(feedback);
    (placements, connections)
}

#[test]
fn only_declared_next_breaks_the_response_cycle_and_its_startup_dependency() {
    let (placements, mut connections) = fixture();
    let order = startup_order(&placements, &connections).unwrap().unwrap();
    assert_eq!(order.len(), placements.len());
    let dependencies = startup_dependencies(&placements, &connections).unwrap();
    assert_eq!(dependencies.len(), 1);
    connections.last_mut().unwrap().sink_port_id = port_id("seed");
    assert_eq!(startup_order(&placements, &connections).unwrap(), None);
    assert_eq!(
        startup_dependencies(&placements, &connections)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn retained_state_or_a_state_kind_name_does_not_authorize_a_delay_cut() {
    let (mut placements, connections) = fixture();
    let state = placements
        .iter_mut()
        .find(|placement| placement.inputs.len() == 2)
        .unwrap();
    state.kind_id = kind_id("state/seeded/finite");
    for law in &mut state.semantic_contract.laws {
        if matches!(law, KindSemanticLaw::TemporalState(_)) {
            *law = KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained);
        }
    }
    assert_eq!(startup_order(&placements, &connections).unwrap(), None);
}

#[test]
fn malformed_declared_delay_refuses_as_invalid_state_instead_of_an_ordinary_cycle() {
    let (placements, connections) = fixture();
    for mutation in 0..4 {
        let mut placements = placements.clone();
        let state = placements
            .iter_mut()
            .find(|placement| placement.inputs.len() == 2)
            .unwrap();
        match mutation {
            0 => state.outputs[0].temporal = PortTemporal::Flow { closes: true },
            1 => state.inputs[1].value_kind = kind_id("wrong"),
            2 => state.limits.max_queue_bytes = 1,
            _ => state
                .semantic_contract
                .laws
                .push(KindSemanticLaw::TemporalState(
                    TemporalStateBehavior::SourceSeededFinite,
                )),
        }
        assert!(matches!(
            startup_order(&placements, &connections),
            Err(crate::PlannerError::InvalidStateContract(_))
        ));
    }
}
