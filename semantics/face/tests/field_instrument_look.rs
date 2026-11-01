//! Source/value custody, not evidence of a rendered or acknowledged Show.
use conduit_face::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CanonicalStartupValue, CheckedSyntaxDocument,
    StartupCatalog,
};

#[allow(dead_code)]
#[path = "../look_resources_build.rs"]
mod make_resources;

fn check(source: &str) -> Result<CheckedSyntaxDocument, conduit_plot::SyntaxCheckDiagnostic> {
    use conduit_core::{kind_id, QUANTITY_INFO_ID, RESOURCE_REFERENCE_INFO_ID};
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias("Quantity", kind_id(QUANTITY_INFO_ID))
        .unwrap();
    catalog
        .insert_value_kind_alias("ResourceRef", kind_id(RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    catalog
        .insert_structured_type(
            "ImageObservationReference",
            conduit_human::ImageObservationReference::semantic_type().unwrap(),
        )
        .unwrap();
    catalog
        .insert_structured_type(
            "TemporalInstant",
            conduit_time::NativeTemporalInstant::semantic_type().unwrap(),
        )
        .unwrap();
    // Retain the actual Source refinements and laws for Make admission.
    let source = [include_str!("../types.conduit"), LOOK_SOURCE, source].join("\n");
    check_syntax_document(&parse_syntax_document(&source), &catalog)
}

#[test]
fn made_resources_are_exact_ordinary_authored_values_of_the_existing_look_type() {
    let checked = check(FIELD_INSTRUMENT_LOOKS_SOURCE).unwrap();
    for (name, made) in [
        ("day", field_instrument_day_look().unwrap()),
        ("night", field_instrument_night_look().unwrap()),
    ] {
        let definition = checked
            .plots
            .iter()
            .find(|plot| plot.name == format!("look/field-instrument-{name}"))
            .unwrap();
        let Some(CanonicalStartupValue::Structured(authored)) =
            &definition.startup_parameters[0].default
        else {
            panic!("ordinary typed startup definition")
        };
        let authored = authored.try_concrete().unwrap();
        assert_eq!(authored, made.clone().into_structured().unwrap());
        let bytes = made.encode().unwrap();
        // Named browser-consumer fixtures retain exactly the checked authored
        // resources, not independently copied CSS colors/geometry or Face data.
        let browser_hex = match name {
            "day" => include_str!(
                "../../../proof/conformance/face-look/field-instrument-day-look-v2.hex"
            ),
            "night" => include_str!(
                "../../../proof/conformance/face-look/field-instrument-night-look-v2.hex"
            ),
            _ => unreachable!(),
        }
        .trim();
        let browser_bytes: Vec<u8> = (0..browser_hex.len())
            .step_by(2)
            .map(|offset| u8::from_str_radix(&browser_hex[offset..offset + 2], 16).unwrap())
            .collect();
        assert_eq!(browser_bytes, bytes);
        assert_eq!(authored.canonical_bytes().unwrap(), bytes);
        assert_eq!(make_resources::resource(&checked, name).unwrap(), bytes);
    }
}

#[test]
fn authored_metric_bounds_units_and_semantic_fields_cannot_bypass_look_checking() {
    for (valid, invalid) in [
        ("type_body_px: 18", "type_body_px: 65"),
        ("type_body_px: 18", "type_body_px: 18ms"),
        ("focus_width_px: 3", "focus_width_px: 0"),
        ("radius_panel_px: 0", "radius_panel_px: 33"),
        ("spoken: none(empty)", "actions: []"),
    ] {
        assert!(FIELD_INSTRUMENT_LOOKS_SOURCE.contains(valid));
        let altered = FIELD_INSTRUMENT_LOOKS_SOURCE.replace(valid, invalid);
        assert!(check(&altered).is_err(), "Source admitted {invalid}");
    }
}

#[test]
fn authored_visual_identity_law_is_admitted_before_retaining_the_resource() {
    let altered = FIELD_INSTRUMENT_LOOKS_SOURCE.replacen(
        "identity: \"conduit.look/field-instrument-day@1\"",
        "identity: \"conduit.look/foreign@1\"",
        1,
    );
    let diagnostic = check(&altered).unwrap_err();
    assert_eq!(diagnostic.code, "CND-FRM-058");
    assert!(diagnostic
        .message
        .contains("ViolatedInvariant { index: 0 }"));
    let source = [include_str!("../types.conduit"), LOOK_SOURCE, &altered].join("\n");
    assert!(source[diagnostic.span.start..diagnostic.span.end].contains("conduit.look/foreign@1"));
}
