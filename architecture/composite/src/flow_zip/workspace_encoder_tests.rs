use super::*;
use conduit_core::{kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoValue};
fn fixtures() -> (StructuredInfoType, Vec<u8>, StructuredInfoType, [u8; 8]) {
    let leaf = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let record = StructuredInfoType::record(
        kind_id("test/zip-workspace"),
        vec![StructuredFieldType::new("number", leaf.clone()).unwrap()],
    )
    .unwrap();
    let bytes = StructuredInfoValue::record(
        record.clone(),
        vec![StructuredFieldValue::new(
            "number",
            StructuredInfoValue::leaf(leaf.clone(), 7_u64.to_le_bytes().to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    (record, bytes, leaf, 9_u64.to_le_bytes())
}
#[test]
fn workspace_pair_preserves_exact_primitive_and_structured_framing_without_allocations() {
    let (record, bytes, leaf, right) = fixtures();
    let mut original =
        PreparedTypedTuplePairEncoder::new(record, bytes.len() as u32, leaf, 8).unwrap();
    let expected = original.encode(&bytes, &right).unwrap().to_vec();
    let shared = WorkspacePair::prepare(&original, [bytes.len() as u32, 8]).unwrap();
    let mut output = vec![0; shared.maximum() as usize];
    let allocations = crate::test_support::allocation::allocations_during(|| {
        for _ in 0..16 {
            assert_eq!(
                shared.encode_into([&bytes, &right], &mut output).unwrap(),
                expected
            );
        }
    });
    assert_eq!(allocations, 0);
    assert_eq!(
        shared.input_schema(0),
        original.into_runtime().canonical_input_schema(0)
    );
    assert_eq!(
        shared.input_schema(1),
        None,
        "primitive ports keep their raw ABI"
    );
    for bad in [&right[..7], &right[..0]] {
        output.fill(0xa5);
        assert!(shared.encode_into([&bytes, bad], &mut output).is_err());
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }
    let mut wrong = bytes.clone();
    wrong[0] ^= 1;
    assert!(shared.encode_into([&wrong, &right], &mut output).is_err());
    assert!(shared
        .encode_into([&bytes, &right], &mut output[..expected.len() - 1])
        .is_err());
}
#[test]
fn pending_sides_survive_missing_workspace_and_cancel_retires_them() {
    let (record, bytes, leaf, right) = fixtures();
    let left_contract = CheckedValueContract::new(
        record.profile().unwrap().value_kind().clone(),
        bytes.len() as u32,
        vec![],
    )
    .unwrap();
    let right_contract = CheckedValueContract::new(kind_id("value/u64"), 8, vec![]).unwrap();
    let mut back =
        FlowZipBack::prepare_typed_finite_workspace(&left_contract, record, &right_contract, leaf)
            .unwrap();
    for (side, data) in [bytes.as_slice(), right.as_slice()].into_iter().enumerate() {
        let mut refs = [None; 2];
        let mut values = [None; 2];
        refs[side] = Some(conduit_kernel::ValueRef {
            slot: side as u16,
            generation: 1,
            byte_len: data.len() as u32,
        });
        values[side] = Some(data);
        let mut io = StepIo::test_frame(refs, [false; 2], [None; 2], None, 16);
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame(values, None)),
            StepOutcome::Progress
        );
        <FlowZipBack as StepBack<2>>::step_committed(&mut back);
    }
    let pending = (back.left_len, back.right_len);
    assert_eq!(pending, (Some(bytes.len()), Some(8)));
    let mut ready = StepIo::test_frame([None; 2], [false; 2], [Some(4096), None], None, 16);
    assert!(matches!(
        back.step(&mut ready, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Fail(_)
    ));
    assert_eq!((back.left_len, back.right_len), pending);
    assert!(!back.output_staged && !back.finish_after_commit);
    <FlowZipBack as StepBack<2>>::cancel(&mut back);
    assert_eq!((back.left_len, back.right_len), (None, None));
    assert!(<FlowZipBack as StepBack<2>>::prepared_output(&back, PortId(0)).is_none());
}
#[test]
fn accounting_separates_composed_output_from_pending_sides_and_schema_storage() {
    let (record, bytes, leaf, _) = fixtures();
    let original = PreparedTypedTuplePairEncoder::new(record, bytes.len() as u32, leaf, 8).unwrap();
    let shared = WorkspacePair::prepare(&original, [bytes.len() as u32, 8]).unwrap();
    let schemas: usize =
        shared.schemas.iter().map(Vec::capacity).sum::<usize>() + shared.output_schema.capacity();
    std::eprintln!("Zip host ABI: outer owner unchanged={}B; old boxed runtime encoder inline={}B, workspace encoder inline={}B; new retained canonical schema capacities={}B; removed composed-output allocation at least={}B per owner; shared max={}B, left/right pending and kernel pool remain. Full dynamic old encoder/allocator total not measured by this receipt.", core::mem::size_of::<FlowZipBack>(), core::mem::size_of::<conduit_core::PreparedTypedTuplePairRuntimeEncoder>(), core::mem::size_of::<WorkspacePair>(), schemas, shared.maximum(), shared.maximum());
    assert_eq!(shared.maximum(), original.maximum_encoded_bytes());
}
