//! Exhaustive finite-law comparison, plus cross-entrance admission/refusal.
//! Inspect the parsed law shape, not a sample of its quoted strings: broadening
//! a law beyond a closed equality disjunction must also fail this contract.
use super::*;
use crate::{ipa_phone::phone_from_ipa, semantic::*};
use alloc::{collections::BTreeSet, format, string::String, string::ToString};
use conduit_plot::{parse_syntax_document, BinaryOperator, ExpressionSyntax, QuotedTextSourceMap};

const PHONE_SOURCE: &str = include_str!("../../ipa.conduit");
const UNIT_SOURCE: &str = include_str!("../../ipa_syntax.conduit");
const KINDS: [(UnitKind, &str); 5] = [
    (UnitKind::Segment, "segment"),
    (UnitKind::PrimaryStress, "primary_stress"),
    (UnitKind::SecondaryStress, "secondary_stress"),
    (UnitKind::Length, "length"),
    (UnitKind::SyllableBoundary, "syllable_boundary"),
];

fn compact(source: &str, expression: &ExpressionSyntax) -> String {
    let span = expression.span();
    source[span.start..span.end]
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn closed_spellings(
    source: &str,
    expression: &ExpressionSyntax,
    field: &str,
    values: &mut BTreeSet<String>,
) {
    match expression {
        ExpressionSyntax::Binary {
            operator: BinaryOperator::BooleanOr,
            left,
            right,
            ..
        } => {
            closed_spellings(source, left, field, values);
            closed_spellings(source, right, field, values);
        }
        ExpressionSyntax::Binary {
            operator: BinaryOperator::Equal,
            left,
            right,
            ..
        } => {
            assert_eq!(compact(source, left), field);
            let ExpressionSyntax::Atomic(token) = right.as_ref() else {
                panic!("closed spelling equality requires a quoted constant")
            };
            let spelling = QuotedTextSourceMap::new(source, token, 64).unwrap();
            assert!(
                values.insert(spelling.decoded().into()),
                "duplicate spelling"
            );
        }
        other => panic!("expected closed spelling disjunction, got {other:?}"),
    }
}

#[test]
fn complete_checked_whitelists_equal_the_rust_grammar() {
    let phone = parse_syntax_document(PHONE_SOURCE);
    let unit = parse_syntax_document(UNIT_SOURCE);
    assert!(phone.diagnostics.is_empty());
    assert!(unit.diagnostics.is_empty());
    let phone = phone
        .types
        .iter()
        .find(|ty| ty.name.text == "SpeechPhoneNotation")
        .unwrap();
    assert_eq!(phone.invariants.len(), 1);
    let mut phone_spellings = BTreeSet::new();
    closed_spellings(
        PHONE_SOURCE,
        &phone.invariants[0].syntax,
        ".spelling",
        &mut phone_spellings,
    );
    let rust: BTreeSet<String> = SUPPORTED_SEGMENTS.iter().map(|s| (*s).into()).collect();
    assert_eq!(
        rust.len(),
        SUPPORTED_SEGMENTS.len(),
        "duplicate Rust segment"
    );
    assert_eq!(phone_spellings, rust);

    let unit = unit
        .types
        .iter()
        .find(|ty| ty.name.text == "SpeechIpaUnitSyntaxAdmission")
        .unwrap();
    assert_eq!(unit.invariants.len(), KINDS.len());
    for (law, (kind, tag)) in unit.invariants.iter().zip(KINDS) {
        let ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } = &law.syntax
        else {
            panic!("one explicit syntax law per unit kind")
        };
        assert_eq!(
            compact(UNIT_SOURCE, condition),
            format!(".definition.kindis{tag}")
        );
        assert_eq!(compact(UNIT_SOURCE, when_false), "true");
        let mut spellings = BTreeSet::new();
        closed_spellings(
            UNIT_SOURCE,
            when_true,
            ".definition.spelling",
            &mut spellings,
        );
        let expected = match kind {
            UnitKind::Segment => rust.clone(),
            UnitKind::PrimaryStress => BTreeSet::from(["ˈ".into()]),
            UnitKind::SecondaryStress => BTreeSet::from(["ˌ".into()]),
            UnitKind::Length => BTreeSet::from(["ː".into()]),
            UnitKind::SyllableBoundary => BTreeSet::from([".".into()]),
        };
        assert_eq!(spellings, expected, "{tag}");
    }
}

fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "finite IPA grammar parity".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap()
}

#[test]
fn rust_parser_and_both_native_laws_agree_on_admission_and_refusal() {
    let mut corpus: BTreeSet<String> = SUPPORTED_SEGMENTS.iter().map(|s| (*s).into()).collect();
    for spelling in SUPPORTED_SEGMENTS {
        // Perturb every supported segment; some combinations are themselves
        // supported, so the exact Rust whitelist determines the expected result.
        for suffix in ["ʰ", "̃", "̩", "͡", " "] {
            corpus.insert(format!("{spelling}{suffix}"));
        }
    }
    for spelling in [
        "",
        "ˈ",
        "ˌ",
        "ː",
        ".",
        "ih",
        "ax",
        "ch",
        "g",
        "tʃ",
        "dʒ",
        "p_aspirated",
        "͡",
        "̃",
        "[t]",
        "/t/",
        "\"",
        "\\",
        "雪",
        "\u{200b}",
        "\u{202e}",
    ] {
        corpus.insert(spelling.to_string());
    }
    for spelling in corpus {
        let expected = supported_unit(&spelling, UnitKind::Segment);
        let phone = SpeechIpaSpelling::new(spelling.clone())
            .ok()
            .and_then(|s| SpeechPhoneNotation::new(provenance(), s).ok());
        assert_eq!(phone.is_some(), expected, "phone Native: {spelling:?}");
        assert_eq!(
            phone_from_ipa(spelling.clone(), provenance()).is_ok(),
            expected,
            "phone parser: {spelling:?}"
        );
        for (kind, _) in KINDS {
            let native_kind = match kind {
                UnitKind::Segment => SpeechIpaUnitKind::Segment,
                UnitKind::PrimaryStress => SpeechIpaUnitKind::PrimaryStress,
                UnitKind::SecondaryStress => SpeechIpaUnitKind::SecondaryStress,
                UnitKind::Length => SpeechIpaUnitKind::Length,
                UnitKind::SyllableBoundary => SpeechIpaUnitKind::SyllableBoundary,
            };
            let unit = SpeechIpaSpelling::new(spelling.clone())
                .ok()
                .and_then(|s| {
                    SpeechIpaUnitDefinition::new(
                        SpeechIpaUnitId::new("parity/unit".into()).unwrap(),
                        native_kind,
                        provenance(),
                        s,
                    )
                    .ok()
                })
                .and_then(|unit| SpeechIpaUnitSyntaxAdmission::new(unit).ok());
            assert_eq!(
                unit.is_some(),
                supported_unit(&spelling, kind),
                "unit Native {kind:?}: {spelling:?}"
            );
        }
    }
}
