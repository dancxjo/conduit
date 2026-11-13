//! Producer ceilings must fit consumers without enlarging admitted queues.
use conduit_core::*;
use conduit_plan_lowering::lowering::{lower_plan_fragment_from_plan, LoweringError};
#[path = "../../core/tests/common/sealed_state.rs"]
mod common;

fn plan(source_maximum: u32, sink_maximum: u32, track: ConnectionTrack) -> Plan {
    let mut fragment = common::fragment();
    fragment.states.clear();
    let source = &mut fragment.placements[0];
    source.inputs.clear();
    source.limits.max_queue_bytes = 256;
    let output = &mut source.outputs[0];
    output.value_kind = kind_id("value/text");
    let location = if track == ConnectionTrack::AbnormalTerminal {
        output.temporal = PortTemporal::Flow { closes: true };
        output.abnormal_kind = Some(kind_id("value/text"));
        FrontValueLocation::OutputAbnormal(output.port_id.clone())
    } else {
        FrontValueLocation::Output(output.port_id.clone())
    };
    source
        .semantic_contract
        .laws
        .push(KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location,
            contract: CheckedValueContract::new(kind_id("value/text"), source_maximum, vec![])
                .unwrap(),
        }]));
    let mut sink = source.clone();
    sink.placement_id = PlacementId::from("sink");
    sink.gear_id = GearId::from("sink");
    sink.inputs = vec![PortDescriptor {
        port_id: port_id("next"),
        value_kind: kind_id("value/text"),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }];
    sink.outputs.clear();
    sink.semantic_contract.laws = vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
        location: FrontValueLocation::Input(port_id("next")),
        contract: CheckedValueContract::new(kind_id("value/text"), sink_maximum, vec![]).unwrap(),
    }])];
    fragment.placements.push(sink);
    fragment.startup_order.push(PlacementId::from("sink"));
    fragment.connections.push(PlannedConnection {
        connection_id: ConnectionId::from("bounded"),
        source_placement_id: PlacementId::from("placement"),
        source_port_id: port_id("current"),
        sink_placement_id: PlacementId::from("sink"),
        sink_port_id: port_id("next"),
        value_kind: kind_id("value/text"),
        resource: None,
        abnormal_kind: None,
        track,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        selected_line: None,
        admitted_lines: vec![],
        item_capacity: 1,
        byte_capacity: 32,
    });
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    common::seal(fragment)
}

#[test]
fn narrower_producer_fits_consumer_without_enlarging_the_queue() {
    for track in [ConnectionTrack::Payload, ConnectionTrack::AbnormalTerminal] {
        for (source, sink) in [(14, 256), (14, 14), (64, 256)] {
            let plan = plan(source, sink, track);
            let (lowered, _) =
                lower_plan_fragment_from_plan(&plan, &plan.fragments[0].fragment_id).unwrap();
            assert_eq!(lowered.cords.len(), 1);
            assert_eq!(lowered.cords[0].spec.byte_capacity, 32);
            assert_eq!(lowered.cords[0].spec.maximum_value_bytes, source.min(32));
        }
    }
}

#[test]
fn wider_producer_is_refused_even_when_the_queue_is_smaller() {
    for track in [ConnectionTrack::Payload, ConnectionTrack::AbnormalTerminal] {
        let plan = plan(256, 14, track);
        assert!(matches!(
            lower_plan_fragment_from_plan(&plan, &plan.fragments[0].fragment_id),
            Err(LoweringError::ConnectionContractMismatch(id)) if id == ConnectionId::from("bounded")
        ));
    }
}

#[test]
fn narrower_bound_does_not_relax_kind_or_temporal_contracts() {
    for change_kind in [true, false] {
        let baseline = plan(14, 256, ConnectionTrack::Payload);
        let mut fragment = baseline.fragments[0].clone();
        if change_kind {
            fragment.connections[0].value_kind = kind_id("value/bytes");
        } else {
            fragment.placements[0].outputs[0].temporal = PortTemporal::Flow { closes: true };
        }
        let plan = common::seal(fragment);
        assert!(matches!(
            lower_plan_fragment_from_plan(&plan, &plan.fragments[0].fragment_id),
            Err(LoweringError::ConnectionContractMismatch(id)) if id == ConnectionId::from("bounded")
        ));
    }
}
