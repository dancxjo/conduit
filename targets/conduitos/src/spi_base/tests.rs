use super::{
    contract::SpiContract, decode::PreparedSpiDecoder, result::PreparedSpiResultEncoder,
    transaction::SpiDisposition,
};
use alloc::{vec, vec::Vec};
use conduit_core::{
    StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue,
    validate_canonical_structured_value,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};

pub(super) fn request(contract: &SpiContract, bytes: &[u8]) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
        panic!("request");
    };
    StructuredInfoValue::record(
        contract.request_type().clone(),
        vec![
            StructuredFieldValue::new(
                "output",
                StructuredInfoValue::leaf(fields[0].value_type().clone(), bytes.to_vec()).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
#[test]
fn checked_spi_topology_is_one_generic_exchange() {
    let contract = SpiContract::prepare().unwrap();
    let (startup, profile) = contract.catalogs();
    let syntax = parse_syntax_document(include_str!(
        "../../../../plots/device-protocols/spi.conduit"
    ));
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "spi-exchange", &profile)
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1);
    assert_eq!(expanded.gears[0].kind_id, contract.kind().kind_id);
}
#[test]
fn packed_requests_borrow_exact_bytes_and_refuse_empty_or_malformed_frames() {
    let contract = SpiContract::prepare().unwrap();
    let decoder = PreparedSpiDecoder::new(&contract).unwrap();
    let bytes = core::array::from_fn::<_, 512, _>(|i| i as u8);
    for length in 1..=512 {
        let wire = request(&contract, &bytes[..length]);
        assert!(wire.len() <= 4096);
        let parsed = decoder.decode(&wire).unwrap();
        assert_eq!(parsed.output(), &bytes[..length]);
        assert!(parsed.output().as_ptr() >= wire.as_ptr());
        assert!(parsed.output().as_ptr() < wire.as_ptr().wrapping_add(wire.len()));
    }
    assert!(decoder.decode(&request(&contract, &[])).is_err());
    let wire = request(&contract, &[1, 2, 3]);
    for length in 0..wire.len() {
        assert!(decoder.decode(&wire[..length]).is_err());
    }
    let mut trailing = wire.clone();
    trailing.push(0);
    assert!(decoder.decode(&trailing).is_err());
}
#[test]
fn prepared_success_and_refusal_encoding_allocate_nothing() {
    let contract = SpiContract::prepare().unwrap();
    let decoder = PreparedSpiDecoder::new(&contract).unwrap();
    let mut encoder = PreparedSpiResultEncoder::new(&contract).unwrap();
    let wire = request(&contract, &[0x55; 512]);
    let allocations = crate::test_allocations::allocations(|| {
        let transaction = decoder.decode(&wire).unwrap();
        let result = encoder
            .completed(transaction.output().len(), &[0xaa; 512])
            .unwrap();
        let value = validate_canonical_structured_value(result).unwrap();
        assert_eq!(value.variant_tag().unwrap(), "completed");
        assert_eq!(
            value
                .variant_payload("completed")
                .unwrap()
                .unwrap()
                .record_field("input")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bytes")
                .unwrap(),
            &[0xaa; 512]
        );
        assert!(encoder.completed(512, &[0; 511]).is_err());
        assert!(encoder.completed(0, &[]).is_err());
        for disposition in [
            SpiDisposition::TimedOut,
            SpiDisposition::ProviderLost,
            SpiDisposition::Unsupported,
            SpiDisposition::StaleAttachment,
            SpiDisposition::Refused,
        ] {
            let result = encoder.disposition(disposition).unwrap();
            assert_ne!(
                validate_canonical_structured_value(result)
                    .unwrap()
                    .variant_tag()
                    .unwrap(),
                "completed"
            );
        }
    });
    assert_eq!(allocations, 0);
}
