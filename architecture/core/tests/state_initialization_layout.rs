//! Preparation accounting is independent from canonical-value validity.
use conduit_core::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn absent() -> StructuredInfoValue {
    let value_type =
        optional_info_type(StructuredInfoType::leaf(kind_id(RESOURCE_REFERENCE_INFO_ID)).unwrap())
            .unwrap();
    StructuredInfoValue::variant(
        value_type,
        "none",
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID)).unwrap(),
            vec![],
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn primitive_and_absent_optional_preflight_cover_decoder_validator_overlap() {
    let primitive = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/bool")).unwrap(),
        vec![1],
    )
    .unwrap();
    for source in [primitive, absent()] {
        let encoded = source.canonical_bytes().unwrap();
        let (layout, observed) = allocation_probe::observe(|| {
            StructuredInfoValue::state_initialization_layout(&encoded)
        });
        let layout = layout.unwrap();
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
        let ((), decoded) = allocation_probe::observe(|| {
            let value = StructuredInfoValue::from_canonical_bytes(&encoded).unwrap();
            assert_eq!(value, source);
        });
        assert!(
            decoded.peak_bytes <= layout.decoded_peak_bytes,
            "{decoded:?}, {layout:?}"
        );
        let (retained_bytes, preparation) = allocation_probe::observe(|| {
            let value = StructuredInfoValue::from_canonical_bytes(&encoded).unwrap();
            match value.value_type().shape() {
                StructuredInfoTypeShape::Leaf(kind) => kind.clone().0.capacity(),
                _ => PreparedStructuredValueValidator::new(value.value_type(), 4096)
                    .unwrap()
                    .retained_backing_bytes()
                    .unwrap(),
            }
        });
        assert!(retained_bytes <= layout.validator_retained_bytes);
        assert!(
            preparation.peak_bytes <= layout.preparation_peak_bytes,
            "{preparation:?}, {layout:?}"
        );
        assert_eq!(preparation.reallocations, 0);
    }
}

#[test]
fn other_initial_values_keep_their_codec_but_not_this_preparation_profile() {
    let ty = optional_info_type(StructuredInfoType::leaf(kind_id("value/bool")).unwrap()).unwrap();
    let present = StructuredInfoValue::variant(
        ty,
        "some",
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id("value/bool")).unwrap(),
            vec![1],
        )
        .unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    assert!(StructuredInfoValue::from_canonical_bytes(&present).is_ok());
    let mut trailing = absent().canonical_bytes().unwrap();
    trailing.push(0);
    for bytes in [&present, &trailing] {
        let (layout, observed) =
            allocation_probe::observe(|| StructuredInfoValue::state_initialization_layout(bytes));
        assert!(layout.is_none());
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    }
}

#[test]
fn fresh_state_clone_storage_counts_actual_owners_and_discards_source_spare_capacity() {
    let mut name = String::with_capacity(1024);
    name.push_str("state");
    let mut initial = Vec::with_capacity(2048);
    initial.extend_from_slice(&[1, 2, 3]);
    let state = PlannedStateBoundary {
        state_id: name.into(),
        gear_id: "gear".into(),
        value_kind: "value/bool".into(),
        initial_value: Some(initial),
        lifetime: StateLifetime::Body,
        retained: None,
        maximum_value_bytes: 4,
        continuation: StateContinuation::ExternallyBounded,
    };
    let expected = state.local_clone_backing_bytes().unwrap();
    assert!(state.local_retained_backing_bytes().unwrap() > expected);
    let (cloned, observed) = allocation_probe::observe(|| state.clone());
    assert_eq!(cloned.local_retained_backing_bytes(), Some(expected));
    assert_eq!(observed.peak_bytes, expected);
    assert_eq!(observed.reallocations, 0);
}
