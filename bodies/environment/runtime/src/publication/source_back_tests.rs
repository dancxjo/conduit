//! Pure storage/lifecycle proof. Actual Source-edge custody is tested by the
//! ordinary resident direct-binding integration, not forged by this fixture.
extern crate std;
use super::*;
use conduit_core::{
    self as c, StructuredInfoType as Type, StructuredInfoTypeShape as Shape,
    StructuredInfoValue as Value,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
mod allocation_probe {
    extern crate std;
    use std::thread_local;
    include!("../../../../../architecture/plot/tests/common/allocation_probe.rs");
}
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
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
fn compact_producer_storage_preserves_exact_bytes_and_transactional_current() {
    let contract = PublicationContract::prepare(PublicationChannel::Reading).unwrap();
    let producer = crate::observation::ObservationBack::prepare(
        &crate::observation::ObservationContract::prepare().unwrap(),
    )
    .unwrap();
    let mut compact =
        SourcePublicationBack::prepare(&contract, 20, PublicationSequence::prepare(20)).unwrap();
    assert_eq!(compact.retained_value_storage_bytes(), 3268);
    let (_, retired) = allocation_probe::observe(|| compact.specialize_fresh(&producer).unwrap());
    assert_eq!((retired.allocations, retired.reallocations), (1, 0));
    assert_eq!(retired.deallocated_bytes, 3268);
    assert!(retired.deallocated_bytes - retired.requested_bytes >= 3000);
    std::println!(
        "source publication retired_heap={} remaining_payload_heap={} object_bytes={}",
        retired.deallocated_bytes,
        compact.retained_value_storage_bytes(),
        core::mem::size_of::<SourcePublicationBack>()
    );
    let mut generic =
        SourcePublicationBack::prepare(&contract, 20, PublicationSequence::prepare(20)).unwrap();
    let mut cases = Vec::new();
    for (t, h) in [(-10, 53), (0, 0), (20, 50), (60, 95)] {
        cases.push(
            coordinates(contract.value_type(), t, h)
                .canonical_bytes()
                .unwrap(),
        );
    }
    let Shape::Variant {
        cases: variants, ..
    } = contract.value_type().shape()
    else {
        panic!()
    };
    let failure = variants
        .iter()
        .find(|c| c.tag() == "unavailable")
        .unwrap()
        .payload_type();
    let Shape::Variant {
        cases: failures, ..
    } = failure.shape()
    else {
        panic!()
    };
    for variant in failures {
        let payload = coordinates(variant.payload_type(), 0, 0);
        let value = Value::variant(failure.clone(), variant.tag(), payload).unwrap();
        cases.push(
            Value::variant(contract.value_type().clone(), "unavailable", value)
                .unwrap()
                .canonical_bytes()
                .unwrap(),
        );
    }
    let ((), measured) = allocation_probe::observe(|| {
        for (index, bytes) in cases.iter().enumerate() {
            assert_eq!(
                step(&mut compact, bytes, bytes.len() as u32, 8),
                step(&mut generic, bytes, bytes.len() as u32, 8)
            );
            assert_eq!(compact.sequence(), index as u64);
            // A rejected next Step retires only pending state, never current.
            assert!(matches!(
                step(&mut compact, &[255], 1, 8).0,
                StepOutcome::Fail(_)
            ));
            assert!(matches!(
                step(&mut generic, &[255], 1, 8).0,
                StepOutcome::Fail(_)
            ));
            compact
                .with_snapshot(|s| generic.with_snapshot(|g| assert_eq!(s.canonical, g.canonical)));
            assert_eq!(
                step(&mut compact, bytes, bytes.len() as u32, 8),
                step(&mut generic, bytes, bytes.len() as u32, 8)
            );
            <SourcePublicationBack as StepBack<1>>::step_committed(&mut compact);
            <SourcePublicationBack as StepBack<1>>::step_committed(&mut generic);
            compact.with_snapshot(|s| {
                assert_eq!(s.canonical, bytes);
                generic.with_snapshot(|g| assert_eq!(s.canonical, g.canonical));
            });
        }
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
    let last = compact.sequence();
    let bytes = &cases[0];
    step(&mut compact, bytes, bytes.len() as u32, 8);
    assert!(compact.specialize_fresh(&producer).is_err());
    <SourcePublicationBack as StepBack<1>>::cancel(&mut compact);
    <SourcePublicationBack as StepBack<1>>::step_committed(&mut compact);
    assert_eq!(compact.sequence(), last);
    compact.with_snapshot(|s| assert_eq!(s.canonical, cases.last().unwrap()));
}
#[test]
fn generic_state_cannot_be_silently_converted_or_normalized() {
    let contract = PublicationContract::prepare(PublicationChannel::Reading).unwrap();
    let producer = crate::observation::ObservationBack::prepare(
        &crate::observation::ObservationContract::prepare().unwrap(),
    )
    .unwrap();
    let mut generic =
        SourcePublicationBack::prepare(&contract, 2, PublicationSequence::prepare(2)).unwrap();
    let bytes = coordinates(contract.value_type(), i64::MAX, i64::MIN)
        .canonical_bytes()
        .unwrap();
    assert_eq!(
        step(&mut generic, &bytes, bytes.len() as u32, 8).0,
        StepOutcome::Progress
    );
    assert!(generic.specialize_fresh(&producer).is_err());
    <SourcePublicationBack as StepBack<1>>::step_committed(&mut generic);
    generic.with_snapshot(|s| assert_eq!(s.canonical, bytes));
    assert!(generic.specialize_fresh(&producer).is_err());
    assert_eq!(generic.retained_value_storage_bytes(), 3268);
    // A compatible Unit remains valid for generic storage, but specialized
    // storage must never normalize its canonical byte representation.
    let mut fahrenheit = coordinates(contract.value_type(), 68, 50)
        .canonical_bytes()
        .unwrap();
    let quantity_offset = {
        let reading = c::validate_canonical_structured_value(&fahrenheit).unwrap();
        let temperature = reading
            .variant_payload("observation")
            .unwrap()
            .unwrap()
            .record_field("temperature")
            .unwrap()
            .unwrap();
        temperature.value_node().as_ptr() as usize - fahrenheit.as_ptr() as usize + 5
    };
    fahrenheit[quantity_offset..quantity_offset + c::QUANTITY_ENCODED_LEN]
        .copy_from_slice(&c::Quantity::new(68, c::Unit::Fahrenheit).encode());
    assert_eq!(
        step(&mut generic, &fahrenheit, fahrenheit.len() as u32, 8).0,
        StepOutcome::Progress
    );
    <SourcePublicationBack as StepBack<1>>::step_committed(&mut generic);
    generic.with_snapshot(|s| assert_eq!(s.canonical, fahrenheit));
    let mut compact =
        SourcePublicationBack::prepare(&contract, 2, PublicationSequence::prepare(2)).unwrap();
    compact.specialize_fresh(&producer).unwrap();
    assert!(matches!(
        step(&mut compact, &fahrenheit, fahrenheit.len() as u32, 8).0,
        StepOutcome::Fail(_)
    ));
    assert_eq!(compact.sequence(), 0);
    assert!(compact.with_snapshot(|_| ()).is_none());
}
