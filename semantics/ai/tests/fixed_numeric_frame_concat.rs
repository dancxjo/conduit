#![cfg(feature = "kernel-operation-owners")]
//! Two ordinary Value concat shapes: exact sample order/bits, no speech policy.
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_codec::FixedF32VectorCodec,
    operation_owners::fixed_numeric::*,
};
use conduit_composite::KernelOperationFactory;
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOutcome},
    HostedValueStore, PortId as KPort, ValueRef,
};
use std::collections::BTreeMap;
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
fn plan() -> Plan {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut catalog = conduit_plot::ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut catalog).unwrap();
    let plot = conduit_plot::parse_with_startup("plot concat-frame (\n >> a: NumericF32Vector40\n >> b: NumericF32Vector40\n >> c: NumericF32Vector40\n >> d: NumericF32Vector40\n result: NumericF32Vector160 >>\n) {\n first: numeric/concatenate40x40\n second: numeric/concatenate40x40\n join: numeric/concatenate80x80\n a >> first.left\n b >> first.right\n c >> second.left\n d >> second.right\n first.result >> join.left\n second.result >> join.right\n join.result >> result\n}\n", &startup, &catalog).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "concat-test".into(),
        boot_id: "concat-test-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "concat-test@1".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![
            fixed_numeric_offer("numeric/concatenate40x40").unwrap(),
            fixed_numeric_offer("numeric/concatenate80x80").unwrap(),
        ],
    };
    let hosts = [host];
    let placements = conduit_planner::default_placements(&plot, &hosts).unwrap();
    conduit_planner::plan_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        1,
        16384,
    )
    .unwrap()
}
fn frame(left: &[u8], right: &[u8], ready: bool) -> StepIo<PORTS> {
    let mut input = [None; PORTS];
    for (i, bytes) in [left, right].into_iter().enumerate() {
        input[i] = Some(ValueRef {
            slot: i as u16,
            generation: 1,
            byte_len: bytes.len() as u32,
        });
    }
    let mut output = [None; PORTS];
    if ready {
        output[0] = Some(16384);
    }
    StepIo::test_frame(input, [false; PORTS], output, None, 3)
}
fn inputs<'a>(left: &'a [u8], right: &'a [u8]) -> StepInputBytes<'a, PORTS> {
    let mut input = [None; PORTS];
    input[0] = Some(left);
    input[1] = Some(right);
    StepInputBytes::test_frame(input, None)
}
fn execute(plan: &Plan, width: usize, occurrence: usize, left: &[u8], right: &[u8]) -> Vec<u8> {
    let name = format!("numeric/concatenate{width}x{width}");
    let gear = plan.fragments[0]
        .placements
        .iter()
        .filter(|gear| gear.kind_id.as_str() == name)
        .nth(occurrence)
        .unwrap();
    let factories = FixedNumericOperationFactory::for_plan(plan, &BTreeMap::new()).unwrap();
    let factory = factories
        .iter()
        .find(|factory| factory.implementation_id() == &gear.implementation_id)
        .unwrap();
    let mut store = HostedValueStore::new(4, 16384, 65536).unwrap();
    let mut back = factory.prepare(gear, &mut store).unwrap();
    let mut blocked = frame(left, right, false);
    assert_eq!(
        back.step(&mut blocked, &inputs(left, right)),
        StepOutcome::Await
    );
    assert!(!blocked.test_consumed(KPort(0)));
    assert!(!blocked.test_consumed(KPort(1)));
    let mut io = frame(left, right, true);
    assert_eq!(
        back.step(&mut io, &inputs(left, right)),
        StepOutcome::Progress
    );
    assert!(io.test_consumed(KPort(0)));
    assert!(io.test_consumed(KPort(1)));
    let result = back.prepared_output(KPort(0)).unwrap().to_vec();
    back.step_committed();
    assert_eq!(
        back.step(&mut io, &inputs(left, right)),
        StepOutcome::Complete
    );
    result
}
#[test]
fn ordinary_factory_frame_concat_preserves_all_four_chunks_and_finite_float_bits() {
    let plan = plan();
    assert!(verify_plan(&plan));
    let values: [f32; 160] = core::array::from_fn(|i| match i % 7 {
        0 => -0.0,
        1 => f32::from_bits(1),
        2 => f32::MIN,
        3 => f32::MAX,
        _ => (i as f32 - 80.) / 16.,
    });
    let mut codec40 =
        FixedF32VectorCodec::<40>::prepare(&fixed_numeric_type("NumericF32Vector40").unwrap())
            .unwrap();
    let chunks: Vec<_> = values
        .as_chunks::<40>()
        .0
        .iter()
        .map(|chunk| codec40.encode(chunk).unwrap().to_vec())
        .collect();
    let a = execute(&plan, 40, 0, &chunks[0], &chunks[1]);
    let b = execute(&plan, 40, 1, &chunks[2], &chunks[3]);
    let actual = execute(&plan, 80, 0, &a, &b);
    let codec160 =
        FixedF32VectorCodec::<160>::prepare(&fixed_numeric_type("NumericF32Vector160").unwrap())
            .unwrap();
    let mut decoded = [0.; 160];
    codec160.decode(&actual, &mut decoded).unwrap();
    assert_eq!(decoded.map(f32::to_bits), values.map(f32::to_bits));
}
#[test]
fn frame_concat_dispatch_refuses_foreign_placement_and_wrong_input_shape_without_consumption() {
    let mut plan = plan();
    let mut codec =
        FixedF32VectorCodec::<40>::prepare(&fixed_numeric_type("NumericF32Vector40").unwrap())
            .unwrap();
    let bytes = codec.encode(&[1.; 40]).unwrap().to_vec();
    let factories = FixedNumericOperationFactory::for_plan(&plan, &BTreeMap::new()).unwrap();
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.kind_id.as_str() == "numeric/concatenate80x80")
        .unwrap();
    let factory = factories
        .iter()
        .find(|factory| factory.implementation_id() == &gear.implementation_id)
        .unwrap();
    let mut store = HostedValueStore::new(4, 16384, 65536).unwrap();
    let mut back = factory.prepare(gear, &mut store).unwrap();
    let mut io = frame(&bytes, &bytes, true);
    assert!(matches!(
        back.step(&mut io, &inputs(&bytes, &bytes)),
        StepOutcome::Fail(_)
    ));
    assert!(!io.test_consumed(KPort(0)));
    assert!(!io.test_consumed(KPort(1)));
    let mut foreign = gear.clone();
    foreign.implementation_id = "foreign/concat".into();
    assert!(factory.prepare(&foreign, &mut store).is_err());
    let gear = plan.fragments[0]
        .placements
        .iter_mut()
        .find(|gear| gear.kind_id.as_str() == "numeric/concatenate40x40")
        .unwrap();
    gear.implementation_id = "foreign/concat".into();
    assert!(factory.prepare(gear, &mut store).is_err());
}
