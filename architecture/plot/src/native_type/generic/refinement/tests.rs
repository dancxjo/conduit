use crate::{check_syntax_document, parse_syntax_document, StartupCatalog};

#[test]
fn dimensions_keep_existing_exact_range_contracts() {
    let source = "type Matrix<C: U16, R: U16> = {\n columns: U16 in C..=C\n rows: U16 in R..=R\n}\ntype Value = Matrix<192, 128>\n";
    let output =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let value = &output.native_types[0];
    assert_eq!(value.value_contracts.len(), 2);
    for contract in &value.value_contracts {
        let (correct, wrong) = if contract.representation_path == ".columns" {
            (192_u16, 193_u16)
        } else {
            (128_u16, 127_u16)
        };
        contract.contract.validate(&correct.to_le_bytes()).unwrap();
        assert!(contract.contract.validate(&wrong.to_le_bytes()).is_err());
    }
    let literal = source
        .replace("C..=C", "192..=192")
        .replace("R..=R", "128..=128");
    // The family contract version differs, but its exact scalar promises agree.
    let literal = literal
        .replace("type Matrix<C: U16, R: U16> =", "type Value =")
        .replace("type Value = Matrix<192, 128>\n", "");
    let old =
        check_syntax_document(&parse_syntax_document(&literal), &StartupCatalog::new()).unwrap();
    assert_eq!(value.value_contracts, old.native_types[0].value_contracts);
    assert_eq!(parse_syntax_document(source).round_trip(), source);
}

#[test]
fn refinement_arithmetic_and_membership_reuse_checked_integer_validation() {
    let source = "type Limits<N: U16> = {\n range: U32 in 0..=(N + 1) * 2\n member: U16 in [N, N + 1]\n}\ntype Value = Limits<64>\n";
    let output =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let contracts = &output.native_types[0].value_contracts;
    let range = &contracts
        .iter()
        .find(|value| value.representation_path == ".range")
        .unwrap()
        .contract;
    range.validate(&130_u32.to_le_bytes()).unwrap();
    assert!(range.validate(&131_u32.to_le_bytes()).is_err());
    let members = &contracts
        .iter()
        .find(|value| value.representation_path == ".member")
        .unwrap()
        .contract;
    members.validate(&64_u16.to_le_bytes()).unwrap();
    members.validate(&65_u16.to_le_bytes()).unwrap();
    assert!(members.validate(&66_u16.to_le_bytes()).is_err());
}

#[test]
fn refinement_overflow_keeps_original_unicode_operator_and_columns() {
    let source = "type Limité<N: U16> = U16 in 0..=N + 1\ntype Value = Limité<65535>\n";
    let error =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap_err();
    assert!(error.message.contains("overflows U16"), "{}", error.message);
    assert_eq!(&source[error.span.start..error.span.end], "+");
    assert_eq!(error.span.line, 1);
    assert_eq!(
        error.span.column,
        source[..error.span.start].chars().count() + 1
    );
    for operand in ["N / 2", "N - 1", "Unknown + N"] {
        let source =
            format!("type Limited<N: U16> = U16 in 0..={operand}\ntype Value = Limited<64>\n");
        assert!(
            check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).is_err()
        );
    }
}

#[test]
fn unrelated_domains_and_large_integer_literals_keep_their_meaning() {
    let source = "type Envelope<N: U16> = {\n capacity: collection U8 = N\n wide: U32 in 0..=100000\n text: Text <= 8B in [\"N + 1\", \"other\"]\n}\ntype Value = Envelope<2>\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let contracts = &checked.native_types[0].value_contracts;
    let wide = &contracts
        .iter()
        .find(|value| value.representation_path == ".wide")
        .unwrap()
        .contract;
    wide.validate(&100000_u32.to_le_bytes()).unwrap();
    let text = &contracts
        .iter()
        .find(|value| value.representation_path == ".text")
        .unwrap()
        .contract;
    text.validate(b"N + 1").unwrap();
    assert!(text.validate(b"3").is_err());
}
