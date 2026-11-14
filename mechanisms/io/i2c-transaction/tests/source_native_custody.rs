//! The installed I2C authoring catalogue must carry inline result owners.
#![cfg(feature = "source-catalog")]
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
    let result = completed_result(&contract);
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

fn completed_result(contract: &I2cContract) -> StructuredInfoValue {
    let result_type = contract.decode_result_type().unwrap();
    let StructuredInfoTypeShape::Variant { cases, .. } = result_type.shape() else {
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
    StructuredInfoValue::variant(
        result_type.clone(),
        "completed",
        StructuredInfoValue::record(
            payload.clone(),
            vec![StructuredFieldValue::new("input", octets).unwrap()],
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn real_ht16k33_state_reaches_imported_inline_result_owners() {
    let contract = I2cContract::prepare().unwrap();
    let (catalogue, _) = contract.catalogs();
    let document = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../../plots/device-protocols/ht16k33.conduit"
        )),
        &catalogue,
    )
    .unwrap();
    let bounds = PreparedNativeBounds {
        canonical_bytes: 4096,
        scopes: 64,
        invariants: 64,
        contracts: 64,
    };
    // Exercise every actual protocol root, including dependencies of inactive
    // variant branches; checking only the current branch hid the custody loss.
    for native in &document.native_types {
        PreparedNativeAdmission::new(&document, &native.identity, bounds).unwrap();
    }
    let native = document
        .native_types
        .iter()
        .find(|n| n.name == "Ht16k33State")
        .unwrap();
    let member = |ty: &StructuredInfoType, name: &str| {
        let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
            panic!("record")
        };
        fields
            .iter()
            .find(|f| f.name() == name)
            .unwrap()
            .value_type()
            .clone()
    };
    let configuration_type = member(&native.value_type, "configuration");
    let configuration = StructuredInfoValue::record(
        configuration_type.clone(),
        vec![
            StructuredFieldValue::new(
                "address",
                StructuredInfoValue::leaf(member(&configuration_type, "address"), vec![112])
                    .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "brightness",
                StructuredInfoValue::leaf(member(&configuration_type, "brightness"), vec![7])
                    .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "blink",
                StructuredInfoValue::leaf(member(&configuration_type, "blink"), vec![0]).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "enabled",
                StructuredInfoValue::leaf(member(&configuration_type, "enabled"), vec![1]).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let failure = StructuredInfoValue::variant(
        member(&native.value_type, "failure"),
        "bus",
        completed_result(&contract),
    )
    .unwrap();
    let value = StructuredInfoValue::record(
        native.value_type.clone(),
        vec![
            StructuredFieldValue::new("configuration", configuration).unwrap(),
            StructuredFieldValue::new(
                "phase",
                StructuredInfoValue::leaf(member(&native.value_type, "phase"), vec![6]).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new("failure", failure).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut admission = PreparedNativeAdmission::new(&document, &native.identity, bounds).unwrap();
    admission.begin(&value).unwrap();
    while admission.step().unwrap() != PreparedNativeProgress::Admitted {}
    assert_eq!(admission.admitted().unwrap().canonical_bytes(), value);
}
