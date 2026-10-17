//! Ordinary authored Fore connections preserve closed specialization identities.
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};

const TYPES: &str = "type Vector<T, N: U16> = collection T = N\n\
type Narrow = Vector<U8, 64>\n\
type Wide = Vector<U8, 128>\n\
type Equivalent = Vector<U8, 32 + 32>\n\
type SameBytes = Vector<U16, 32>\n\
type OtherVector<T, N: U16> = collection T = N\n\
type Foreign = OtherVector<U8, 64>\n\
type Nested<N: U16> = {\n values: Vector<U8, N>\n}\n\
type Nested64 = Nested<64>\n\
type Nested128 = Nested<128>\n";

fn source(input: &str, output: &str) -> String {
    format!("{TYPES}\nplot relay (\n >> value: {input}\n result: {output} >>\n) {{\n value >> result\n}}\n")
}

#[test]
fn matching_and_equivalent_dimensions_connect_in_an_ordinary_fore() {
    for (input, output) in [("Narrow", "Narrow"), ("Nested64", "Nested64")] {
        let source = source(input, output);
        let syntax = parse_syntax_document(&source);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        assert_eq!(checked.plots.len(), 1);
    }
    let literal = source("Narrow", "Narrow");
    let arithmetic = literal.replace(
        "type Narrow = Vector<U8, 64>",
        "type Narrow = Vector<U8, 32 + 32>",
    );
    let check = |source: &str| {
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap()
    };
    assert_eq!(
        check(&literal).native_types,
        check(&arithmetic).native_types
    );
}

#[test]
fn different_dimensions_nested_dimensions_and_nominal_owners_refuse() {
    for (input, output) in [
        ("Narrow", "Wide"),
        ("Nested64", "Nested128"),
        ("Narrow", "SameBytes"),
        ("Narrow", "Foreign"),
        ("Narrow", "Equivalent"),
    ] {
        let source = source(input, output);
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty());
        let error = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap_err();
        assert_eq!(error.code, "CND-FRM-058");
        assert!(
            error.message.contains("semantic Type mismatch"),
            "{error:?}"
        );
        assert!(error.span.start >= TYPES.len(), "{error:?}");
    }
}
