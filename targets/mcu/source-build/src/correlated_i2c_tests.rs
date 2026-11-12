//! Checked Source correlation transcript; no hardware execution claim.
use super::*;
use sha2::{Digest, Sha256};
#[test]
fn correlated_bus_source_selects_one_native_owner_for_all_requests() {
    let source = include_bytes!("../../../../plots/device-protocols/i2c-correlated.conduit");
    let requests = [Specialization::Zip {
        left: ValueReference {
            type_name: "I2cCorrelation",
            maximum_bytes: 512,
        },
        right: ValueReference {
            type_name: "I2cResult",
            maximum_bytes: conduit_i2c_transaction::contract::I2C_RESULT_VALUE_BYTES as u32,
        },
        feedback: false,
    }];
    let workset = crate::workset::compile_specialized(
        source,
        &format!("{:x}", Sha256::digest(source)),
        &["i2c-correlated-exchange".into()],
        None,
        &requests,
    )
    .unwrap();
    let entry = &workset.entries[0];
    assert_eq!(
        entry
            .plot
            .expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == conduit_i2c_transaction::contract::I2C_KIND)
            .count(),
        1
    );
    let material = crate::software::compile_with_types(
        &entry.plot.expanded,
        Some(&workset.checked_source),
        24 * 1024,
    )
    .unwrap();
    assert_eq!(material.nodes.len(), 1);
    assert!(matches!(
        material.nodes[0].role,
        crate::software::SoftwareRole::Composite(Role::Zip {
            feedback: false,
            ..
        })
    ));
    // Execute the actual checked Source expressions around the same typed
    // pair codec used by the zip Back. This is an authoring transcript,
    // not physical bus or whole-workset execution evidence.
    fn evaluator(
        doc: &p::CheckedSyntaxDocument,
        name: &str,
    ) -> p::PreparedPortableExpressionEvaluator {
        let entry =
            p::expand_canonical_plot_for_authoring(doc, name, &p::ProfileCatalog::new()).unwrap();
        let c::ConfigurationValue::Text(encoded) = &entry.expanded.gears[0].configuration[0].value
        else {
            panic!("program")
        };
        p::PreparedPortableExpressionEvaluator::new(
            &p::PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
        )
        .unwrap()
    }
    fn ty(doc: &p::CheckedSyntaxDocument, name: &str) -> c::StructuredInfoType {
        doc.native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type
            .clone()
    }
    fn field(ty: &c::StructuredInfoType, name: &str) -> c::StructuredInfoType {
        let c::StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
            panic!("record")
        };
        fields
            .iter()
            .find(|f| f.name() == name)
            .unwrap()
            .value_type()
            .clone()
    }
    fn record(
        ty: &c::StructuredInfoType,
        fields: Vec<(&str, c::StructuredInfoValue)>,
    ) -> c::StructuredInfoValue {
        c::StructuredInfoValue::record(
            ty.clone(),
            fields
                .into_iter()
                .map(|(name, value)| c::StructuredFieldValue::new(name, value).unwrap())
                .collect(),
        )
        .unwrap()
    }
    let doc = &workset.checked_source;
    let context = ty(doc, "I2cCorrelation");
    let request_type = ty(doc, "I2cCorrelatedRequest");
    let contract = conduit_i2c_transaction::contract::I2cContract::prepare().unwrap();
    let transaction_type = contract.decode_request_type().unwrap();
    let mut context_program = evaluator(doc, "i2c-correlated-context");
    let mut transaction_program = evaluator(doc, "i2c-correlated-transaction");
    let mut result_program = evaluator(doc, "i2c-correlated-result");
    let mut pair = c::PreparedTypedTuplePairEncoder::new(
        context.clone(),
        512,
        contract.decode_result_type().unwrap(),
        conduit_i2c_transaction::contract::I2C_RESULT_VALUE_BYTES as u32,
    )
    .unwrap();
    let mut results =
        conduit_i2c_transaction::result::PreparedI2cResultEncoder::new(&contract).unwrap();
    for (index, (client, sequence, address, command)) in [
        (11u64, 47u64, 0x76u8, 0xd0u8),
        (22, 47, 0x70, 0x21),
        (11, 48, 0x76, 0xf7),
    ]
    .into_iter()
    .enumerate()
    {
        let correlation = record(
            &context,
            vec![
                (
                    "client",
                    c::StructuredInfoValue::leaf(
                        field(&context, "client"),
                        client.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                ),
                (
                    "sequence",
                    c::StructuredInfoValue::leaf(
                        field(&context, "sequence"),
                        sequence.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                ),
            ],
        );
        let expected_context = correlation.canonical_bytes().unwrap();
        let transaction = record(
            &transaction_type,
            vec![
                (
                    "address",
                    c::StructuredInfoValue::leaf(
                        field(&transaction_type, "address"),
                        vec![address],
                    )
                    .unwrap(),
                ),
                (
                    "read",
                    c::StructuredInfoValue::leaf(field(&transaction_type, "read"), vec![0])
                        .unwrap(),
                ),
                (
                    "write",
                    c::StructuredInfoValue::sequence(
                        field(&transaction_type, "write"),
                        vec![c::StructuredInfoValue::leaf(
                            c::StructuredInfoType::leaf(c::kind_id("value/u8")).unwrap(),
                            vec![command],
                        )
                        .unwrap()],
                    )
                    .unwrap(),
                ),
            ],
        );
        let expected_transaction = transaction.canonical_bytes().unwrap();
        let input = record(
            &request_type,
            vec![("correlation", correlation), ("transaction", transaction)],
        )
        .canonical_bytes()
        .unwrap();
        let context_bytes = context_program.evaluate(&input).unwrap().to_vec();
        assert_eq!(context_bytes, expected_context);
        assert_eq!(
            transaction_program.evaluate(&input).unwrap(),
            expected_transaction
        );
        let result = if index == 1 {
            results
                .disposition(conduit_i2c_transaction::I2cDisposition::NotAcknowledged)
                .unwrap()
                .to_vec()
        } else {
            results.completed(0, &[]).unwrap().to_vec()
        };
        let encoded_pair = pair.encode(&context_bytes, &result).unwrap();
        let output = c::StructuredInfoValue::from_canonical_bytes(
            result_program.evaluate(encoded_pair).unwrap(),
        )
        .unwrap();
        let c::StructuredInfoValueShape::Record(fields) = output.shape() else {
            panic!("result record")
        };
        assert_eq!(
            fields
                .iter()
                .find(|f| f.name() == "correlation")
                .unwrap()
                .value()
                .canonical_bytes()
                .unwrap(),
            expected_context
        );
        assert_eq!(
            fields
                .iter()
                .find(|f| f.name() == "result")
                .unwrap()
                .value()
                .canonical_bytes()
                .unwrap(),
            result
        );
    }
}
