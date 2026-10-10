use super::super::tests::fixture;
use super::*;

fn family(policy: TypedLiteralLexicalPolicy) -> TypedLiteralFamily {
    let (_, _, mut family) = fixture();
    family.branches[0].lexical_policy = policy;
    family.branches[0].maximum_payload_bytes = 64;
    let mut alias = family.branches[0].clone();
    alias.delimiter = TypedLiteralDelimiter::DoubleSquare;
    family.branches.push(alias);
    family
}

#[test]
fn resolved_family_preserves_unicode_bytes_and_selects_only_declared_pair() {
    let family = family(TypedLiteralLexicalPolicy::RawUnicode);
    let source = "ph⟦t͡ʃ.ə⟧ + remaining";
    let value = family.scan_literal("ph", source).unwrap();
    assert_eq!(value.raw_payload, "t͡ʃ.ə");
    assert_eq!(&source[value.payload_bytes.clone()], value.raw_payload);
    assert_eq!(value.authored, "ph⟦t͡ʃ.ə⟧");
    assert_eq!(&source[value.consumed_bytes..], " + remaining");
    assert_eq!(value.branch.delimiter, TypedLiteralDelimiter::DoubleSquare);
    assert_eq!(value.payload, value.raw_payload);
    let ascii = family.scan_literal("ph", "ph/t͡ʃ.ə/").unwrap();
    assert_eq!(ascii.payload, value.payload);
    assert_eq!(ascii.branch.constructor_kind, value.branch.constructor_kind);
    assert_eq!(ascii.branch.result_type, value.branch.result_type);
    assert_ne!(ascii.authored, value.authored);
}

#[test]
fn pattern_aliases_share_existing_class_escape_anchor_and_flag_laws() {
    let family = family(TypedLiteralLexicalPolicy::PortablePattern);
    let ascii = family.scan_literal("r", "/ignored");
    assert_eq!(ascii.unwrap_err(), TypedLiteralScanRefusal::UnboundPrefix);
    for source in ["r/^t͡ʃ[ə/]+$/i rest", "r⟦^t͡ʃ[ə/]+$⟧i rest"] {
        let value = family.scan_literal("r", source).unwrap();
        assert_eq!(value.raw_payload, "^t͡ʃ[ə/]+$");
        assert_eq!(value.payload, "t͡ʃ[ə/]+");
        assert!(value.case_insensitive && value.anchored_start && value.anchored_end);
        assert_eq!(&source[value.consumed_bytes..], " rest");
        assert!(crate::parse_text_pattern(value.payload).is_ok());
    }
    let quoted = family.scan_literal("r", r"r/a\/b/").unwrap();
    assert_eq!(quoted.payload, r"a\/b");
    assert!(crate::parse_text_pattern(quoted.payload).is_err());
    for source in ["r/foo/m", "r⟦foo⟧ii", "r//", "r⟦^$⟧", "r/[foo/"] {
        assert!(family.scan_literal("r", source).is_err(), "{source}");
    }
}

#[test]
fn raw_closing_parity_never_decodes_or_normalizes_payload() {
    let family = family(TypedLiteralLexicalPolicy::RawUnicode);
    for (source, payload) in [
        (r"ph/a\/b/", r"a\/b"),
        (r"ph/path\\/", r"path\\"),
        (r"ph⟦a\⟧b⟧", r"a\⟧b"),
        ("ph/e\u{301}/", "e\u{301}"),
        ("ph/é/", "é"),
    ] {
        assert_eq!(family.scan_literal("ph", source).unwrap().payload, payload);
    }
}

#[test]
fn foreign_spaced_confusable_and_unreviewed_entrances_refuse() {
    let family = family(TypedLiteralLexicalPolicy::RawUnicode);
    for source in [
        "other/x/",
        "ph / x / y",
        "ph[x]",
        "ph〈x〉",
        "ph⟨x⟩",
        "ph\u{202e}/x/",
        "ph\u{200b}/x/",
    ] {
        assert!(family.scan_literal("ph", source).is_err(), "{source:?}");
    }
    assert_eq!(
        family.scan_literal("ph", "ph/x").unwrap_err(),
        TypedLiteralScanRefusal::Unterminated
    );
    assert_eq!(
        family.scan_literal("ph", "ph/x/i").unwrap_err(),
        TypedLiteralScanRefusal::InvalidPatternOrFlags
    );
}

#[test]
fn payload_byte_limits_apply_before_domain_work_including_unicode() {
    for policy in [
        TypedLiteralLexicalPolicy::RawUnicode,
        TypedLiteralLexicalPolicy::PortablePattern,
    ] {
        let family = family(policy);
        for body in ["a".repeat(64), "ə".repeat(32)] {
            assert_eq!(
                family
                    .scan_literal("r", &format!("r/{body}/"))
                    .unwrap()
                    .raw_payload,
                body
            );
            assert_eq!(
                family
                    .scan_literal("r", &format!("r/{body}a/"))
                    .unwrap_err(),
                TypedLiteralScanRefusal::PayloadLimit
            );
        }
        assert_eq!(
            family
                .scan_literal("r", &format!("r/{}", "ə".repeat(1_000_000)))
                .unwrap_err(),
            TypedLiteralScanRefusal::PayloadLimit
        );
    }
}
