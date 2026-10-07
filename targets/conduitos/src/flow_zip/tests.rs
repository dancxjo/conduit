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

#[test]
fn explicit_frame_profile_admits_larger_inputs_but_preserves_total_pair_bound() {
    let ty = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let small = CheckedValueContract::new(kind_id("value/u64"), 1000, alloc::vec![]).unwrap();
    let frame = CheckedValueContract::new(kind_id("value/u64"), 9962, alloc::vec![]).unwrap();
    let mut default = FlowZipOperationFactory::default();
    assert!(default.install(&small, &ty, &frame, &ty).is_err());
    let mut larger = FlowZipOperationFactory::frame16k();
    let admitted = larger.install(&small, &ty, &frame, &ty).unwrap();
    assert_eq!(
        admitted.implementation.implementation_id.as_str(),
        FRAME16K_IMPLEMENTATION
    );
    assert_eq!(
        admitted.implementation.execution_profile_id.as_str(),
        FRAME16K_PROFILE
    );
    let paired = admitted
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|v| v.location == FrontValueLocation::Output(port_id("paired")))
        .unwrap();
    assert!(paired.contract.maximum_bytes <= MAXIMUM_PAIR_BYTES);
    assert!(larger.install(&frame, &ty, &frame, &ty).is_err());
    let excessive =
        CheckedValueContract::new(kind_id("value/u64"), MAXIMUM_PAIR_BYTES + 1, alloc::vec![])
            .unwrap();
    assert!(larger.install(&excessive, &ty, &small, &ty).is_err());
    let foreign = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    assert!(larger.install(&small, &foreign, &frame, &ty).is_err());
    assert_eq!(larger.offers().count(), 1);
}

#[test]
fn default_and_frame_profiles_have_distinct_custody_for_the_same_schema() {
    let ty = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let value = CheckedValueContract::new(kind_id("value/u64"), 8, alloc::vec![]).unwrap();
    let ordinary = FlowZipOperationFactory::default()
        .install(&value, &ty, &value, &ty)
        .unwrap();
    let large = FlowZipOperationFactory::frame16k()
        .install(&value, &ty, &value, &ty)
        .unwrap();
    assert_eq!(
        ordinary.capability_id.as_str(),
        "conduitos/flow-zip-finite/value/u64/8/value/u64/8@1"
    );
    assert_ne!(ordinary.capability_id, large.capability_id);
    assert_ne!(
        ordinary.implementation.implementation_id,
        large.implementation.implementation_id
    );
    assert_ne!(
        ordinary.implementation.execution_profile_id,
        large.implementation.execution_profile_id
    );
    assert_eq!(ordinary.semantic_contract, large.semantic_contract);
}

#[test]
fn specialized_factory_retains_multiple_exact_kinds_and_feedback_modes() {
    let wide = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let narrow = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let left = CheckedValueContract::new(kind_id("value/u64"), 8, alloc::vec![]).unwrap();
    let right = CheckedValueContract::new(kind_id("value/u8"), 1, alloc::vec![]).unwrap();
    let mut factory = FlowZipOperationFactory::frame16k();
    let first = factory
        .install_specialized(&left, &wide, &right, &narrow)
        .unwrap();
    let second = factory
        .install_specialized(&right, &narrow, &left, &wide)
        .unwrap();
    let feedback = factory
        .install_feedback_specialized(&left, &wide, &right, &narrow)
        .unwrap();
    assert_ne!(first.kind_id, second.kind_id);
    assert_ne!(first.kind_id, feedback.kind_id);
    assert_eq!(factory.offers().count(), 3);
    assert!(
        factory
            .install_specialized(&left, &wide, &right, &narrow)
            .is_err()
    );
    assert!(
        factory
            .install_specialized(&left, &narrow, &right, &narrow)
            .is_err()
    );
    assert_eq!(factory.offers().count(), 3);
    assert!(!factory.pairs[&first.capability_id].feedback);
    assert!(factory.pairs[&feedback.capability_id].feedback);
    assert_eq!(
        first.kind_contract_revision.as_str(),
        conduit_semantic_catalog::FLOW_ZIP_FINITE_SPECIALIZED_REVISION
    );
    assert_eq!(
        feedback.kind_contract_revision.as_str(),
        conduit_semantic_catalog::FLOW_ZIP_FEEDBACK_SPECIALIZED_REVISION
    );
}
