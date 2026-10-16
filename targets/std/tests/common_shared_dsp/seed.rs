//! Initial state and tick come from checked Source constructors before Play.
use conduit_core::*;
use conduit_plot::*;
fn program(source: &str, entry: &str) -> PortableExpressionProgram {
    let source = source
        .split("plot shared-excitation-unpack (")
        .next()
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("Source program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
pub fn seed(source: &str, word: i32) -> Vec<u8> {
    let program = program(source, "shared-excitation-start");
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("seed")
    };
    let input = StructuredInfoValue::record(
        program.input_type.clone(),
        vec![StructuredFieldValue::new(
            "word",
            StructuredInfoValue::leaf(fields[0].value_type().clone(), word.to_le_bytes().to_vec())
                .unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    program.evaluate(&input.canonical_bytes().unwrap()).unwrap()
}
pub fn tick(source: &str) -> Vec<u8> {
    program(source, "shared-excitation-tick")
        .evaluate(&[0])
        .unwrap()
}
