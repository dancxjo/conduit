use super::*;
fn program() -> PortableExpressionProgram {
    let ty = c::StructuredInfoType::leaf(c::kind_id(c::BOOL_INFO_ID)).unwrap();
    PortableExpressionProgram {
        input_type: ty.clone(),
        input_maximum_bytes: None,
        output_type: ty.clone(),
        root: conduit_plot::PortableExpressionNode {
            value_type: ty,
            operation: conduit_plot::PortableExpressionOperation::Input,
        },
    }
}
fn host() -> StdHost {
    StdHost::new_with_config(crate::StdHostConfig {
        host_id: "host/expression-selection".into(),
        boot_id: "boot/expression-selection".into(),
        offer_generation: c::OfferGeneration(1),
    })
}
#[test]
fn specialization_preserves_unrelated_payload_and_retires_only_owned_base() {
    let mut host = host();
    let before = host.advertisement.clone();
    let index = host
        .advertisement
        .capabilities
        .iter()
        .position(|offer| !offer.inputs.is_empty())
        .unwrap();
    let id = host.advertisement.capabilities[index].capability_id.clone();
    let payload = host.advertisement.capabilities[index].inputs.as_ptr();
    let requested = [(program(), c::PortTemporal::Value)];
    let evidence = host
        .select_expression_programs(&requested, 1_048_576)
        .unwrap();
    assert_eq!(evidence.offer_generation.0, before.offer_generation.0 + 1);
    assert!(evidence.descriptor_preparation_estimate_bytes > 0);
    assert_eq!(host.advertisement.resources, before.resources);
    for offer in &before.capabilities {
        assert!(host.advertisement.capabilities.contains(offer));
    }
    assert_eq!(
        host.advertisement
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == id)
            .unwrap()
            .inputs
            .as_ptr(),
        payload
    );
    let offer =
        conduit_std_offers::pure_expression_std_offer(&requested[0].0, requested[0].1).unwrap();
    assert!(host.advertisement.capabilities.contains(&offer));
    host.select_expression_programs(&[], 1_048_576).unwrap();
    assert_eq!(
        host.advertisement.capabilities.len(),
        before.capabilities.len()
    );
    assert!(before
        .capabilities
        .iter()
        .all(|offer| host.advertisement.capabilities.contains(offer)));
    assert_eq!(host.advertisement.resources, before.resources);
    assert_eq!(host.advertisement.bases, before.bases);
    assert_eq!(
        host.advertisement
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == id)
            .unwrap()
            .inputs
            .as_ptr(),
        payload
    );
    host.select_expression_programs(&requested, 1_048_576)
        .unwrap();
    assert_eq!(
        host.advertisement
            .capabilities
            .iter()
            .filter(|entry| entry.capability_id == offer.capability_id)
            .count(),
        1
    );
}
#[test]
fn duplicates_count_and_descriptor_limits_refuse_without_inventory_mutation() {
    let mut host = host();
    let before = host.advertisement.clone();
    let registry = host.base_registry.clone();
    let p = program();
    assert!(host
        .select_expression_programs(&[(p.clone(), c::PortTemporal::Value)], 1)
        .is_err());
    assert!(host
        .select_expression_programs(
            &[
                (p.clone(), c::PortTemporal::Value),
                (p.clone(), c::PortTemporal::Value)
            ],
            1_048_576
        )
        .unwrap_err()
        .contains("duplicate"));
    assert!(host
        .select_expression_programs(&vec![(p.clone(), c::PortTemporal::Value); 17], 1_048_576)
        .is_err());
    assert!(host
        .select_expression_programs(&[(p, c::PortTemporal::Value)], 1024)
        .is_err());
    assert_eq!(host.advertisement, before);
    assert_eq!(host.base_registry, registry);
    assert!(host.expression_program_base.is_none());
}
#[test]
fn canonical_count_matches_shared_encoding_and_invalid_program_refuses() {
    let mut host = host();
    let mut p = program();
    assert_eq!(
        p.canonical_byte_len().unwrap(),
        p.canonical_bytes().unwrap().len()
    );
    p.input_maximum_bytes = Some(c::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 + 1);
    let before = host.advertisement.clone();
    assert!(host
        .select_expression_programs(&[(p, c::PortTemporal::Value)], 1_048_576)
        .is_err());
    assert_eq!(host.advertisement, before);
}

#[test]
fn unknown_program_refuses_without_inventory_change() {
    let mut host = host();
    let mut p = program();
    p.root.operation = conduit_plot::PortableExpressionOperation::SemanticCall {
        kind: "test/not-a-known-expression".into(),
        arguments: Vec::new(),
    };
    let before = host.advertisement.clone();
    assert!(host
        .select_expression_programs(&[(p, c::PortTemporal::Value)], 1_048_576)
        .is_err());
    assert_eq!(host.advertisement, before);
}
