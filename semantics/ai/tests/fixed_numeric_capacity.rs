#![cfg(feature = "kernel-operation-owners")]
use conduit_ai::{
    fixed_numeric_catalog::fixed_numeric_type, fixed_numeric_codec::FixedF32VectorCodec,
    fixed_numeric_signal_back::*, fixed_numeric_temporal::*, operation_owners::fixed_numeric::*,
};
use conduit_composite::KernelOperationFactory;
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOutcome},
    HostedValueStore, PortId as KPort, ValueRef,
};
use std::collections::BTreeMap;
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
#[allow(dead_code)]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
fn plan(count: usize, large: bool) -> Result<Plan, String> {
    plan_kind(count, large, "numeric/flow-add128")
}
fn plan_kind(count: usize, large: bool, kind: &str) -> Result<Plan, String> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    if kind.starts_with("numeric/flow-") {
        if large {
            install_closing_numeric_catalogs_capacity64(&mut startup, &mut profiles)?;
        } else {
            install_closing_numeric_catalogs(&mut startup, &mut profiles)?;
        }
    } else if large {
        conduit_ai::fixed_numeric_catalog::install_fixed_numeric_catalogs_capacity64(
            &mut startup,
            &mut profiles,
        )?;
    } else {
        conduit_ai::fixed_numeric_catalog::install_fixed_numeric_catalogs(
            &mut startup,
            &mut profiles,
        )?;
    }
    let mut source = String::from("plot capacity {\n");
    for i in 0..count {
        source.push_str(&format!(" n{i}: {kind}\n"));
    }
    source.push_str("}\n");
    let plot = conduit_plot::parse_with_startup(&source, &startup, &profiles)
        .map_err(|e| format!("{e:?}"))?;
    let offer = if large {
        fixed_numeric_offer_capacity64(kind)?
    } else {
        fixed_numeric_offer(kind)?
    };
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "test/capacity".into(),
        boot_id: "test/boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "test/numeric".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![offer],
    }];
    let placements =
        conduit_planner::default_placements(&plot, &hosts).map_err(|e| format!("{e:?}"))?;
    conduit_planner::plan_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        1,
        16384,
    )
    .map_err(|e| format!("{e:?}"))
}
#[test]
fn explicit_instance_profile_preserves_default_front_and_refuses_overbooking_and_cross_selection() {
    let default = fixed_numeric_offer("numeric/flow-add128").unwrap();
    let large = fixed_numeric_offer_capacity64("numeric/flow-add128").unwrap();
    assert_eq!(default.limits.max_active_instances, 16);
    assert_eq!(large.limits.max_active_instances, 64);
    assert_eq!(default.semantic_contract, large.semantic_contract);
    assert_eq!(default.inputs, large.inputs);
    assert_eq!(default.outputs, large.outputs);
    assert_eq!(default.limits.max_queue_bytes, large.limits.max_queue_bytes);
    assert_ne!(default.kind_contract_revision, large.kind_contract_revision);
    assert_ne!(default.capability_id, large.capability_id);
    assert!(plan(17, false).is_err());
    assert!(plan(65, true).is_err());
    let ordinary = plan(16, false).unwrap();
    FixedNumericOperationFactory::for_plan(&ordinary, &BTreeMap::new()).unwrap();
    let admitted = plan(17, true).unwrap();
    assert!(FixedNumericOperationFactory::for_plan(&admitted, &BTreeMap::new()).is_err());
    let owners =
        FixedNumericOperationFactory::for_plan_capacity64(&admitted, &BTreeMap::new()).unwrap();
    let owner = &owners[0];
    let declared: u64 = admitted.fragments[0]
        .placements
        .iter()
        .map(|gear| u64::from(owner.budget(gear).unwrap().value_bytes))
        .sum();
    assert_eq!(declared, 17 * 16384);
    let mut forged = admitted.fragments[0].placements[0].clone();
    forged.execution_profile_id = default.implementation.execution_profile_id;
    assert!(owner.budget(&forged).is_err());
}
#[test]
fn seventeen_prepared_flow_backs_have_bounded_storage_and_allocation_free_repeated_steps() {
    let admitted = plan(17, true).unwrap();
    let owners =
        FixedNumericOperationFactory::for_plan_capacity64(&admitted, &BTreeMap::new()).unwrap();
    let owner = &owners[0];
    let mut store = HostedValueStore::new(64, 16384, 64 * 16384).unwrap();
    let ty = fixed_numeric_type("NumericF32Vector128").unwrap();
    let mut codec = FixedF32VectorCodec::<128>::prepare(&ty).unwrap();
    let left = codec.encode(&[1.0; 128]).unwrap().to_vec();
    let right = codec.encode(&[2.0; 128]).unwrap().to_vec();
    let expected = codec.encode(&[3.0; 128]).unwrap().to_vec();
    let refs = [
        ValueRef {
            slot: 0,
            generation: 1,
            byte_len: left.len() as u32,
        },
        ValueRef {
            slot: 1,
            generation: 1,
            byte_len: right.len() as u32,
        },
    ];
    let mut backs: Vec<_> = admitted.fragments[0]
        .placements
        .iter()
        .map(|gear| owner.prepare(gear, &mut store).unwrap())
        .collect();
    assert_eq!(backs.len(), 17);
    for back in &mut backs {
        for _ in 0..3 {
            let mut inputs = [None; PORTS];
            inputs[0] = Some(refs[0]);
            inputs[1] = Some(refs[1]);
            let mut capacity = [None; PORTS];
            capacity[0] = Some(16384);
            let mut io = StepIo::test_frame(inputs, [false; PORTS], capacity, None, 3);
            let mut bytes = [None; PORTS];
            bytes[0] = Some(left.as_slice());
            bytes[1] = Some(right.as_slice());
            let inputs = StepInputBytes::test_frame(bytes, None);
            let (out, heap) = allocation_probe::observe(|| back.step(&mut io, &inputs));
            assert_eq!(out, StepOutcome::Progress);
            assert_eq!(heap.allocations, 0);
            assert_eq!(heap.reallocations, 0);
            assert_eq!(back.prepared_output(KPort(0)).unwrap(), expected.as_slice());
            back.step_committed();
        }
    }
    // An edited profile cannot bypass each concrete Back's exact admission.
    let mut forged = admitted.fragments[0].placements[0].clone();
    forged.limits.max_active_instances = 65;
    assert!(FixedElementwiseBack::<128>::prepare_flow_planned::<PORTS>(
        &forged,
        3,
        FixedElementwiseOperation::Add
    )
    .is_err());
}

