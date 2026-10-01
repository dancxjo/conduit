use conduit_form::rust_binding::{generate_rust_bindings_with_codes, RustBindingOptions};
use conduit_form::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{collections::BTreeMap, env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_form::rust_binding::semantic_core::kind_id(
                conduit_form::rust_binding::semantic_core::RESOURCE_REFERENCE_INFO_ID,
            ),
        )
        .expect("resource references are one exact portable leaf");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &catalog,
    )
    .expect("human semantic Types must check");
    let generated = generate_rust_bindings_with_codes(
        &checked.native_types,
        &checked.codes,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            serde_record_types: ["InputSurfacePoint".into()].into(),
            serde_variant_exclusions: [
                "ChordInfo".into(),
                "ControlChordModifier".into(),
                "KeymapDisposition".into(),
            ]
            .into(),
            copy_record_types: [
                "ExperienceTemporalPolicy".into(),
                "ImageRegion".into(),
                "KeyEvent".into(),
            ]
            .into(),
            copy_record_value_getters: ["KeyEvent".into()].into(),
            direct_checked_record_constructors: ["ImageObservationReference".into()].into(),
            public_record_fields: ["ImageObservationReference".into(), "ImageTextRecord".into()]
                .into(),
            record_constructor_orders: BTreeMap::from([
                (
                    "KeyEvent".into(),
                    vec![
                        "usage".into(),
                        "transition".into(),
                        "left_control_after".into(),
                        "left_shift_after".into(),
                        "left_alt_after".into(),
                        "left_gui_after".into(),
                        "right_control_after".into(),
                        "right_shift_after".into(),
                        "right_alt_after".into(),
                        "right_gui_after".into(),
                    ],
                ),
                (
                    "ImageObservationReference".into(),
                    vec!["content".into(), "width".into(), "height".into()],
                ),
            ]),
            record_constructor_names: BTreeMap::from([
                ("KeyEvent".into(), "new_native".into()),
                ("ImageObservationReference".into(), "new_native".into()),
            ]),
            serde_variant_orders: BTreeMap::from([
                (
                    "ControlChordModifier".into(),
                    ["left", "right", "both"].map(String::from).into(),
                ),
                (
                    "ChordInfo".into(),
                    [
                        "cancel_or_escape",
                        "clear_or_refresh",
                        "repeat_or_replan",
                        "palette",
                        "inspect",
                        "plan",
                        "command",
                        "activate",
                    ]
                    .map(String::from)
                    .into(),
                ),
                (
                    "KeymapDisposition".into(),
                    ["no_text", "text", "cancelled", "refused"]
                        .map(String::from)
                        .into(),
                ),
            ]),
            ..RustBindingOptions::default()
        },
    )
    .expect("human semantic Types and codes must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated human bindings");
}
