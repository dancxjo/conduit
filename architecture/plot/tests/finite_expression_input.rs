//! Authored byte ceilings survive canonical program encoding and evaluation.
use conduit_core::ConfigurationValue;
use conduit_plot::*;

fn programs(source: &str, entry: &str) -> Vec<PortableExpressionProgram> {
    let parsed = parse_syntax_document(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    expanded
        .gears
        .iter()
        .filter_map(|gear| {
            gear.configuration.iter().find_map(|entry| {
                if entry.key != "program" {
                    return None;
                }
                let ConfigurationValue::Text(encoded) = &entry.value else {
                    panic!("program")
                };
                Some(PortableExpressionProgram::from_canonical_hex(encoded).unwrap())
            })
        })
        .collect()
}

const SOURCE: &str = "
plot small (
 wire: Bytes <= 64B >> size: U64
) = (bytes/length(.))
plot large (
 wire: Bytes <= 2048B >> size: U64
) = (bytes/length(.))
plot legacy (
 wire: Bytes <= 65536B >> size: U64
) = (bytes/length(.))
plot sizes (
 >> first: Bytes <= 64B
 >> second: Bytes <= 2048B
 small_size: U64 >>
 large_size: U64 >>
) {
 first >> small() >> small_size
 second >> large() >> large_size
}
";

#[test]
fn distinct_input_ceilings_remain_exact_in_one_graph() {
    let mut bounds = programs(SOURCE, "sizes")
        .into_iter()
        .map(|program| {
            assert_eq!(
                program.input_maximum_bytes,
                Some(program.maximum_prepared_input_bytes().unwrap())
            );
            let encoded = program.canonical_bytes().unwrap();
            assert!(encoded.starts_with(b"conduit.pure-expression.program.v3"));
            assert_eq!(
                PortableExpressionProgram::from_canonical_bytes(&encoded).unwrap(),
                program
            );
            program.input_maximum_bytes.unwrap()
        })
        .collect::<Vec<_>>();
    bounds.sort();
    assert_eq!(bounds, vec![64, 2048]);
}

#[test]
fn both_evaluators_enforce_the_exact_input_ceiling() {
    let program = programs(SOURCE, "large").remove(0);
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let accepted = vec![7; 2048];
    assert_eq!(
        prepared.evaluate(&accepted).unwrap(),
        program.evaluate(&accepted).unwrap()
    );
    assert_eq!(
        program.evaluate(&vec![7; 2049]),
        Err(PortableExpressionEvaluationRefusal::InvalidInput)
    );
    assert!(prepared.evaluate(&vec![7; 2049]).is_err());
}

#[test]
fn legacy_identity_and_invalid_metadata_are_explicit() {
    let mut legacy = programs(SOURCE, "legacy").remove(0);
    legacy.input_maximum_bytes = None;
    let unbounded = conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32;
    let bytes = legacy.canonical_bytes().unwrap();
    assert!(bytes.starts_with(b"conduit.pure-expression.program.v2"));
    assert_eq!(
        PortableExpressionProgram::from_canonical_bytes(&bytes)
            .unwrap()
            .canonical_bytes()
            .unwrap(),
        bytes
    );
    let bounded = programs(SOURCE, "large").remove(0);
    assert_ne!(
        bounded.canonical_hex().unwrap(),
        legacy.canonical_hex().unwrap()
    );
    let mut forged = bounded.canonical_bytes().unwrap();
    let header = b"conduit.pure-expression.program.v3".len();
    forged[header..header + 4].copy_from_slice(&(unbounded + 1).to_le_bytes());
    assert!(PortableExpressionProgram::from_canonical_bytes(&forged).is_err());
    forged[header..header + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(PortableExpressionProgram::from_canonical_bytes(&forged).is_err());
}

#[test]
fn a_coarse_fore_ceiling_never_broadens_the_structural_type() {
    let mut program = programs(SOURCE, "large").remove(0);
    program.input_maximum_bytes = Some(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32);
    let encoded = program.canonical_bytes().unwrap();
    let decoded = PortableExpressionProgram::from_canonical_bytes(&encoded).unwrap();
    let invalid = vec![7; conduit_core::MAXIMUM_BYTES_INFO_BYTES + 1];
    assert_eq!(
        decoded.evaluate(&invalid),
        Err(PortableExpressionEvaluationRefusal::InvalidInput)
    );
    assert!(PreparedPortableExpressionEvaluator::new(&decoded)
        .unwrap()
        .evaluate(&invalid)
        .is_err());
}

#[test]
fn semantic_call_arguments_keep_the_source_ceiling() {
    use conduit_core::{
        kind_id, port_id, CapabilityLimits, Kind, KindIdentity, PortDescriptor, PortDirection,
        PortTemporal,
    };
    let port = |name, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id("value/u64"),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert_kind(Kind {
            kind_id: kind_id("test/count"),
            kind_contract_revision: KindIdentity::from("test/count@1"),
            startup_parameters: vec![],
            shorthand: Some((port_id("input"), port_id("output"))),
            inputs: vec![port("input", PortDirection::Input)],
            outputs: vec![port("output", PortDirection::Output)],
            configuration: vec![],
            semantic_laws: pure_expression_semantic_laws(),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 8,
            },
        })
        .unwrap();
    for (expression, expected) in [
        ("test/count(bytes/length(.))", vec![2048]),
        ("test/count(test/count(bytes/length(.)) + 1)", vec![8, 2048]),
    ] {
        let syntax = parse_syntax_document(&format!(
            "plot call (\n wire: Bytes <= 2048B >> result: U64\n) = ({expression})\n"
        ));
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        let expanded = expand_canonical_plot_for_authoring(&checked, "call", &catalog)
            .unwrap()
            .expanded;
        let mut bounds = expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_contract_revision.as_str() == PURE_EXPRESSION_REVISION)
            .map(|argument| {
                let ConfigurationValue::Text(encoded) = &argument
                    .configuration
                    .iter()
                    .find(|entry| entry.key == "program")
                    .unwrap()
                    .value
                else {
                    panic!("program")
                };
                let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
                let maximum = program.maximum_prepared_input_bytes().unwrap();
                if let Some(bound) = program.input_maximum_bytes {
                    assert_eq!(bound, maximum);
                    assert_eq!(
                        argument.semantic_contract.value_contracts()[0]
                            .contract
                            .maximum_bytes,
                        maximum
                    );
                } else {
                    assert_eq!(maximum, 8);
                    assert!(argument.semantic_contract.value_contracts().is_empty());
                }
                maximum
            })
            .collect::<Vec<_>>();
        bounds.sort();
        assert_eq!(bounds, expected);
    }
}

