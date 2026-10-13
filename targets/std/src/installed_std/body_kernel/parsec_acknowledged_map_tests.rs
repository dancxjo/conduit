//! Actual selected Plan genesis and source output; private acknowledgement mechanics.
use super::*;
use conduit_body::{Body, BodyPlotPlan, ResidentPlot};
use conduit_plan_lowering::{
    fragment_set::{lower_local_fragment_set_from_plans, FragmentSetBounds},
    lowering::FIXED_KERNEL_STORAGE_PROFILE,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_presentation::{ParsecMapCommand, ParsecMapState};

fn fixture() -> (
    crate::StdHost,
    BodyPlan,
    BodyPlayIdentity,
    KernelIdentityMap,
    LoweredForePort,
    NodeId,
) {
    let host = crate::BodyLiveForeQueue::parsec_input_test_host(crate::StdHostConfig {
        host_id: "proof/acknowledged-map".into(),
        boot_id: "proof/acknowledged-map/boot".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let plan = crate::BodyLiveForeQueue::authored_parsec_input_plan_on_host(host.advertisement());
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "sign/map-born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "sign/map-wake".into()).unwrap().1;
    let body_plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        }],
    )
    .unwrap();
    let prospective = BodyPlayIdentity::bind(&body_plan, 0);
    let lowered = lower_local_fragment_set_from_plans(
        &[&body_plan.plots[0].plan],
        FIXED_KERNEL_STORAGE_PROFILE,
        FragmentSetBounds {
            fragments: 1,
            nodes: super::super::super::MAX_NODES as u16,
            cords: super::super::super::MAX_CORDS as u16,
            queue_slots: super::super::super::MAX_QUEUE_SLOTS as u16,
            value_bytes: 16 * 1024 * 1024,
            sign_items: 4096,
            sign_bytes: 1024 * 1024,
        },
    )
    .unwrap();
    let partition = &lowered.partitions[0];
    let port = partition
        .fore_ports
        .iter()
        .find(|p| p.front_port_id.as_str() == "states")
        .unwrap()
        .clone();
    let PlannedActivationEntry::Scan(scan) = &body_plan.plots[0].plan.activations[0] else {
        panic!("scan");
    };
    let node = partition
        .identity
        .node_for_placement(&scan.owner_placement_id)
        .unwrap();
    (
        host,
        body_plan,
        prospective,
        partition.identity.clone(),
        port,
        node,
    )
}

#[test]
fn selected_genesis_requires_exact_confirm_and_source_ack_updates_without_allocation() {
    let (host, plan, play, identity, port, node) = fixture();
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    assert!(map.current_state().is_err());
    assert!(map.current_result().is_err());
    // Private lifecycle fixture; production confirmation is inside issued Play closure.
    map.confirm(host.advertisement(), &play).unwrap();
    let initial = ParsecMapState::decode(map.current_state().unwrap()).unwrap();
    assert!(!*initial.started());
    assert!(!*initial.keyboard());
    assert_eq!(*initial.mapping_revision(), 1);
    let sources = crate::flow_activation::ParsecSourcePlans::prepare(host.advertisement()).unwrap();
    let mut evaluator = sources.prepare_evaluator().unwrap();
    let command = ParsecMapCommand::new("game/start".into(), 1, false)
        .unwrap()
        .encode()
        .unwrap();
    let output = evaluator
        .evaluate(map.current_state().unwrap(), &command)
        .unwrap()
        .result_bytes()
        .to_vec();
    let capacities = (map.state.capacity(), map.candidate.capacity());
    let allocations = crate::allocation_probe::begin();
    map.acknowledge_lowered(&port, node, 0, &output).unwrap();
    assert_eq!(allocations.finish(), 0);
    assert_eq!((map.state.capacity(), map.candidate.capacity()), capacities);
    let result = ParsecMapTransitionResult::decode(map.current_result().unwrap()).unwrap();
    assert!(*result.accepted());
    assert!(*result.state().started());
    assert_eq!(*result.state().mapping_revision(), 2);
    map.revoke();
    assert!(map.current_state().is_err());
    assert!(map.acknowledge_lowered(&port, node, 1, &output).is_err());
}

#[test]
fn wrong_body_play_host_and_lowered_identity_never_activate_genesis() {
    let (host, plan, play, identity, _, _) = fixture();
    let mut wrong_identity = identity.clone();
    wrong_identity.plan_id = "forged/map-plan".into();
    assert!(
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &wrong_identity)
            .is_err()
    );
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    assert!(map
        .confirm(host.advertisement(), &BodyPlayIdentity::bind(&plan, 1))
        .is_err());
    assert!(map.current_state().is_err());
    let mut map =
        ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
    let mut wrong = host.advertisement().clone();
    wrong.boot_id = "wrong/boot".into();
    assert!(map.confirm(&wrong, &play).is_err());
    assert!(map.confirm(host.advertisement(), &play).is_err());
}

#[test]
fn wrong_endpoint_sequence_shape_and_owner_law_revoke_map() {
    let (host, plan, play, identity, port, node) = fixture();
    let PlannedActivationEntry::Scan(scan) = &plan.plots[0].plan.activations[0] else {
        panic!("scan");
    };
    let good = scan.initial_accumulator.clone();
    let oversize = vec![0; PARSEC_MAP_FRAME_BYTES + 1];
    let wrong_shape = ParsecMapState::new(false, 1, false)
        .unwrap()
        .encode()
        .unwrap();
    let forged = ParsecMapTransitionResult::decode(&good)
        .unwrap()
        .into_structured()
        .unwrap();
    let conduit_core::StructuredInfoValueShape::Record(fields) = forged.shape() else {
        panic!("record");
    };
    let state = fields.iter().find(|f| f.name() == "state").unwrap().value();
    let conduit_core::StructuredInfoValueShape::Record(state_fields) = state.shape() else {
        panic!("state");
    };
    let forged_state = conduit_core::StructuredInfoValue::record(
        state.value_type().clone(),
        state_fields
            .iter()
            .map(|f| {
                if f.name() != "mapping_revision" {
                    return f.clone();
                }
                conduit_core::StructuredFieldValue::new(
                    f.name(),
                    conduit_core::StructuredInfoValue::leaf(
                        f.value().value_type().clone(),
                        18u64.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let forged = conduit_core::StructuredInfoValue::record(
        forged.value_type().clone(),
        fields
            .iter()
            .map(|f| {
                if f.name() != "state" {
                    return f.clone();
                }
                conduit_core::StructuredFieldValue::new("state", forged_state.clone()).unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut wrong_port = port.clone();
    wrong_port.cord = conduit_kernel::CordId(port.cord.0 + 1);
    for (port, node, sequence, bytes) in [
        (&wrong_port, node, 0, good.as_slice()),
        (&port, NodeId(node.0 + 1), 0, good.as_slice()),
        (&port, node, 1, good.as_slice()),
        (&port, node, 32, good.as_slice()),
        (&port, node, 0, oversize.as_slice()),
        (&port, node, 0, wrong_shape.as_slice()),
        (&port, node, 0, forged.as_slice()),
    ] {
        let mut map =
            ParsecAcknowledgedMap::prepare(&plan, host.advertisement(), &play, &identity).unwrap();
        map.confirm(host.advertisement(), &play).unwrap();
        assert!(map
            .acknowledge_lowered(port, node, sequence, bytes)
            .is_err());
        assert!(map.current_state().is_err());
        assert!(map.current_result().is_err());
    }
}
