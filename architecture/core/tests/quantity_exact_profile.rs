use conduit_core::*;

#[test]
fn checked_exact_leaf_refuses_wrong_version_quantum_width_and_noncanonical_bytes() {
    assert_eq!(
        primitive_info_kind(QUANTITY_INFO_ID),
        Some(PrimitiveInfoKind::Quantity)
    );
    let ty = StructuredInfoType::leaf(kind_id(QUANTITY_INFO_ID)).unwrap();
    let contract = CheckedValueContract::new(
        kind_id(QUANTITY_INFO_ID),
        MAX_QUANTITY_ENCODED_BYTES as u32,
        vec![],
    )
    .unwrap();
    for source in ["1Qm", "1qm", "1Qm³", "1qm³", "1um²", "-1uW", "273.15K"] {
        let exact = Quantity::parse_plot_literal(source).unwrap();
        assert_eq!(
            validate_primitive_info(QUANTITY_INFO_ID, &exact.encode()),
            Ok(())
        );
        assert!(StructuredInfoValue::leaf(ty.clone(), exact.encode().to_vec()).is_ok());
        assert!(contract.validate(&exact.encode()).is_ok());
        assert!(validate_primitive_info(QUANTITY_INFO_ID, &[0; 9]).is_err());
    }
    let narrow = [0; 9];
    assert!(StructuredInfoValue::leaf(ty.clone(), narrow.to_vec()).is_err());
    let canonical = Quantity::from_decimal(1, 0, Unit::Meter).unwrap().encode();
    let mut forged = canonical.clone();
    forged[0] = 1;
    assert!(contract.validate(&forged).is_err());
    let mut forged = canonical.clone();
    *forged.last_mut().unwrap() ^= 1;
    assert!(contract.validate(&forged).is_err());
    let mut forged = canonical;
    forged.push(0);
    assert!(contract.validate(&forged).is_err());
    assert!(StructuredInfoValue::leaf(ty, forged).is_err());
}
