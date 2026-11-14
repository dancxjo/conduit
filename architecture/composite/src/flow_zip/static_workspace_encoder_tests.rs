use super::*;
use conduit_core::{kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoValue};
fn fixture() -> (
    StructuredInfoType,
    Vec<u8>,
    StructuredInfoType,
    [&'static [u8]; 3],
) {
    let leaf = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let record = StructuredInfoType::record(
        kind_id("test/zip-static"),
        vec![StructuredFieldType::new("number", leaf.clone()).unwrap()],
    )
    .unwrap();
    let bytes = StructuredInfoValue::record(
        record.clone(),
        vec![StructuredFieldValue::new(
            "number",
            StructuredInfoValue::leaf(leaf.clone(), 7u64.to_le_bytes().to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let pair =
        PreparedTypedTuplePairEncoder::new(record.clone(), bytes.len() as u32, leaf.clone(), 8)
            .unwrap();
    let schemas = [
        record.canonical_bytes().unwrap(),
        leaf.canonical_bytes().unwrap(),
        pair.value_type().canonical_bytes().unwrap(),
    ]
    .map(|bytes| &*Box::leak(bytes.into_boxed_slice()));
    (record, bytes, leaf, schemas)
}
#[test]
fn linked_pair_matches_owned_values_and_refusals_without_execution_allocations() {
    let (record, bytes, leaf, schemas) = fixture();
    let mut original =
        PreparedTypedTuplePairEncoder::new(record, bytes.len() as u32, leaf, 8).unwrap();
    let shared = StaticWorkspacePair::prepare(&original, [bytes.len() as u32, 8], schemas).unwrap();
    #[cfg(feature = "schema-value-storage")]
    {
        assert!(core::ptr::eq(shared.output_schema(), schemas[2]));
        assert!(core::ptr::eq(shared.input_schema(0).unwrap(), schemas[0]));
        assert_eq!(shared.input_schema(1), None);
    }
    let expected = original
        .encode(&bytes, &9u64.to_le_bytes())
        .unwrap()
        .to_vec();
    let mut output = vec![0; shared.maximum() as usize];
    assert_eq!(
        crate::test_support::allocation::allocations_during(|| {
            for _ in 0..16 {
                assert_eq!(
                    shared
                        .encode_into([&bytes, &9u64.to_le_bytes()], &mut output)
                        .unwrap(),
                    expected
                );
            }
        }),
        0
    );
    for invalid in [&[][..], &[0; 7][..], &[0; 9][..]] {
        output.fill(0xa5);
        assert!(shared.encode_into([&bytes, invalid], &mut output).is_err());
        assert!(output.iter().all(|n| *n == 0xa5));
    }
    output.fill(0xa5);
    assert!(shared
        .encode_into(
            [&bytes, &9u64.to_le_bytes()],
            &mut output[..expected.len() - 1]
        )
        .is_err());
    assert!(output.iter().all(|n| *n == 0xa5));
    let mut wrong = bytes.clone();
    wrong[0] ^= 1;
    assert!(shared
        .encode_into([&wrong, &9u64.to_le_bytes()], &mut output)
        .is_err());
    for index in 0..3 {
        let mut altered = schemas;
        altered[index] = &[0];
        assert!(StaticWorkspacePair::prepare(&original, [bytes.len() as u32, 8], altered).is_err());
        altered[index] = schemas[(index + 1) % 3];
        assert!(StaticWorkspacePair::prepare(&original, [bytes.len() as u32, 8], altered).is_err());
    }
}
#[test]
fn static_zip_keeps_original_pending_sizes_and_feedback_lifecycle_flags() {
    let (record, bytes, leaf, schemas) = fixture();
    let left = CheckedValueContract::new(
        record.profile().unwrap().value_kind().clone(),
        bytes.len() as u32,
        vec![],
    )
    .unwrap();
    let right = CheckedValueContract::new(kind_id("value/u64"), 8, vec![]).unwrap();
    for feedback in [false, true] {
        let ordinary = if feedback {
            FlowZipBack::prepare_typed_feedback_workspace(
                &left,
                record.clone(),
                &right,
                leaf.clone(),
            )
        } else {
            FlowZipBack::prepare_typed_finite_workspace(&left, record.clone(), &right, leaf.clone())
        }
        .unwrap();
        let linked = if feedback {
            FlowZipBack::prepare_typed_feedback_static_workspace(
                &left,
                record.clone(),
                &right,
                leaf.clone(),
                schemas,
            )
        } else {
            FlowZipBack::prepare_typed_finite_static_workspace(
                &left,
                record.clone(),
                &right,
                leaf.clone(),
                schemas,
            )
        }
        .unwrap();
        assert_eq!(
            (linked.left.len(), linked.right.len(), linked.output_maximum),
            (
                ordinary.left.len(),
                ordinary.right.len(),
                ordinary.output_maximum
            )
        );
        assert_eq!(
            (linked.finite, linked.feedback, linked.return_owed),
            (ordinary.finite, ordinary.feedback, ordinary.return_owed)
        );
        assert!(matches!(
            linked.encoder,
            encoder::PairEncoder::StaticWorkspace(_)
        ));
    }
}

#[test]
fn static_pending_sides_survive_missing_workspace_and_cancel_retires_them() {
    let (record, bytes, leaf, schemas) = fixture();
    let left = CheckedValueContract::new(
        record.profile().unwrap().value_kind().clone(),
        bytes.len() as u32,
        vec![],
    )
    .unwrap();
    let right = CheckedValueContract::new(kind_id("value/u64"), 8, vec![]).unwrap();
    let mut back =
        FlowZipBack::prepare_typed_finite_static_workspace(&left, record, &right, leaf, schemas)
            .unwrap();
    let right = 9u64.to_le_bytes();
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
    <FlowZipBack as StepBack<2>>::cancel(&mut back);
    assert_eq!((back.left_len, back.right_len), (None, None));
    assert!(<FlowZipBack as StepBack<2>>::prepared_output(&back, PortId(0)).is_none());
}
