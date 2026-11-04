//! The installed I2C authoring catalogue must carry inline result owners.
use conduit_core::*;
use conduit_i2c_transaction::contract::I2cContract;
use conduit_plot::{rust_binding::*, *};
#[test]
fn ordinary_i2c_catalogue_supplies_complete_checked_native_result_closure() {
    let contract = I2cContract::prepare().unwrap();
    let (catalogue, _) = contract.catalogs();
    let document=check_syntax_document(&parse_syntax_document("with machine/i2c/transact/result as I2cResult\ntype ResultEnvelope = {\n result: I2cResult\n}\n"),&catalogue).unwrap();
    let native = document
        .native_types
        .iter()
        .find(|n| n.name == "ResultEnvelope")
        .unwrap();
    let StructuredInfoTypeShape::Variant { cases, .. } = contract.result_type().shape() else {
        panic!("variant")
    };
    let payload = cases
        .iter()
        .find(|c| c.tag() == "completed")
        .unwrap()
        .payload_type();
    let StructuredInfoTypeShape::Record { fields, .. } = payload.shape() else {
        panic!("record")
    };
    let input = fields
        .iter()
        .find(|f| f.name() == "input")
        .unwrap()
        .value_type();
    let StructuredInfoTypeShape::Sequence { element, .. } = input.shape() else {
        panic!("sequence")
    };
    let octets = StructuredInfoValue::sequence(
        input.clone(),
        vec![StructuredInfoValue::leaf(element.clone(), vec![96]).unwrap()],
    )
    .unwrap();
    let result = StructuredInfoValue::variant(
        contract.result_type().clone(),
        "completed",
        StructuredInfoValue::record(
            payload.clone(),
            vec![StructuredFieldValue::new("input", octets).unwrap()],
        )
        .unwrap(),
    )
    .unwrap();
    let bytes = StructuredInfoValue::record(
        native.value_type.clone(),
        vec![StructuredFieldValue::new("result", result).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut admission = PreparedNativeAdmission::new(
        &document,
        &native.identity,
        PreparedNativeBounds {
            canonical_bytes: 4096,
            scopes: 32,
            invariants: 32,
            contracts: 32,
        },
    )
    .unwrap();
    admission.begin(&bytes).unwrap();
    while admission.step().unwrap() != PreparedNativeProgress::Admitted {}
    assert_eq!(admission.admitted().unwrap().canonical_bytes(), bytes);
}
