use super::*;

#[test]
fn emitted_image_recovers_every_original_offer_field_without_a_raw_catalog_copy() {
    let (document, program) = crate::resident_expression_tests::context();
    let image = PreparedResidentExpression::prepare(
        &document,
        &program,
        c::PortTemporal::Value,
        ResidentExpressionBounds {
            native: conduit_plot::rust_binding::PreparedNativeBounds {
                canonical_bytes: 4096,
                scopes: 64,
                invariants: 64,
                contracts: 64,
            },
            maximum_occurrences: 4096,
            maximum_program_bytes: 512 * 1024,
            maximum_generated_bytes: 1024 * 1024,
            private_work_units: 1000,
        },
    )
    .unwrap();
    assert!(!image.code.contains("OFFER_JSON"));
    let literal = image
        .code
        .split("static OFFER_IMAGE:&[u8]=&[")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap();
    let bytes: Vec<u8> = literal
        .split(',')
        .map(|n| n.trim().parse().unwrap())
        .collect();
    let view = conduit_wire::DeflatedDocument::parse_image(&bytes).unwrap();
    let expected = serde_json::to_vec(&image.offer).unwrap();
    let mut output = vec![0; expected.len()];
    let raw = view
        .decode_into(
            &mut output,
            conduit_wire::DeflatedDocumentLimits {
                maximum_expanded_bytes: 256 * 1024,
                maximum_compressed_bytes: 32 * 1024,
            },
        )
        .unwrap();
    assert_eq!(raw, expected);
    assert!(bytes.len() < expected.len());
    println!(
        "immutable offer image: {} -> {} bytes",
        expected.len(),
        bytes.len()
    );
}
