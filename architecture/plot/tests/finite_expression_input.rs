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