#[test]
fn exact_identity_retains_output_contract_without_claiming_transformed_bounds() {
    use conduit_core::{port_id, FrontValueLocation};
    for (body, identity) in [(".", true), (". == 0 ? 1 : .", false)] {
        let source =
            format!("plot relay (\n wire: U64 <= 64B >> result: U64 <= 64B\n) = ({body})\n");
        let parsed = parse_syntax_document(&source);
        assert!(parsed.diagnostics.is_empty());
        let checked = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap();
        let expanded =
            expand_canonical_plot_for_authoring(&checked, "relay", &ProfileCatalog::new()).unwrap();
        let gear = &expanded.expanded.gears[0];
        let program = programs(&source, "relay").pop().unwrap();
        assert_eq!(
            gear.semantic_contract.laws,
            program.semantic_laws().unwrap()
        );
        let front = gear.checked_front();
        let output = front.value_contract(&FrontValueLocation::Output(port_id("output")));
        assert_eq!(output.is_some(), identity);
        if let Some(output) = output {
            assert_eq!(program.maximum_prepared_input_bytes().unwrap(), 8);
            assert_eq!(program.maximum_prepared_output_bytes().unwrap(), 8);
            assert_eq!(output.maximum_bytes, 64);
            assert_eq!(
                Some(output),
                front.value_contract(&FrontValueLocation::Input(port_id("input")))
            );
            let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
            assert_eq!(prepared.output_capacity(), 8);
            assert_eq!(
                prepared.evaluate(&255u64.to_le_bytes()).unwrap(),
                &255u64.to_le_bytes()
            );
            assert!(prepared.evaluate(&[0; 65]).is_err());
        }
    }
}

#[test]
fn bounded_boolean_identity_filter_keeps_flow_and_optional_value_contracts_distinct() {
    use conduit_core::{
        kind_id, port_id, FrontValueLocation, KindSemanticLaw, PortTemporal, StructuredInfoType,
    };
    let boolean = StructuredInfoType::leaf(kind_id("value/bool")).unwrap();
    let program = PortableExpressionProgram {
        input_type: boolean.clone(),
        input_maximum_bytes: Some(1),
        output_type: boolean.clone(),
        root: PortableExpressionNode {
            value_type: boolean,
            operation: PortableExpressionOperation::Input,
        },
    };
    let decoded =
        PortableExpressionProgram::from_canonical_hex(&program.canonical_hex().unwrap()).unwrap();
    for temporal in [PortTemporal::Flow { closes: true }, PortTemporal::Value] {
        let definition = portable_filter_definition(&decoded, temporal).unwrap();
        let contracts: Vec<_> = decoded
            .filter_semantic_laws(temporal)
            .unwrap()
            .into_iter()
            .filter_map(|law| {
                if let KindSemanticLaw::ValueContracts(contracts) = law {
                    Some(contracts)
                } else {
                    None
                }
            })
            .flatten()
            .collect();
        let outputs: Vec<_> = contracts
            .iter()
            .filter(|contract| contract.location == FrontValueLocation::Output(port_id("output")))
            .collect();
        if matches!(temporal, PortTemporal::Flow { .. }) {
            assert_eq!(outputs.len(), 1);
            assert_eq!(
                outputs[0].contract.value_kind,
                definition.outputs[0].value_kind
            );
            assert_eq!(outputs[0].contract.maximum_bytes, 1);
        } else {
            assert!(outputs.is_empty());
            assert_ne!(definition.outputs[0].value_kind.as_str(), "value/bool");
        }
        assert_eq!(
            contracts
                .iter()
                .filter(|contract| contract.location == FrontValueLocation::Input(port_id("input")))
                .count(),
            1
        );
    }
}
