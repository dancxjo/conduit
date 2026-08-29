//! Reviewed finite reusable Form Backs for both Morse directions.

use conduit_form::{
    check_syntax_document, parse_syntax_document, CanonicalBackCatalog, ProfileCatalog,
    StartupCatalog,
};

const TEXT_MORSE_BACK: &str = r#"form text/morse (
    unit-ms: Count = 120
    text: value/text@1 > pattern: value/morse-pattern@1
) {
    symbols: text/morse-symbols
    timing: morse/symbols-to-pattern(unit-ms)
    text > symbols > timing > pattern
}
"#;

const TEXT_MORSE_SYMBOLS_BACK: &str = r#"form text/morse-symbols (
    text: value/text@1 > symbols: value/morse-symbols@1
) {
    characters: text/characters
    lookup: morse/lookup
    gaps: morse/intersperse
    flatten: morse/flatten
    text > characters > lookup > gaps > flatten > symbols
}
"#;

const MORSE_TEXT_BACK: &str = r#"form morse/text (
    pattern: value/morse-pattern@1 > text: value/text@1
) {
    symbols: morse/pattern-to-symbols
    decode: morse/symbols-to-text
    pattern > symbols > decode > text
}
"#;

pub fn install_morse_backs(
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
    backs: &mut CanonicalBackCatalog,
) -> Result<(), alloc::string::String> {
    for (kind, form_name, source) in [
        (crate::TEXT_MORSE_KIND, "text/morse", TEXT_MORSE_BACK),
        (
            crate::TEXT_MORSE_SYMBOLS_KIND,
            "text/morse-symbols",
            TEXT_MORSE_SYMBOLS_BACK,
        ),
        (crate::MORSE_TEXT_KIND, "morse/text", MORSE_TEXT_BACK),
    ] {
        let checked = check_syntax_document(&parse_syntax_document(source), startup)
            .map_err(|error| alloc::format!("check {form_name} Back: {error:?}"))?;
        let definition = profile
            .get(&conduit_core::kind_id(kind))
            .ok_or_else(|| alloc::format!("missing {kind} definition"))?;
        let signature = startup
            .signature(kind)
            .ok_or_else(|| alloc::format!("missing {kind} startup Face"))?;
        backs
            .insert_with_startup(
                definition,
                &signature.startup_parameters,
                &checked,
                form_name,
            )
            .map_err(|error| alloc::format!("install {form_name} Back: {error:?}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_form::{CanonicalBackError, StartupParameterSignature};

    #[test]
    fn text_morse_expands_through_a_nested_back_to_five_typed_leaves() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        crate::install_text_catalogs(&mut startup, &mut profile).unwrap();
        crate::install_morse_catalogs(&mut startup, &mut profile).unwrap();
        let checked = check_syntax_document(
            &parse_syntax_document(
                "form main (\n pattern: value/morse-pattern@1 >\n) {\n source: text/literal(\"SOS\")\n morse: text/morse(80)\n source > morse > pattern\n}\n",
            ),
            &startup,
        )
        .unwrap();
        let mut backs = CanonicalBackCatalog::new();
        install_morse_backs(&startup, &profile, &mut backs).unwrap();
        let expanded = conduit_form::expand_canonical_form_for_authoring_with_backs(
            &checked, "main", &profile, &backs,
        )
        .unwrap()
        .expanded;
        assert_eq!(expanded.realization_backs.len(), 2);
        assert!(expanded
            .realization_backs
            .iter()
            .any(|back| back.kind_id.as_str() == crate::TEXT_MORSE_KIND));
        assert!(expanded
            .realization_backs
            .iter()
            .any(|back| back.kind_id.as_str() == crate::TEXT_MORSE_SYMBOLS_KIND));
        let kinds = expanded
            .gears
            .iter()
            .map(|gear| gear.kind_id.as_str())
            .collect::<alloc::vec::Vec<_>>();
        for expected in [
            crate::TEXT_CHARACTERS_KIND,
            crate::MORSE_LOOKUP_KIND,
            crate::MORSE_INTERSPERSE_KIND,
            crate::MORSE_FLATTEN_KIND,
            crate::MORSE_SYMBOLS_TO_PATTERN_KIND,
        ] {
            assert!(kinds.contains(&expected));
        }
    }

    #[test]
    fn startup_face_difference_refuses_back_substitution() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        crate::install_text_catalogs(&mut startup, &mut profile).unwrap();
        crate::install_morse_catalogs(&mut startup, &mut profile).unwrap();
        let checked = check_syntax_document(&parse_syntax_document(TEXT_MORSE_BACK), &startup)
            .expect("reviewed Back checks");
        let definition = profile
            .get(&conduit_core::kind_id(crate::TEXT_MORSE_KIND))
            .unwrap();
        let mismatched_startup = [StartupParameterSignature {
            name: "tempo".into(),
            value_type: "Count".into(),
            default: Some("120".into()),
        }];
        let error = CanonicalBackCatalog::new()
            .insert_with_startup(
                definition,
                &mismatched_startup,
                &checked,
                crate::TEXT_MORSE_KIND,
            )
            .unwrap_err();
        assert_eq!(
            error,
            CanonicalBackError::FaceMismatch(crate::TEXT_MORSE_KIND.into())
        );
    }

    #[test]
    fn recursive_back_cycle_refuses_during_expansion() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        crate::install_text_catalogs(&mut startup, &mut profile).unwrap();
        crate::install_morse_catalogs(&mut startup, &mut profile).unwrap();
        let cyclic_source = r#"form text/morse-symbols (
    text: value/text@1 > symbols: value/morse-symbols@1
) {
    again: text/morse-symbols
    text > again > symbols
}
"#;
        let cyclic = check_syntax_document(&parse_syntax_document(cyclic_source), &startup)
            .expect("cyclic Back checks before expansion");
        let definition = profile
            .get(&conduit_core::kind_id(crate::TEXT_MORSE_SYMBOLS_KIND))
            .unwrap();
        let signature = startup.signature(crate::TEXT_MORSE_SYMBOLS_KIND).unwrap();
        let mut backs = CanonicalBackCatalog::new();
        backs
            .insert_with_startup(
                definition,
                &signature.startup_parameters,
                &cyclic,
                crate::TEXT_MORSE_SYMBOLS_KIND,
            )
            .unwrap();
        let caller_source = r#"form main (
    symbols: value/morse-symbols@1 >
) {
    source: text/literal("SOS")
    encode: text/morse-symbols
    source > encode > symbols
}
"#;
        let caller = check_syntax_document(&parse_syntax_document(caller_source), &startup)
            .expect("caller checks");
        let error = conduit_form::expand_canonical_form_for_authoring_with_backs(
            &caller, "main", &profile, &backs,
        )
        .unwrap_err();
        assert_eq!(error.code, "CND-FRM-035");
        assert!(error.message.contains("recursive form expansion cycle"));
    }
}