#[test]
fn capacity64_index_profile_prepares_exact_slice_and_refuses_edited_budget() {
    use conduit_ai::{fixed_numeric_index_back::*, fixed_numeric_index_codec::FixedU16IndexCodec};
    let kind = "numeric/flow-slice384x128";
    assert!(plan_kind(17, false, kind).is_err());
    assert!(plan_kind(65, true, kind).is_err());
    let admitted = plan_kind(17, true, kind).unwrap();
    let owners =
        FixedNumericOperationFactory::for_plan_capacity64(&admitted, &BTreeMap::new()).unwrap();
    let mut store = HostedValueStore::new(64, 16384, 64 * 16384).unwrap();
    let mut back = owners[0]
        .prepare(&admitted.fragments[0].placements[0], &mut store)
        .unwrap();
    let mut vector =
        FixedF32VectorCodec::<384>::prepare(&fixed_numeric_type("NumericF32Vector384").unwrap())
            .unwrap();
    let values = core::array::from_fn(|index| index as f32);
    let encoded = vector.encode(&values).unwrap().to_vec();
    let scalar = StructuredInfoType::leaf(kind_id("value/u16")).unwrap();
    let mut index = FixedU16IndexCodec::<1>::prepare(&scalar).unwrap();
    let start = index.encode(&[128]).to_vec();
    let mut output =
        FixedF32VectorCodec::<128>::prepare(&fixed_numeric_type("NumericF32Vector128").unwrap())
            .unwrap();
    let expected = output
        .encode(&core::array::from_fn(|i| values[128 + i]))
        .unwrap()
        .to_vec();
    for _ in 0..3 {
        let mut ports = [None; PORTS];
        for (i, bytes) in [&encoded, &start].iter().enumerate() {
            ports[i] = Some(ValueRef {
                slot: i as u16,
                generation: 1,
                byte_len: bytes.len() as u32,
            });
        }
        let mut capacities = [None; PORTS];
        capacities[0] = Some(16384);
        let mut io = StepIo::test_frame(ports, [false; PORTS], capacities, None, 3);
        let mut bytes = [None; PORTS];
        bytes[0] = Some(encoded.as_slice());
        bytes[1] = Some(start.as_slice());
        let inputs = StepInputBytes::test_frame(bytes, None);
        let (outcome, heap) = allocation_probe::observe(|| back.step(&mut io, &inputs));
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(heap.allocations, 0);
        assert_eq!(heap.reallocations, 0);
        assert_eq!(back.prepared_output(KPort(0)).unwrap(), expected.as_slice());
        back.step_committed();
    }
    let mut forged = admitted.fragments[0].placements[0].clone();
    forged.limits.max_active_instances = 65;
    assert!(FixedIndexBack::<384, 128>::prepare_flow_planned::<PORTS>(
        &forged,
        3,
        FixedIndexOperation::Slice
    )
    .is_err());
}

