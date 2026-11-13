//! Same production bridge/kernel, with exact structured validation. Root activation
//! remains an explicit inert unit fixture, never a live Line or Start proof.
use super::*;
use std::boxed::Box;
fn fixture() -> (c::KindId, &'static [u8], std::vec::Vec<u8>) {
    let boolean = c::StructuredInfoType::leaf(c::kind_id(c::BOOL_INFO_ID)).unwrap();
    let schema = c::StructuredInfoType::record(
        c::kind_id("test/fore-frame"),
        vec![
            c::StructuredFieldType::new("flag", boolean.clone()).unwrap(),
            c::StructuredFieldType::new("hidden", boolean.clone()).unwrap(),
        ],
    )
    .unwrap();
    let value = c::StructuredInfoValue::record(
        schema.clone(),
        vec![
            c::StructuredFieldValue::new(
                "flag",
                c::StructuredInfoValue::leaf(boolean.clone(), vec![0]).unwrap(),
            )
            .unwrap(),
            c::StructuredFieldValue::new(
                "hidden",
                c::StructuredInfoValue::leaf(boolean, vec![1]).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let kind = schema.profile().unwrap().value_kind().clone();
    (
        kind,
        Box::leak(schema.canonical_bytes().unwrap().into_boxed_slice()),
        value,
    )
}
#[test]
fn structured_payload_uses_original_activation_pressure_and_delivery_without_allocation() {
    let (kind, schema, value) = fixture();
    let pins = Pins::with_capacity(kind.as_str(), 512);
    let abi = || ingress_abi(kind.as_str(), 512);
    assert!(matches!(
        PrimitiveIngressSpec::checked(&pins.lines[0], abi()),
        Err(Refusal::UnsupportedAbi)
    ));
    let mut input = [0; 512];
    let mut output = [0; 512];
    let (bound, cost) = crate::member_return_tests::return_allocation_probe::observe(|| {
        let ingress = StructuredIngressSpec::checked(&pins.lines[0], abi(), schema).unwrap();
        let egress = EgressSpec::checked(&pins.lines[1], abi()).unwrap();
        let prepared =
            PreparedForeBridge::prepare([4; 32], ingress, egress, &mut input, &mut output).unwrap();
        // Inert test binding only; the public specification cannot activate it.
        unsafe { prepared.bind() }
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    let mut wrong = pins.activation();
    wrong.pins[0][0] ^= 1;
    assert!(matches!(bound.activate(wrong), Err(Refusal::Scope)));
    let (observer, port) = bound.activate(pins.activation()).unwrap();
    let mut observer = ForeObserver::new(Existing, observer);
    let mut kernel = kernel_with_capacity(16, 512);
    let mut hidden_bad = value.clone();
    *hidden_bad.last_mut().unwrap() = 2;
    let mut trailing = value.clone();
    trailing.push(0);
    let oversized = vec![0; 513];
    let (_, cost) = crate::member_return_tests::return_allocation_probe::observe(|| {
        for invalid in [&hidden_bad[..], &trailing, &value[..value.len() - 1]] {
            assert_eq!(port.stage_ingress(0, invalid), Err(Refusal::InvalidValue));
        }
        assert_eq!(port.stage_ingress(0, &oversized), Err(Refusal::Bounds));
        assert_eq!(port.stage_ingress(1, &value), Err(Refusal::Sequence));
        port.stage_ingress(0, &value).unwrap();
        assert_eq!(port.stage_ingress(0, &value), Err(Refusal::Busy));
        observer.before_step(&mut kernel).unwrap();
        delivered_input(&port, 0);
        kernel.step().unwrap();
        observer.before_step(&mut kernel).unwrap();
        port.with_egress(0, |actual| assert_eq!(actual, value))
            .unwrap();
        port.sent(Event::EgressOffered {
            sequence: 0,
            length: value.len(),
        })
        .unwrap();
        port.peer_accepted(0).unwrap();
        port.peer_delivered(0).unwrap();
        observer.before_step(&mut kernel).unwrap();
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    drop(observer);
    assert_eq!(port.stage_ingress(1, &value), Err(Refusal::Inactive));
}
#[test]
fn full_type_and_contract_envelope_are_pinned_independently_from_queue_capacity() {
    let (kind, schema, value) = fixture();
    let contract = c::CheckedValueContract::new(kind.clone(), value.len() as u32, vec![]).unwrap();
    let abi = || c::AssignedForeAbi {
        value_contract: Some(&contract),
        ..ingress_abi(kind.as_str(), 512)
    };
    let pins = Pins::from_abi(abi());
    let spec = StructuredIngressSpec::checked(&pins.lines[0], abi(), schema).unwrap();
    assert_eq!(spec.byte_capacity(), 512);
    assert!(IngressSpec::from(spec).validate(&value).is_ok());
    let mut larger = value.clone();
    larger.push(0);
    assert_eq!(
        IngressSpec::from(spec).validate(&larger),
        Err(Refusal::Bounds)
    );
    let foreign = Box::leak(
        c::StructuredInfoType::leaf(c::kind_id(c::BOOL_INFO_ID))
            .unwrap()
            .canonical_bytes()
            .unwrap()
            .into_boxed_slice(),
    );
    assert!(matches!(
        StructuredIngressSpec::checked(&pins.lines[0], abi(), foreign),
        Err(Refusal::UnsupportedAbi)
    ));
    let altered = c::AssignedForeAbi {
        byte_capacity: 513,
        ..abi()
    };
    assert!(matches!(
        StructuredIngressSpec::checked(&pins.lines[0], altered, schema),
        Err(Refusal::UnsupportedAbi)
    ));
}
