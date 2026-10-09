use conduit_core::*;

#[test]
fn checked_exact_leaf_refuses_wrong_version_quantum_width_and_noncanonical_bytes() {
    assert_eq!(
        primitive_info_kind(EXACT_DECIMAL_QUANTITY_INFO_ID),
        Some(PrimitiveInfoKind::ExactDecimalQuantity)
    );
    let ty = StructuredInfoType::leaf(kind_id(EXACT_DECIMAL_QUANTITY_INFO_ID)).unwrap();
    let contract =
        CheckedValueContract::new(kind_id(EXACT_DECIMAL_QUANTITY_INFO_ID), 20, vec![]).unwrap();
    for source in ["1Qm", "1qm", "1Qm³", "1qm³", "1um2", "-1uW", "273.15K"] {
        let exact = ExactDecimalQuantity::parse_plot_literal(source).unwrap();
        assert_eq!(
            validate_primitive_info(EXACT_DECIMAL_QUANTITY_INFO_ID, &exact.encode()),
            Ok(())
        );
        assert!(StructuredInfoValue::leaf(ty.clone(), exact.encode().to_vec()).is_ok());
        assert!(contract.validate(&exact.encode()).is_ok());
        assert!(validate_primitive_info(QUANTITY_INFO_ID, &exact.encode()).is_err());
    }
    let legacy = Quantity::new(1, QuantityUnit::Meter).encode();
    assert!(StructuredInfoValue::leaf(ty.clone(), legacy.to_vec()).is_err());
    let mut forged = ExactDecimalQuantity::new(1, 0, QuantityUnit::Meter)
        .unwrap()
        .encode();
    forged[0] = 2;
    assert!(contract.validate(&forged).is_err());
    forged[0] = 1;
    forged[2..4].copy_from_slice(&129_i16.to_le_bytes());
    assert!(contract.validate(&forged).is_err());
    forged[2..4].copy_from_slice(&0_i16.to_le_bytes());
    forged[4..].copy_from_slice(&10_i128.to_le_bytes());
    assert!(contract.validate(&forged).is_err());
    assert!(StructuredInfoValue::leaf(ty, forged.to_vec()).is_err());
}