#[test]
fn explicit_value_capacity_preserves_one_shot_semantics_and_default_resource_limits() {
    for kind in ["numeric/tanh128", "numeric/add128", "numeric/slice384x128"] {
        let default = fixed_numeric_offer(kind).unwrap();
        let large = fixed_numeric_offer_capacity64(kind).unwrap();
        assert_eq!(default.limits.max_active_instances, 16);
        assert_eq!(large.limits.max_active_instances, 64);
        assert_eq!(default.semantic_contract, large.semantic_contract);
        assert_eq!(default.inputs, large.inputs);
        assert_eq!(default.outputs, large.outputs);
        assert_ne!(default.kind_contract_revision, large.kind_contract_revision);
        assert!(plan_kind(17, false, kind).is_err());
        assert!(plan_kind(65, true, kind).is_err());
        let admitted = plan_kind(17, true, kind).unwrap();
        assert!(FixedNumericOperationFactory::for_plan(&admitted, &BTreeMap::new()).is_err());
        FixedNumericOperationFactory::for_plan_capacity64(&admitted, &BTreeMap::new()).unwrap();
    }
    for kind in [
        "numeric/dense32x64",
        "numeric/linear128x384",
        "numeric/embedding224x12",
    ] {
        assert_eq!(
            fixed_numeric_offer_capacity64(kind).unwrap(),
            fixed_numeric_offer(kind).unwrap()
        );
    }
    let admitted = plan_kind(17, true, "numeric/tanh128").unwrap();
    let owners =
        FixedNumericOperationFactory::for_plan_capacity64(&admitted, &BTreeMap::new()).unwrap();
    let mut store = HostedValueStore::new(64, 16384, 64 * 16384).unwrap();
    let mut codec =
        FixedF32VectorCodec::<128>::prepare(&fixed_numeric_type("NumericF32Vector128").unwrap())
            .unwrap();
    let input = codec.encode(&[0.; 128]).unwrap().to_vec();
    for gear in &admitted.fragments[0].placements {
        let mut back = owners[0].prepare(gear, &mut store).unwrap();
        let mut ports = [None; PORTS];
        ports[0] = Some(ValueRef {
            slot: 0,
            generation: 1,
            byte_len: input.len() as u32,
        });
        let mut capacity = [None; PORTS];
        capacity[0] = Some(16384);
        let mut io = StepIo::test_frame(ports, [false; PORTS], capacity, None, 3);
        let mut bytes = [None; PORTS];
        bytes[0] = Some(input.as_slice());
        let bytes = StepInputBytes::test_frame(bytes, None);
        let (outcome, heap) = allocation_probe::observe(|| back.step(&mut io, &bytes));
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(heap.allocations, 0);
        assert_eq!(heap.reallocations, 0);
        assert_eq!(back.prepared_output(KPort(0)).unwrap(), input.as_slice());
        back.step_committed();
        let mut io = StepIo::test_frame(ports, [false; PORTS], capacity, None, 3);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Complete);
    }
    let mut forged = admitted.fragments[0].placements[0].clone();
    forged.limits.max_active_instances = 65;
    assert!(
        conduit_ai::fixed_numeric_operations_back::FixedTanhBack::<128>::prepare_planned::<PORTS>(
            &forged, 3
        )
        .is_err()
    );
}
