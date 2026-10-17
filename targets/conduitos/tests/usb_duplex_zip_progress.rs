//! Source/kernel conformance below USB physics: finite Zip is not a delayed gate.
use conduit_composite::FlowZipBack;
use conduit_core::{CheckedValueContract, StructuredInfoType, kind_id};
use conduit_kernel::{
    PortId, ValueRef,
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
};
use conduitos::protocol_source::{
    PreparedProtocolEntry, ProtocolSourcePackage, ProtocolSpecializationRequest,
    ProtocolValueReference,
};

#[test]
fn duplex_source_places_two_independent_feedback_zips_of_one_specialization() {
    let package = ProtocolSourcePackage::compile(
        include_str!("../plots/usb/control-endpoint-write-read.conduit").into(),
        &[ProtocolSpecializationRequest::FeedbackZip {
            left: ProtocolValueReference {
                type_name: "UsbTransferCompleted".into(),
                maximum_bytes: 256,
            },
            right: ProtocolValueReference {
                type_name: "UsbWriteReadRequests".into(),
                maximum_bytes: 4096,
            },
        }],
    )
    .unwrap();
    let entry = PreparedProtocolEntry::prepare(
        &serde_json::to_vec(&package).unwrap(),
        "usb-control-endpoint-write-read",
    )
    .unwrap();
    let zips: Vec<_> = entry
        .expanded()
        .expanded
        .gears
        .iter()
        .filter(|gear| gear.kind_id.as_str() == "flow/zip/feedback")
        .collect();
    assert_eq!(zips.len(), 2);
    assert_ne!(zips[0].gear_id, zips[1].gear_id);
}

#[test]
fn finite_zip_discards_an_early_closed_request_before_delayed_completion() {
    // Both product gate placements use this same generic Back. A request
    // producer can finish before the actual transfer completion exists.
    let ty = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let contract = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
    for _ in 0..2 {
        let mut zip =
            FlowZipBack::prepare_typed_finite(&contract, ty.clone(), &contract, ty.clone())
                .unwrap();
        let bytes = conduit_core::encode_count(1);
        let value = ValueRef {
            slot: 1,
            generation: 1,
            byte_len: bytes.len() as u32,
        };
        let mut io =
            StepIo::test_frame([None, Some(value)], [false; 2], [Some(256), None], None, 16);
        assert_eq!(
            zip.step(
                &mut io,
                &StepInputBytes::test_frame([None, Some(&bytes)], None)
            ),
            StepOutcome::Progress
        );
        assert!(io.test_consumed(PortId(1)));
        <FlowZipBack as StepBack<2>>::step_committed(&mut zip);
        let mut io = StepIo::test_frame([None; 2], [false, true], [Some(256), None], None, 16);
        assert_eq!(
            zip.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Complete
        );
        assert!(io.test_consumed_closed(PortId(1)));
        let mut io =
            StepIo::test_frame([Some(value), None], [false; 2], [Some(256), None], None, 16);
        assert_eq!(
            zip.step(
                &mut io,
                &StepInputBytes::test_frame([Some(&bytes), None], None)
            ),
            StepOutcome::Complete
        );
        assert!(!io.test_consumed(PortId(0)));
        assert!(<FlowZipBack as StepBack<2>>::prepared_output(&zip, PortId(0)).is_none());
    }
}

#[test]
fn feedback_zip_retains_closed_request_until_delayed_completion_then_emits_once() {
    let ty = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let contract = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
    let mut zip =
        FlowZipBack::prepare_typed_feedback(&contract, ty.clone(), &contract, ty).unwrap();
    let bytes = conduit_core::encode_count(1);
    let value = ValueRef {
        slot: 1,
        generation: 1,
        byte_len: bytes.len() as u32,
    };
    for (references, closed, inputs) in [
        (
            [None, Some(value)],
            [false; 2],
            [None, Some(bytes.as_slice())],
        ),
        ([None; 2], [false, true], [None; 2]),
        (
            [Some(value), None],
            [false; 2],
            [Some(bytes.as_slice()), None],
        ),
    ] {
        let mut io = StepIo::test_frame(references, closed, [Some(256), None], None, 16);
        assert_eq!(
            zip.step(&mut io, &StepInputBytes::test_frame(inputs, None)),
            StepOutcome::Progress
        );
        <FlowZipBack as StepBack<2>>::step_committed(&mut zip);
    }
    let mut pressured = StepIo::test_frame([None; 2], [false; 2], [None; 2], None, 16);
    assert_eq!(
        zip.step(&mut pressured, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Await
    );
    let mut io = StepIo::test_frame([None; 2], [false; 2], [Some(256), None], None, 16);
    assert_eq!(
        zip.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Progress
    );
    let encoder = conduit_core::PreparedTypedTuplePairEncoder::new(
        StructuredInfoType::leaf(kind_id("value/count")).unwrap(),
        8,
        StructuredInfoType::leaf(kind_id("value/count")).unwrap(),
        8,
    )
    .unwrap();
    let mut encoder = encoder;
    assert_eq!(
        <FlowZipBack as StepBack<2>>::prepared_output(&zip, PortId(0)).unwrap(),
        encoder.encode(&bytes, &bytes).unwrap()
    );
    <FlowZipBack as StepBack<2>>::step_committed(&mut zip);
    let mut io = StepIo::test_frame([None; 2], [true, false], [Some(256), None], None, 16);
    assert_eq!(
        zip.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Complete
    );
    assert!(<FlowZipBack as StepBack<2>>::prepared_output(&zip, PortId(0)).is_none());
}
