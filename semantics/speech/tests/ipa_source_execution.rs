#![cfg(feature = "semantic-bindings")]
use conduit_plot::{rust_binding::NativeRustBinding, PortableExpressionProgram};
use conduit_speech::semantic::*;
fn definition(spelling: &str, kind: SpeechIpaUnitKind) -> SpeechIpaUnitDefinition {
    SpeechIpaUnitDefinition::new(
        SpeechIpaUnitId::new("notation/unit".into()).unwrap(),
        kind,
        SpeechEvidenceProvenance::new(
            "actual Source conformance".into(),
            SpeechEvidenceSource::Manual,
            None,
        )
        .unwrap(),
        SpeechIpaSpelling::new(spelling.into()).unwrap(),
    )
    .unwrap()
}
#[test]
fn actual_source_unit_laws_and_pure_projection_preserve_complete_definition() {
    let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
        env!("OUT_DIR"),
        "/ipa_supported_unit_program.hex"
    )))
    .unwrap();
    let mut cases = vec![];
    for spelling in [
        "ɪ", "ə", "θ", "ð", "ʃ", "ʒ", "ŋ", "ɹ", "j", "ɡ", "t͡ʃ", "d͡ʒ", "pʰ", "tʰ", "kʰ", "ɾ", "n̩",
        "ã", "ã",
    ] {
        cases.push((spelling, SpeechIpaUnitKind::Segment));
    }
    cases.extend([
        ("ˈ", SpeechIpaUnitKind::PrimaryStress),
        ("ˌ", SpeechIpaUnitKind::SecondaryStress),
        ("ː", SpeechIpaUnitKind::Length),
        (".", SpeechIpaUnitKind::SyllableBoundary),
    ]);
    for (spelling, kind) in cases {
        let value = definition(spelling, kind);
        let admitted = SpeechIpaUnitSyntaxAdmission::new(value.clone()).unwrap();
        let output = program.evaluate(&admitted.encode().unwrap()).unwrap();
        assert_eq!(SpeechIpaUnitDefinition::decode(&output).unwrap(), value);
    }
}
#[test]
fn actual_source_laws_refuse_unsupported_units_and_wrong_kinds() {
    for spelling in ["ih", "ax", "ch", "p_aspirated", "tʃ", "̃", "p̃", "n̩ʰ"] {
        assert!(
            SpeechIpaUnitSyntaxAdmission::new(definition(spelling, SpeechIpaUnitKind::Segment))
                .is_err(),
            "{spelling}"
        );
    }
    assert!(
        SpeechIpaUnitSyntaxAdmission::new(definition("ˈ", SpeechIpaUnitKind::SecondaryStress))
            .is_err()
    );
    assert!(
        SpeechIpaUnitSyntaxAdmission::new(definition("ː", SpeechIpaUnitKind::Segment)).is_err()
    );
}
