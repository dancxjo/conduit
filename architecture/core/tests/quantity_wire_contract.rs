//! Exact per-port realization facts, without activating optional wire values.
use conduit_core::*;
use QuantityValueProfile::{Decimal, Exact};

#[test]
fn wire_contract_roundtrips_with_stable_identity_and_port_local_profiles() {
    let contract = QuantityWireContract::new(
        vec![("z".into(), Exact), ("a".into(), Decimal)],
        vec![("result".into(), Exact)],
    )
    .unwrap();
    let property = contract.to_property().unwrap();
    assert_eq!(property.profile(), &QuantityWireContract::property_kind());
    assert_eq!(
        QuantityWireContract::from_property(&property),
        Ok(contract.clone())
    );
    assert_eq!(contract.input(&"a".into()), Decimal);
    assert_eq!(contract.input(&"z".into()), Exact);
    assert_eq!(contract.input(&"result".into()), Decimal);
    assert_eq!(contract.output(&"result".into()), Exact);
    assert_eq!(contract.output(&"a".into()), Decimal);
    let reordered = QuantityWireContract::new(
        vec![("a".into(), Decimal), ("z".into(), Exact)],
        vec![("result".into(), Exact)],
    )
    .unwrap();
    assert_eq!(reordered.to_property(), Ok(property));
    let empty = QuantityWireContract::default();
    assert_eq!(
        QuantityWireContract::from_property(&empty.to_property().unwrap()),
        Ok(empty)
    );
}

#[test]
fn duplicates_invalid_ports_and_excessive_contracts_refuse_before_encoding() {
    assert_eq!(
        QuantityWireContract::new(vec![("x".into(), Exact), ("x".into(), Decimal)], vec![]),
        Err(QuantityWireContractRefusal::DuplicatePort)
    );
    assert_eq!(
        QuantityWireContract::new(vec![("".into(), Exact)], vec![]),
        Err(QuantityWireContractRefusal::InvalidPort)
    );
    assert_eq!(
        QuantityWireContract::new(
            vec![("x".repeat(MAXIMUM_STRUCTURED_NAME_BYTES + 1).into(), Exact)],
            vec![]
        ),
        Err(QuantityWireContractRefusal::InvalidPort)
    );
    let ports = (0..=MAXIMUM_QUANTITY_WIRE_PORTS)
        .map(|i| (format!("p{i}").into(), Exact))
        .collect();
    assert_eq!(
        QuantityWireContract::new(ports, vec![]),
        Err(QuantityWireContractRefusal::TooManyPorts)
    );
}

#[test]
fn malformed_domain_codes_and_foreign_properties_cannot_grant_exact_eligibility() {
    let contract = QuantityWireContract::new(vec![], vec![("result".into(), Exact)]).unwrap();
    let property = contract.to_property().unwrap();
    let mut encoded = property.canonical_value().to_vec();
    assert_eq!(encoded.last(), Some(&2));
    *encoded.last_mut().unwrap() = 9;
    let forged = StructuredConfigurationValue::new(property.profile().clone(), encoded).unwrap();
    assert_eq!(
        QuantityWireContract::from_property(&forged),
        Err(QuantityWireContractRefusal::InvalidProperty)
    );
    let foreign = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/u8")).unwrap(),
        vec![2],
    )
    .unwrap();
    let foreign = StructuredConfigurationValue::new(
        foreign.value_type().profile().unwrap().value_kind().clone(),
        foreign.canonical_bytes().unwrap(),
    )
    .unwrap();
    assert_eq!(
        QuantityWireContract::from_property(&foreign),
        Err(QuantityWireContractRefusal::InvalidProperty)
    );
}
