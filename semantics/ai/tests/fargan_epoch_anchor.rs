#![cfg(feature = "kernel-step")]
use conduit_core::*;
use conduit_plot::*;
fn fixture(artifact: u8, precision: &str) -> CheckedSyntaxDocument {
    fixture_values([[artifact; 32], [2; 32], [3; 32]], precision)
}
fn fixture_values(values: [[u8; 32]; 3], precision: &str) -> CheckedSyntaxDocument {
    let bytes = |values: &[u8; 32]| {
        format!(
            "[{}]",
            values
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let anchor=format!("{{artifact_identity: {}, model_descriptor_identity: {}, session_basis_identity: {}, precision: {precision}(\"\")}}",bytes(&values[0]),bytes(&values[1]),bytes(&values[2]));
    let source = [
        include_str!("../../speech/fargan_model_identity.conduit").to_string(),
        include_str!("../../speech/fargan_epoch_anchor.conduit").replace(
            "selected: FarganModelFrameAnchor",
            &format!("selected: FarganModelFrameAnchor = {anchor}"),
        ),
    ]
    .join("\n");
    check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap()
}
fn selected(checked: &CheckedSyntaxDocument) -> Vec<u8> {
    let CanonicalStartupValue::Structured(value) = checked.plots[0].startup_parameters[0]
        .default
        .as_ref()
        .unwrap()
    else {
        panic!("startup")
    };
    value.try_concrete().unwrap().canonical_bytes().unwrap()
}
#[test]
fn source_exact_selected_anchor_checks_every_identity_and_precision_with_prepared_parity() {
    let checked = fixture(1, "reference_float32");
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-model-anchor-matches",
        &ProfileCatalog::new(),
    )
    .unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for (document, expected) in [
        (&checked, true),
        (&fixture(4, "reference_float32"), false),
        (&fixture(1, "compact_signed_q7"), false),
    ] {
        let input = selected(document);
        assert_eq!(
            program.evaluate(&input).unwrap(),
            InfoBool::new(expected).encode()
        );
        assert_eq!(
            prepared.evaluate(&input).unwrap(),
            InfoBool::new(expected).encode()
        );
    }
    for field in 0..3 {
        for index in 0..32 {
            let mut values = [[1; 32], [2; 32], [3; 32]];
            values[field][index] = 9;
            let input = selected(&fixture_values(values, "reference_float32"));
            assert_eq!(
                program.evaluate(&input).unwrap(),
                InfoBool::FALSE.encode(),
                "field{field} byte{index}"
            );
            assert_eq!(
                prepared.evaluate(&input).unwrap(),
                InfoBool::FALSE.encode(),
                "field{field} byte{index}"
            );
        }
    }
    assert_eq!(
        expanded.expanded.gears[0].inputs[0].temporal,
        PortTemporal::Flow { closes: true }
    );
}
