use super::{
    contract::I2cContract, decode::PreparedI2cDecoder, result::PreparedI2cResultEncoder,
    transaction::I2cDisposition,
};
use alloc::{vec, vec::Vec};
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue, kind_id,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};

pub(super) fn request(contract: &I2cContract, address: u8, write: &[u8], read: u8) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
        panic!("request");
    };
    let ty = |name| {
        fields
            .iter()
            .find(|f| f.name() == name)
            .unwrap()
            .value_type()
            .clone()
    };
    let octet = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    StructuredInfoValue::record(
        contract.request_type().clone(),
        vec![
            StructuredFieldValue::new(
                "address",
                StructuredInfoValue::leaf(ty("address"), vec![address]).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "read",
                StructuredInfoValue::leaf(ty("read"), vec![read]).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "write",
                StructuredInfoValue::sequence(
                    ty("write"),
                    write
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

#[test]
fn checked_i2c_topology_has_one_class_neutral_call() {
    let contract = I2cContract::prepare().unwrap();
    let (startup, profile) = contract.catalogs();
    let syntax = parse_syntax_document(include_str!(
        "../../../../plots/device-protocols/i2c.conduit"
    ));
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "i2c-transaction", &profile)
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1);
    assert_eq!(expanded.gears[0].kind_id, contract.kind().kind_id);
    // Canonical Type identity owns schema constraints; transport byte bounds belong
    // to the selected Host Call, not an extra authored Fore refinement.
    assert!(contract.kind().value_contracts().is_empty());
    assert_eq!(contract.kind().limits.max_queue_items, 1);
}

#[test]
fn bounded_request_decoding_is_exact_and_does_not_interpret_registers() {
    let contract = I2cContract::prepare().unwrap();
    let decoder = PreparedI2cDecoder::new(&contract).unwrap();
    let bytes = core::array::from_fn::<_, 32, _>(|i| i as u8);
    for length in 0..=32 {
        let encoded = request(&contract, 0x76, &bytes[..length], 32);
        let decoded = decoder.decode(&encoded).unwrap();
        let transaction = decoded.transaction().unwrap();
        assert_eq!(transaction.address(), 0x76);
        assert_eq!(transaction.write(), &bytes[..length]);
        assert_eq!(transaction.read_length(), 32);
    }
    let valid = request(&contract, 0x76, &[0xff], 1);
    for length in 0..valid.len() {
        assert!(decoder.decode(&valid[..length]).is_err());
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(decoder.decode(&trailing).is_err());
    let empty = request(&contract, 0x76, &[], 0);
    assert!(decoder.decode(&empty).is_err());
}

#[test]
fn actual_shortness_and_bus_dispositions_remain_distinct() {
    let contract = I2cContract::prepare().unwrap();
    let mut encoder = PreparedI2cResultEncoder::new(&contract).unwrap();
    let tag = |bytes: &[u8]| {
        let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
        let conduit_core::StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
            panic!("variant");
        };
        alloc::string::String::from(tag)
    };
    for length in 0..=32 {
        assert_eq!(
            tag(encoder
                .completed(length, &vec![7; usize::from(length)])
                .unwrap()),
            "completed"
        );
        if length > 0 {
            assert_eq!(
                tag(encoder
                    .completed(length, &vec![7; usize::from(length) - 1])
                    .unwrap()),
                "short"
            );
        }
    }
    assert!(encoder.completed(0, &[7]).is_err());
    assert!(encoder.completed(33, &[]).is_err());
    for (outcome, expected) in [
        (I2cDisposition::DeviceError, "device-error"),
        (I2cDisposition::NotAcknowledged, "not-acknowledged"),
        (I2cDisposition::ArbitrationLost, "arbitration-lost"),
        (I2cDisposition::TimedOut, "timed-out"),
        (I2cDisposition::ProviderLost, "provider-lost"),
        (I2cDisposition::StaleAttachment, "stale-attachment"),
        (I2cDisposition::Unsupported, "unsupported"),
        (I2cDisposition::Refused, "refused"),
    ] {
        assert_eq!(tag(encoder.disposition(outcome).unwrap()), expected);
    }
}

#[test]
fn register_idioms_construct_exact_bounded_requests_as_ordinary_plots() {
    let contract = I2cContract::prepare().unwrap();
    let (startup, profile) = contract.catalogs();
    let syntax = parse_syntax_document(include_str!(
        "../../../../plots/device-protocols/i2c-register.conduit"
    ));
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    for name in ["i2c-register-read", "i2c-register-write"] {
        let expanded = expand_canonical_plot_for_authoring(&checked, name, &profile)
            .unwrap()
            .expanded;
        assert_eq!(expanded.gears.len(), 2);
        assert!(
            expanded
                .gears
                .iter()
                .any(|g| g.kind_id == contract.kind().kind_id)
        );
    }
    for name in ["i2c-register-read-request", "i2c-register-write-request"] {
        let expanded = expand_canonical_plot_for_authoring(&checked, name, &profile)
            .unwrap()
            .expanded;
        let conduit_core::ConfigurationValue::Text(encoded) =
            &expanded.gears[0].configuration[0].value
        else {
            panic!("expression")
        };
        let p = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        assert_eq!(&p.output_type, contract.request_type());
        let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&p).unwrap();
        let decoder = PreparedI2cDecoder::new(&contract).unwrap();
        let StructuredInfoTypeShape::Record { fields, .. } = p.input_type.shape() else {
            panic!("query")
        };
        for register in 0..=255_u8 {
            let input = StructuredInfoValue::record(
                p.input_type.clone(),
                fields
                    .iter()
                    .map(|field| {
                        let byte = match field.name() {
                            "address" => 0x76,
                            "register" => register,
                            "value" => 0xa5,
                            _ => panic!("field"),
                        };
                        StructuredFieldValue::new(
                            field.name(),
                            StructuredInfoValue::leaf(field.value_type().clone(), vec![byte])
                                .unwrap(),
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap()
            .canonical_bytes()
            .unwrap();
            let encoded = prepared.evaluate(&input).unwrap();
            assert_eq!(encoded, p.evaluate(&input).unwrap());
            let decoded = decoder.decode(encoded).unwrap();
            let request = decoded.transaction().unwrap();
            assert_eq!(request.address(), 0x76);
            if name == "i2c-register-read-request" {
                assert_eq!(request.write(), [register]);
                assert_eq!(request.read_length(), 1);
            } else {
                assert_eq!(request.write(), [register, 0xa5]);
                assert_eq!(request.read_length(), 0);
            }
        }
    }
}
