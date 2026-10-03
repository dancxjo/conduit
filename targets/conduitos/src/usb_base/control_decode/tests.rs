use super::*;
use alloc::vec;
use conduit_core::{
    StructuredFieldType, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, kind_id,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};

fn encoded_request(
    request_type: &StructuredInfoType,
    setup: [u8; 8],
    output: &[u8],
) -> alloc::vec::Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = request_type.shape() else {
        panic!("request record")
    };
    let field = |name| {
        fields
            .iter()
            .find(|field| field.name() == name)
            .unwrap()
            .value_type()
            .clone()
    };
    let octet = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    StructuredInfoValue::record(
        request_type.clone(),
        vec![
            StructuredFieldValue::new(
                "setup",
                StructuredInfoValue::leaf(field("setup"), setup.to_vec()).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "output",
                StructuredInfoValue::sequence(
                    field("output"),
                    output
                        .iter()
                        .map(|byte| StructuredInfoValue::leaf(octet.clone(), vec![*byte]).unwrap())
                        .collect(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

fn encoded(contract: &ControlContract, setup: [u8; 8], output: &[u8]) -> alloc::vec::Vec<u8> {
    encoded_request(contract.request_type(), setup, output)
}

#[test]
fn typed_control_plot_checks_and_expands_to_one_class_neutral_kind() {
    let contract = ControlContract::prepare().unwrap();
    let (startup, profile) = contract.catalogs();
    let syntax = parse_syntax_document(include_str!("../../../plots/usb/control.conduit"));
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "usb-control-call", &profile)
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1);
    assert_eq!(expanded.gears[0].kind_id, contract.kind().kind_id);
    assert_eq!(
        contract.kind().inputs[0].value_kind,
        *contract.request_type().profile().unwrap().value_kind()
    );
    assert_eq!(
        contract.kind().outputs[0].value_kind,
        *contract.result_type().profile().unwrap().value_kind()
    );
    assert_eq!(contract.kind().value_contracts().len(), 2);
    assert_eq!(contract.kind().limits.max_queue_items, 1);
    let StructuredInfoTypeShape::Variant { cases, .. } = contract.result_type().shape() else {
        panic!("typed result")
    };
    assert_eq!(
        cases
            .iter()
            .map(|case| case.tag())
            .collect::<alloc::vec::Vec<_>>(),
        ["completed", "provider-lost", "stalled", "unsupported"]
    );
}

#[test]
fn exact_typed_request_decodes_every_native_payload_size() {
    let contract = ControlContract::prepare().unwrap();
    let decoder = PreparedControlRequestDecoder::new(&contract).unwrap();
    let bytes: [u8; 256] = core::array::from_fn(|i| i as u8);
    for length in 0..=256_u16 {
        let mut setup = [0; 8];
        setup[1] = 0xff; // uninterpreted opcode, no hidden class policy
        setup[6..].copy_from_slice(&length.to_le_bytes());
        let input = encoded(&contract, setup, &bytes[..usize::from(length)]);
        let decoded = decoder.decode(&input).unwrap();
        let request = decoded.request(256).unwrap();
        assert_eq!(request.setup(), &setup);
        assert_eq!(request.output(), &bytes[..usize::from(length)]);
        if length > 0 {
            assert_eq!(
                decoded.request(length - 1).err(),
                Some(ControlRequestRefusal::DataEnvelope)
            );
        }
    }
    let mut setup = [0; 8];
    setup[0] = 128;
    setup[6..].copy_from_slice(&256_u16.to_le_bytes());
    let input = encoded(&contract, setup, &[]);
    for _ in 0..10_000 {
        let decoded = decoder.decode(&input).unwrap();
        assert!(decoded.request(256).unwrap().input());
    }
}

#[test]
fn canonical_type_geometry_and_trailing_data_refuse_independently() {
    let contract = ControlContract::prepare().unwrap();
    let decoder = PreparedControlRequestDecoder::new(&contract).unwrap();
    let mut setup = [0; 8];
    setup[6] = 2;
    let input = encoded(&contract, setup, &[7]);
    assert!(matches!(
        decoder.decode(&input),
        Err(ControlDecodeRefusal::Transfer(
            ControlRequestRefusal::OutputLengthMismatch
        ))
    ));
    setup[0] = 128;
    let input = encoded(&contract, setup, &[7]);
    assert!(matches!(
        decoder.decode(&input),
        Err(ControlDecodeRefusal::Transfer(
            ControlRequestRefusal::UnexpectedOutputData
        ))
    ));
    let valid = encoded(&contract, setup, &[]);
    for prefix in 0..valid.len() {
        assert!(matches!(
            decoder.decode(&valid[..prefix]),
            Err(ControlDecodeRefusal::Canonical(_))
        ));
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(matches!(
        decoder.decode(&trailing),
        Err(ControlDecodeRefusal::Canonical(_))
    ));
    let forged_type = StructuredInfoType::record(
        kind_id("forged/schema"),
        vec![
            StructuredFieldType::new(
                "setup",
                StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
            )
            .unwrap(),
            StructuredFieldType::new(
                "output",
                StructuredInfoType::sequence(
                    StructuredInfoType::leaf(kind_id("value/u8")).unwrap(),
                    256,
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    assert_eq!(
        decoder
            .decode(&encoded_request(&forged_type, setup, &[]))
            .err(),
        Some(ControlDecodeRefusal::Canonical(
            StructuredInfoRefusal::WrongType
        ))
    );
}
