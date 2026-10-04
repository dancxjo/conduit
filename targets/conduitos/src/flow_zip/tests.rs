use super::*;

#[test]
fn prepared_offer_count_is_finite_and_failed_installations_preserve_it() {
    let ty = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let right = CheckedValueContract::new(kind_id("value/u64"), 8, alloc::vec![]).unwrap();
    let mut factory = FlowZipOperationFactory::default();
    let first = factory.install(&right, &ty, &right, &ty).unwrap();
    assert!(factory.install(&right, &ty, &right, &ty).is_err());
    assert_eq!(factory.offers().count(), 1);
    assert_eq!(factory.offers().next().unwrap(), &first);
    for index in 1..MAXIMUM_SPECIALIZATIONS {
        let left = CheckedValueContract::new(kind_id("value/u64"), 8 + index as u32, alloc::vec![])
            .unwrap();
        factory.install(&left, &ty, &right, &ty).unwrap();
    }
    let next = CheckedValueContract::new(
        kind_id("value/u64"),
        8 + MAXIMUM_SPECIALIZATIONS as u32,
        alloc::vec![],
    )
    .unwrap();
    assert!(factory.install(&next, &ty, &right, &ty).is_err());
    assert_eq!(factory.offers().count(), MAXIMUM_SPECIALIZATIONS);
}

#[test]
fn unsupported_input_envelopes_and_foreign_schemas_never_become_offers() {
    let ty = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let other = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let contract = CheckedValueContract::new(kind_id("value/u64"), 8, alloc::vec![]).unwrap();
    let too_large =
        CheckedValueContract::new(kind_id("value/u64"), MAXIMUM_INPUT_BYTES + 1, alloc::vec![])
            .unwrap();
    let mut factory = FlowZipOperationFactory::default();
    assert!(factory.install(&too_large, &ty, &contract, &ty).is_err());
    assert!(factory.install(&contract, &other, &contract, &ty).is_err());
    assert_eq!(factory.offers().count(), 0);
}
