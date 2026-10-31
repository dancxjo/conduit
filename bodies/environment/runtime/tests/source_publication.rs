//! Exact node-only storage parity; no physical/future stack claim.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{
    self as c, StructuredInfoType as Type, StructuredInfoTypeShape as Shape,
    StructuredInfoValue as Value,
};
use conduit_environment_runtime::publication::{
    PublicationBack, PublicationChannel as Channel, PublicationContract, PublicationSequence,
    SourcePublicationBack,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
fn value(ty: &Type) -> Value {
    coordinates(ty, 20, 50)
}
fn coordinates(ty: &Type, temperature: i64, humidity: i64) -> Value {
    match ty.shape() {
        Shape::Leaf(kind) => {
            let bytes = match kind.as_str() {
                c::TEMPERATURE_INFO_ID => c::Quantity::new(temperature, c::Unit::Celsius)
                    .encode()
                    .to_vec(),
                c::RATIO_INFO_ID => c::Quantity::new(humidity, c::Unit::Percent)
                    .encode()
                    .to_vec(),
                c::EMPTY_INFO_ID => vec![],
                _ => vec![0],
            };
            Value::leaf(ty.clone(), bytes).unwrap()
        }
        Shape::Collection { element, length } => Value::collection(
            ty.clone(),
            (0..length)
                .map(|_| coordinates(element, temperature, humidity))
                .collect(),
        )
        .unwrap(),
        Shape::Record { fields, .. } => Value::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    c::StructuredFieldValue::new(
                        field.name(),
                        coordinates(field.value_type(), temperature, humidity),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        Shape::Variant { cases, .. } => Value::variant(
            ty.clone(),
            cases[0].tag(),
            coordinates(cases[0].payload_type(), temperature, humidity),
        )
        .unwrap(),
        _ => panic!("publication profile"),
    }
}

fn step<B: StepBack<1>>(
    back: &mut B,
    bytes: &[u8],
    declared: u32,
    fuel: u16,
) -> (StepOutcome, bool) {
    let mut io = StepIo::test_frame(
        [Some(ValueRef {
            slot: 0,
            generation: 1,
            byte_len: declared,
        })],
        [false],
        [None],
        None,
        fuel,
    );
    let outcome = back.step(&mut io, &StepInputBytes::test_frame([Some(bytes)], None));
    (outcome, io.test_consumed(PortId(0)))
}
#[test]
fn exact_nodes_match_full_storage_all_channels_coordinates_and_transactions() {
    let mut saving = 0;
    for channel in Channel::ALL {
        let contract = PublicationContract::prepare(channel).unwrap();
        let full_order = PublicationSequence::prepare(4);
        let node_order = PublicationSequence::prepare(4);
        let (mut full, a) = allocation_probe::observe(|| {
            PublicationBack::prepare(&contract, 4, full_order.clone()).unwrap()
        });
        let (mut node, b) = allocation_probe::observe(|| {
            SourcePublicationBack::prepare(&contract, 4, node_order.clone()).unwrap()
        });
        let prefix = contract.value_type().canonical_bytes().unwrap().len();
        assert_eq!(a.retained_bytes() - b.retained_bytes(), 2 * prefix);
        saving += 2 * prefix;
        assert!(node.with_snapshot(|_| ()).is_none());
        for (index, (temperature, humidity)) in
            [(i64::MIN, i64::MIN), (i64::MAX, i64::MAX), (0, 100)]
                .into_iter()
                .enumerate()
        {
            let bytes = coordinates(contract.value_type(), temperature, humidity)
                .canonical_bytes()
                .unwrap();
            let wrong = Value::leaf(Type::leaf(c::kind_id("value/u8")).unwrap(), vec![1])
                .unwrap()
                .canonical_bytes()
                .unwrap();
            let (_, memory) = allocation_probe::observe(|| {
                for (input, declared, fuel) in [
                    (&wrong[..], wrong.len() as u32, 8),
                    (&bytes[..bytes.len() - 1], bytes.len() as u32 - 1, 8),
                    (&bytes[..], bytes.len() as u32 + 1, 8),
                    (&bytes[..], bytes.len() as u32, 0),
                ] {
                    assert_eq!(
                        step(&mut node, input, declared, fuel),
                        step(&mut full, input, declared, fuel)
                    );
                }
                assert_eq!(
                    step(&mut node, &bytes, bytes.len() as u32, 8),
                    step(&mut full, &bytes, bytes.len() as u32, 8)
                );
                assert_eq!(node.sequence(), index as u64);
                node.with_snapshot(|s| assert_eq!(s.canonical, full.snapshot().unwrap().canonical));
                <SourcePublicationBack as StepBack<1>>::step_committed(&mut node);
                <PublicationBack as StepBack<1>>::step_committed(&mut full);
                for _ in 0..3 {
                    node.with_snapshot(|s| {
                        let expected = full.snapshot().unwrap();
                        assert_eq!(
                            (s.channel, s.sequence, s.canonical),
                            (expected.channel, expected.sequence, expected.canonical)
                        );
                        assert_eq!(s.canonical, bytes);
                    });
                }
            });
            assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        }
        let bytes = value(contract.value_type()).canonical_bytes().unwrap();
        assert_eq!(
            step(&mut node, &bytes, bytes.len() as u32, 8),
            step(&mut full, &bytes, bytes.len() as u32, 8)
        );
        <SourcePublicationBack as StepBack<1>>::cancel(&mut node);
        <PublicationBack as StepBack<1>>::cancel(&mut full);
        <SourcePublicationBack as StepBack<1>>::step_committed(&mut node);
        <PublicationBack as StepBack<1>>::step_committed(&mut full);
        assert_eq!(node_order.committed(), full_order.committed());
        assert_eq!(node_order.committed(), 3);
        node.with_snapshot(|s| assert_eq!(s.canonical, full.snapshot().unwrap().canonical));
        assert_eq!(
            step(&mut node, &bytes, bytes.len() as u32, 8),
            step(&mut full, &bytes, bytes.len() as u32, 8)
        );
    }
    assert_eq!(saving, 1628);
}

#[test]
fn zero_and_exhausted_budgets_and_closed_input_match_original() {
    for maximum in [0, 1] {
        let contract = PublicationContract::prepare(Channel::ProviderFailure).unwrap();
        let bytes = value(contract.value_type()).canonical_bytes().unwrap();
        let mut full = PublicationBack::prepare(
            &contract,
            maximum,
            PublicationSequence::prepare(u64::from(maximum)),
        )
        .unwrap();
        let mut node = SourcePublicationBack::prepare(
            &contract,
            maximum,
            PublicationSequence::prepare(u64::from(maximum)),
        )
        .unwrap();
        if maximum == 1 {
            assert_eq!(
                step(&mut full, &bytes, bytes.len() as u32, 8),
                step(&mut node, &bytes, bytes.len() as u32, 8)
            );
            <PublicationBack as StepBack<1>>::step_committed(&mut full);
            <SourcePublicationBack as StepBack<1>>::step_committed(&mut node);
        }
        assert_eq!(
            step(&mut full, &bytes, bytes.len() as u32, 8),
            step(&mut node, &bytes, bytes.len() as u32, 8)
        );
        let mut a = StepIo::test_frame([None], [true], [None], None, 8);
        let mut b = StepIo::test_frame([None], [true], [None], None, 8);
        assert_eq!(
            full.step(&mut a, &StepInputBytes::test_frame([None], None)),
            node.step(&mut b, &StepInputBytes::test_frame([None], None))
        );
        assert!(a.test_consumed_closed(PortId(0)));
        assert!(b.test_consumed_closed(PortId(0)));
    }
}
