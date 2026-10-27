//! Same numeric builder behind rich and opaque immutable flat verification.
use conduit_core as c;
use conduit_kernel::{self as k};
use conduit_plan_lowering::local_tables::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/execution_regions.rs"]
mod fixtures;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
type Rich<'a> = DirectLocalKernelTables<'a, 2, 1, 4, 8, 1, 2>;
type Flat<'a> = FlatDirectLocalKernelTables<'a, 2, 1, 4, 8, 1, 2>;
fn local_plan() -> c::Plan {
    let mut f = fixtures::transfer_plan().fragments.remove(0);
    f.execution_regions.clear();
    fixtures::common::seal(f)
}
fn metadata(plan: c::Plan) -> c::VerifiedPlanMetadata {
    c::verify_plan_metadata_with_configurations(plan, |_| Ok::<_, ()>(vec![])).unwrap()
}
#[test]
fn same_tables_routes_and_full_budgets_without_heap_or_alleged_scope() {
    let p = local_plan();
    let rich = metadata(p.clone());
    let id = &rich.preparation_metadata().fragments[0].fragment_id;
    let rich = Rich::prepare(&rich, id).unwrap();
    let f = &p.fragments[0];
    let inputs: Vec<Vec<c::PortDescriptorRef<'_>>> = f
        .placements
        .iter()
        .map(|gear| gear.inputs.iter().map(Into::into).collect())
        .collect();
    let outputs: Vec<Vec<c::PortDescriptorRef<'_>>> = f
        .placements
        .iter()
        .map(|gear| gear.outputs.iter().map(Into::into).collect())
        .collect();
    let calls: Vec<Vec<c::HostCallRequirementRef<'_>>> = f
        .placements
        .iter()
        .map(|gear| gear.host_calls.iter().map(Into::into).collect())
        .collect();
    let gears: Vec<_> = f
        .placements
        .iter()
        .enumerate()
        .map(|(index, gear)| {
            let mut view = c::PlannedGearRef::from(gear);
            view.inputs = c::PortDescriptorRefs::Borrowed(&inputs[index]);
            view.outputs = c::PortDescriptorRefs::Borrowed(&outputs[index]);
            view.host_calls = c::HostCallRequirementRefs::Borrowed(&calls[index]);
            view
        })
        .collect();
    let cords: Vec<_> = f.connections.iter().map(Into::into).collect();
    let signs: Vec<_> = f.expected_sign.iter().map(Into::into).collect();
    let mut input = c::FlatPlanRef::from_plan(&p).unwrap();
    input.placements = c::PlannedGearRefs::Borrowed(&gears);
    input.tail.connections = c::PlannedConnectionRefs::Borrowed(&cords);
    input.tail.expected_sign = c::ExpectedSignRefs::Borrowed(&signs);
    let (verified, verification) = allocation_probe::observe(|| input.verify().unwrap());
    let (flat, numeric) = allocation_probe::observe(|| Flat::prepare(&verified).unwrap());
    assert_eq!(
        (
            verification.allocations,
            verification.reallocations,
            numeric.allocations,
            numeric.reallocations
        ),
        (0, 0, 0, 0)
    );
    assert!(core::ptr::eq(flat.preparation_scope(), &verified));
    assert_eq!(flat.nodes(), rich.nodes());
    assert_eq!(flat.cords(), rich.cords());
    assert_eq!(flat.startup_order(), rich.startup_order());
    assert_eq!(
        (
            flat.cord_value_slots(),
            flat.cord_value_bytes(),
            flat.sign_items(),
            flat.sign_bytes()
        ),
        (
            rich.cord_value_slots(),
            rich.cord_value_bytes(),
            rich.sign_items(),
            rich.sign_bytes()
        )
    );
    for n in 0..2 {
        for p in 0..4 {
            assert_eq!(
                flat.routes()
                    .route(k::NodeId(n), k::PortId(p))
                    .map(|v| v.collect::<Vec<_>>()),
                rich.routes()
                    .route(k::NodeId(n), k::PortId(p))
                    .map(|v| v.collect::<Vec<_>>())
            );
        }
    }
    for (i, g) in f.placements.iter().enumerate() {
        assert_eq!(
            flat.node_for_placement(g.placement_id.as_str()),
            Some(k::NodeId(i as u16))
        );
    }
}
#[test]
fn exact_lowering_refusals_are_shared_for_resealed_structural_inputs() {
    for case in 0..18 {
        let mut f = local_plan().fragments.remove(0);
        match case {
            0 => f.placements[0].outputs[0].direction = c::PortDirection::Input,
            1 => f.placements[1].inputs[0].direction = c::PortDirection::Output,
            2 => {
                let port = f.placements[0].outputs[0].clone();
                f.placements[0].outputs.push(port);
            }
            3 => f.connections[0].item_capacity = 0,
            4 => f.connections[0].byte_capacity = 0,
            5 => f.connections[0].source_port_id = "missing".into(),
            6 => f.connections[0].sink_port_id = "missing".into(),
            7 => f.connections[0].value_kind = "wrong-kind".into(),
            8 => f.placements[0].outputs[0].value_kind = "wrong-kind".into(),
            9 => f.placements[1].inputs[0].value_kind = "wrong-kind".into(),
            10 => f.sign_storage_budget.item_capacity ^= 1,
            11 => {
                f.expected_sign
                    .push(c::ExpectedSign::PlacementTerminal("missing".into()));
                f.sign_storage_budget =
                    c::mandatory_sign_storage_requirement(&f.expected_sign).unwrap();
            }
            12 => {
                f.startup_order.pop();
            }
            13 => f.startup_order[0] = "missing".into(),
            14 => f.placements[0].host_calls.push(c::HostCallRequirement {
                contract_id: "call".into(),
                target_kind: None,
                maximum_in_flight: 2,
                maximum_input_bytes: 1,
                maximum_output_bytes: 1,
            }),
            15 => {
                let call = c::HostCallRequirement {
                    contract_id: "call".into(),
                    target_kind: None,
                    maximum_in_flight: 1,
                    maximum_input_bytes: 1,
                    maximum_output_bytes: 1,
                };
                f.placements[0].host_calls = vec![call.clone(), call];
            }
            16 => f.placements[0].resources.push(c::ResourceBinding {
                pool_id: "pool".into(),
                class_id: "class".into(),
                units: 0,
                protected: None,
                compute: None,
                content: None,
            }),
            _ => f.placements[0].resources.push(c::ResourceBinding {
                pool_id: "pool".into(),
                class_id: "class".into(),
                units: 1,
                protected: None,
                compute: None,
                content: None,
            }),
        }
        let p = fixtures::common::seal(f);
        let flat = c::FlatPlanRef::from_plan(&p).unwrap().verify().unwrap();
        let rich = metadata(p.clone());
        let id = &rich.preparation_metadata().fragments[0].fragment_id;
        let expected = Rich::prepare(&rich, id).map(|_| ());
        let actual = Flat::prepare(&flat).map(|_| ());
        assert!(expected.is_err(), "case {case}");
        assert_eq!(actual, expected, "case {case}");
    }
}
#[test]
fn fixed_capacity_refusals_do_not_become_execution_authority() {
    let p = local_plan();
    let flat = c::FlatPlanRef::from_plan(&p).unwrap().verify().unwrap();
    let rich = metadata(p.clone());
    let id = &rich.preparation_metadata().fragments[0].fragment_id;
    assert_eq!(
        FlatDirectLocalKernelTables::<1, 1, 4, 8, 1, 2>::prepare(&flat).map(|_| ()),
        DirectLocalKernelTables::<1, 1, 4, 8, 1, 2>::prepare(&rich, id).map(|_| ())
    );
    assert_eq!(
        FlatDirectLocalKernelTables::<2, 1, 0, 8, 1, 2>::prepare(&flat).map(|_| ()),
        DirectLocalKernelTables::<2, 1, 0, 8, 1, 2>::prepare(&rich, id).map(|_| ())
    );
    assert_eq!(
        FlatDirectLocalKernelTables::<2, 1, 4, 0, 0, 2>::prepare(&flat).map(|_| ()),
        DirectLocalKernelTables::<2, 1, 4, 0, 0, 2>::prepare(&rich, id).map(|_| ())
    );
}
